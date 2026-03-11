import json
import sys
import statistics
import os
from dataclasses import dataclass, field
from typing import Optional


@dataclass
class SessionPhase:
    """A contiguous run of frames sharing a coherent delivery cadence.

    Phases are segmented when the rolling inter-frame delta crosses a
    multiple-of-ideal boundary, signalling a compositor regime change.
    """

    start_frame: int
    end_frame: int
    frames: list = field(default_factory=list, repr=False)

    @property
    def duration_ms(self) -> float:
        return sum(f["delta_ms"] for f in self.frames)

    @property
    def mean_delta(self) -> float:
        return statistics.mean(f["delta_ms"] for f in self.frames)

    @property
    def effective_hz(self) -> float:
        return 1000.0 / self.mean_delta if self.mean_delta > 0 else 0.0

    @property
    def jitter(self) -> float:
        deltas = [f["delta_ms"] for f in self.frames]
        if len(deltas) < 2:
            return 0.0
        return statistics.mean(
            abs(deltas[i] - deltas[i - 1]) for i in range(1, len(deltas))
        )


@dataclass
class StutterEvent:
    """A discrete frame delivery anomaly or sustained cluster thereof.

    A cluster groups consecutive anomalous frames within `merge_gap`
    frames of each other into one compound event. `vblanks_missed`
    counts integer whole-vblank skips (delta > 2× ideal); frames with
    fractional-vblank delivery (e.g. 1.5×) are flagged separately via
    `fractional_slip`.
    """

    frame_index: int
    frame_number: int
    delta_ms: float
    ideal_ms: float
    vblanks_missed: int
    fractional_slip: bool
    cluster_size: int = 1
    cluster_frame_numbers: list = field(default_factory=list, repr=False)
    recovery_frames: list = field(default_factory=list, repr=False)

    @property
    def severity(self) -> str:
        if self.cluster_size >= 10:
            return "CLUSTER"
        if self.vblanks_missed >= 7:
            return "CRITICAL"
        if self.vblanks_missed >= 3:
            return "SEVERE"
        if self.fractional_slip and self.cluster_size > 1:
            return "SLIP"
        return "MINOR"

    @property
    def recovery_jitter(self) -> Optional[float]:
        if len(self.recovery_frames) < 2:
            return None
        deltas = [f["delta_ms"] for f in self.recovery_frames]
        return statistics.mean(
            abs(deltas[i] - deltas[i - 1]) for i in range(1, len(deltas))
        )


def _load_frames(file_path: str) -> list[dict]:
    """Parses NDJSON frame log, skipping comment lines and malformed records.

    Detects frame-counter resets (non-monotonic frame numbers) and
    re-indexes all frames with a session-global monotonic index so that
    phase segmentation and stutter detection are not confused by resets.

    Args:
        file_path: Path to the frame log file produced by write_frame_log_row.

    Returns:
        List of parsed frame dicts with an added ``global_index`` field.

    Raises:
        SystemExit: On missing file or empty/invalid log.
    """
    if not os.path.exists(file_path):
        print(f"Error: File '{file_path}' not found.")
        sys.exit(1)

    raw: list[dict] = []
    with open(file_path) as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            try:
                raw.append(json.loads(line))
            except json.JSONDecodeError:
                continue

    if not raw:
        print(f"Error: No valid JSON data found in '{file_path}'.")
        sys.exit(1)

    resets: list[int] = []
    prev_frame_no = raw[0]["frame"]
    for i, r in enumerate(raw):
        if i > 0 and r["frame"] <= prev_frame_no and r["frame"] < 10:
            resets.append(i)
        prev_frame_no = r["frame"]
        r["global_index"] = i

    if resets:
        print(
            f"  NOTE: Frame counter reset detected at record indices "
            f"{resets} (concatenated sessions). Global index assigned.\n"
        )

    return raw


def _detect_stutter_events(
    frames: list[dict],
    ideal_ms: float,
    stutter_threshold_x: float = 1.5,
    merge_gap: int = 5,
    recovery_window: int = 8,
) -> list[StutterEvent]:
    """Identifies and clusters frame delivery anomalies.

    A frame is anomalous if its delta >= `stutter_threshold_x` × ideal_ms.
    Consecutive anomalous frames whose global indices are within `merge_gap`
    of each other are merged into a single compound StutterEvent so that
    sustained cadence degradations (e.g. a 60-frame ~126Hz slip) are not
    reported as dozens of independent minor events.

    Distinguishes whole-vblank misses (delta > 2× ideal, `vblanks_missed > 0`)
    from fractional-vblank slips (1.5×–2× ideal, `fractional_slip = True`).

    Args:
        frames: Full ordered list of frame records with ``global_index``.
        ideal_ms: Monitor vblank period in milliseconds.
        stutter_threshold_x: Multiplier above which a frame is anomalous.
        merge_gap: Max gap in global_index between anomalies to merge.
        recovery_window: Frames to capture after each event for recovery analysis.

    Returns:
        List of StutterEvent instances in chronological order.
    """
    anomaly_indices: list[int] = [
        i
        for i, f in enumerate(frames)
        if f["delta_ms"] >= ideal_ms * stutter_threshold_x
    ]

    if not anomaly_indices:
        return []

    clusters: list[list[int]] = []
    current_cluster = [anomaly_indices[0]]
    for idx in anomaly_indices[1:]:
        if idx - current_cluster[-1] <= merge_gap:
            current_cluster.append(idx)
        else:
            clusters.append(current_cluster)
            current_cluster = [idx]
    clusters.append(current_cluster)

    events: list[StutterEvent] = []
    for cluster in clusters:
        anchor_i = cluster[0]
        anchor = frames[anchor_i]
        worst = max(cluster, key=lambda i: frames[i]["delta_ms"])
        worst_frame = frames[worst]

        vblanks_missed = max(0, int(worst_frame["delta_ms"] / ideal_ms) - 1)
        fractional = (
            ideal_ms * 1.25 <= worst_frame["delta_ms"] < ideal_ms * 2.0
            and vblanks_missed == 0
        )

        recovery_start = cluster[-1] + 1
        recovery = frames[recovery_start : recovery_start + recovery_window]

        events.append(
            StutterEvent(
                frame_index=anchor_i,
                frame_number=anchor["frame"],
                delta_ms=worst_frame["delta_ms"],
                ideal_ms=ideal_ms,
                vblanks_missed=vblanks_missed,
                fractional_slip=fractional,
                cluster_size=len(cluster),
                cluster_frame_numbers=[frames[i]["frame"] for i in cluster],
                recovery_frames=recovery,
            )
        )

    return events


def _segment_phases(
    frames: list[dict],
    ideal_ms: float,
    smoothing: int = 20,
    threshold_x: float = 1.2,
) -> list[SessionPhase]:
    """Splits the session into contiguous delivery-cadence phases.

    Uses ``global_index`` for continuity so that frame-counter resets in
    concatenated log files do not create phantom phase boundaries.

    Args:
        frames: Full ordered list of frame records with ``global_index``.
        ideal_ms: Monitor vblank period in milliseconds.
        smoothing: Rolling window size for cadence classification.
        threshold_x: Rolling-mean multiplier above which cadence is degraded.

    Returns:
        List of SessionPhase instances in chronological order.
    """
    threshold = ideal_ms * threshold_x
    phases: list[SessionPhase] = []
    current: list[dict] = []
    current_degraded: Optional[bool] = None

    for i, frame in enumerate(frames):
        window = frames[max(0, i - smoothing) : i + 1]
        rolling_mean = statistics.mean(f["delta_ms"] for f in window)
        is_degraded = rolling_mean >= threshold

        if current_degraded is None:
            current_degraded = is_degraded

        if is_degraded != current_degraded:
            if len(current) >= smoothing:
                phases.append(
                    SessionPhase(
                        start_frame=current[0]["global_index"],
                        end_frame=current[-1]["global_index"],
                        frames=current,
                    )
                )
            elif phases:
                phases[-1].frames.extend(current)
                phases[-1].end_frame = current[-1]["global_index"]
            current = []
            current_degraded = is_degraded

        current.append(frame)

    if current:
        if phases:
            phases[-1].frames.extend(current)
            phases[-1].end_frame = current[-1]["global_index"]
        else:
            phases.append(
                SessionPhase(
                    start_frame=current[0]["global_index"],
                    end_frame=current[-1]["global_index"],
                    frames=current,
                )
            )

    return phases


def _performance_label(multiplier: float) -> str:
    if 0.95 <= multiplier <= 1.05:
        return "PERFECT (Native Refresh)"
    if multiplier < 2.10:
        return "GOOD (Consistent Half-Rate)"
    return "PERFORMANCE LIMITED (Dropped Beats)"


def _jitter_label(jitter: float) -> str:
    if jitter < 0.3:
        return "LOCKED"
    if jitter < 1.0:
        return "STABLE"
    return "STUTTERY"


def _verdict(multiplier: float, jitter: float, avg_sync: float) -> str:
    if 0.95 <= multiplier <= 1.05:
        if jitter < 0.5:
            base = "NATIVE PERFORMANCE. GPU is perfectly tracking the monitor."
            if avg_sync < 70:
                base += (
                    "\n  NOTE: Low avg sync score is expected over long sessions, the"
                    "\n        fixed phase origin precesses through all vblank phases as"
                    "\n        sub-microsecond residuals accumulate. Per-frame drift values"
                    "\n        and stutter events remain accurate."
                )
            return base
        return "NATIVE BUT JITTERY. Correct speed, but delivery spacing is uneven."
    if multiplier > 2.0:
        return "GPU BOUND. Throughput is significantly lower than refresh rate."
    return "ACCEPTABLE. Standard presentation timing."


def _print_markdown_report(
    file_path: str,
    frames: list[dict],
    ideal: float,
    target_hz: float,
    avg_delta: float,
    jitter: float,
    avg_sync: float,
    avg_drift: float,
    max_drift: float,
    drift_stdev: float,
    multiplier: float,
    stutter_events: list[StutterEvent],
    phases: list[SessionPhase],
) -> None:
    """Prints the telemetry report formatted as Markdown."""
    duration_s = sum(f["delta_ms"] for f in frames) / 1000.0

    print(f"# Telemetry Report: `{os.path.basename(file_path)}`\n")
    print(f"- **Target:** {target_hz:.1f} Hz ({ideal:.4f} ms/frame)")
    print(f"- **Frames Analysed:** {len(frames)}")
    print(f"- **Session Duration:** {duration_s:.2f} s\n")

    print("## Global Pacing\n")
    print("| Metric | Value | Evaluation |")
    print("|---|---|---|")
    print(
        f"| Avg Delivery Time | {avg_delta:.4f} ms | {_performance_label(multiplier)} |"
    )
    print(f"| V-Sync Multiplier | {multiplier:.2f} x | |")
    print(f"| Jitter (IFI delta) | {jitter:.4f} ms | {_jitter_label(jitter)} |\n")

    print("## Phase Drift\n")
    print("| Metric | Value |")
    print("|---|---|")
    print(f"| Avg Phase Drift | {avg_drift:+.4f} ms |")
    print(f"| Max Phase Drift | {max_drift:+.4f} ms |")
    print(f"| Drift Std Dev | {drift_stdev:.4f} ms |")
    print(f"| Avg Sync Score | {avg_sync:.2f} % |\n")

    print("## Stutter Events\n")
    if not stutter_events:
        print("None detected.\n")
    else:
        total_vblanks_lost = sum(e.vblanks_missed for e in stutter_events)
        total_anomalous = sum(e.cluster_size for e in stutter_events)
        session_pct = 100 * total_anomalous / len(frames)

        print(f"- **Distinct events:** {len(stutter_events)}")
        print(
            f"- **Anomalous frames:** {total_anomalous} ({session_pct:.2f}% of session)"
        )
        print(f"- **Vblanks lost:** {total_vblanks_lost}\n")

        print("| IDX | WORST Δ | SZ | MISSED | SEVERITY | RECOV. JITTER |")
        print("|---|---|---|---|---|---|")
        for e in stutter_events:
            rj = (
                f"{e.recovery_jitter:.4f} ms"
                if e.recovery_jitter is not None
                else "n/a"
            )
            slip_marker = "~" if e.fractional_slip else ""
            print(
                f"| {e.frame_index} | {slip_marker}{e.delta_ms:.4f} ms | "
                f"{e.cluster_size} | {e.vblanks_missed} | {e.severity} | {rj} |"
            )
        print(
            "\n> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)\n"
        )

    if len(phases) > 1:
        print("## Session Phases\n")
        print("*Cadence regimes, keyed on global frame index*\n")
        print("| # | GLOBAL IDX | MEAN Δ | EFF. Hz | JITTER |")
        print("|---|---|---|---|---|")
        for idx, ph in enumerate(phases, 1):
            frame_range = f"{ph.start_frame}–{ph.end_frame}"
            print(
                f"| {idx} | {frame_range} | {ph.mean_delta:.4f} ms | "
                f"{ph.effective_hz:.1f} Hz | {ph.jitter:.4f} ms |"
            )
        print()

    print("## Verdict\n")
    verdict_lines = _verdict(multiplier, jitter, avg_sync).split("\n")
    print(f"**{verdict_lines[0]}**\n")
    if len(verdict_lines) > 1:
        for line in verdict_lines[1:]:
            if line.strip():
                print(f"> {line.strip()}")
        print()


def analyze_frame_log(file_path: str, markdown: bool = False) -> None:
    """Prints a telemetry report for a frame log produced by the wgpu benchmark.

    Covers global pacing stats, clustered stutter events with recovery
    analysis, and a per-phase cadence breakdown keyed on global frame index
    to handle concatenated/reset log files correctly.

    Args:
        file_path: Path to the NDJSON frame log file.
    """
    frames = _load_frames(file_path)

    deltas = [f["delta_ms"] for f in frames]
    drifts = [f["drift_ms"] for f in frames]
    sync_scores = [f["sync"] for f in frames]
    ideal = frames[0]["ideal_ms"]
    target_hz = 1000.0 / ideal

    avg_delta = statistics.mean(deltas)
    jitter = statistics.mean(
        abs(deltas[i] - deltas[i - 1]) for i in range(1, len(deltas))
    )
    avg_sync = statistics.mean(sync_scores)
    avg_drift = statistics.mean(drifts)
    max_drift = max(drifts, key=abs)
    drift_stdev = statistics.stdev(drifts) if len(drifts) > 1 else 0.0
    multiplier = avg_delta / ideal

    stutter_events = _detect_stutter_events(frames, ideal)
    phases = _segment_phases(frames, ideal)

    if markdown:
        _print_markdown_report(
            file_path,
            frames,
            ideal,
            target_hz,
            avg_delta,
            jitter,
            avg_sync,
            avg_drift,
            max_drift,
            drift_stdev,
            multiplier,
            stutter_events,
            phases,
        )
        return

    W = 70
    sep = "─" * W

    print(f"\n{'━' * W}")
    print(f"  TELEMETRY REPORT  ·  {os.path.basename(file_path)}")
    print(f"{'━' * W}")

    print(f"\n  TARGET            {target_hz:.1f} Hz  ({ideal:.4f} ms/frame)")
    print(f"  FRAMES ANALYSED   {len(frames)}")
    print(f"  SESSION DURATION  {sum(deltas) / 1000:.2f} s\n")

    print(sep)
    print("  GLOBAL PACING")
    print(sep)
    print(
        f"  Avg Delivery Time     {avg_delta:8.4f} ms   [{_performance_label(multiplier)}]"
    )
    print(f"  V-Sync Multiplier     {multiplier:8.2f} x")
    print(f"  Jitter (IFI delta)    {jitter:8.4f} ms   [{_jitter_label(jitter)}]")
    print(sep)
    print("  PHASE DRIFT")
    print(sep)
    print(f"  Avg Phase Drift       {avg_drift:+8.4f} ms")
    print(f"  Max Phase Drift       {max_drift:+8.4f} ms")
    print(f"  Drift Std Dev         {drift_stdev:8.4f} ms")
    print(f"  Avg Sync Score        {avg_sync:8.2f} %")

    print(f"\n{sep}")
    print("  STUTTER EVENTS")
    print(sep)

    if not stutter_events:
        print("  None detected.\n")
    else:
        total_vblanks_lost = sum(e.vblanks_missed for e in stutter_events)
        total_anomalous = sum(e.cluster_size for e in stutter_events)
        print(f"  Distinct events    {len(stutter_events)}")
        print(
            f"  Anomalous frames   {total_anomalous}  ({100 * total_anomalous / len(frames):.2f}% of session)"
        )
        print(f"  Vblanks lost       {total_vblanks_lost}\n")

        col = (
            f"  {'IDX':>7}  {'WORST Δ':>10}  {'SZ':>4}  {'MISSED':>6}"
            f"  {'SEVERITY':<9}  {'RECOV. JITTER':>13}"
        )
        print(col)
        print(f"  {'─' * 7}  {'─' * 10}  {'─' * 4}  {'─' * 6}  {'─' * 9}  {'─' * 13}")
        for e in stutter_events:
            rj = (
                f"{e.recovery_jitter:.4f} ms"
                if e.recovery_jitter is not None
                else "        n/a"
            )
            slip_marker = "~" if e.fractional_slip else " "
            print(
                f"  {e.frame_index:>7}  {slip_marker}{e.delta_ms:>8.4f}ms"
                f"  {e.cluster_size:>4}  {e.vblanks_missed:>6}"
                f"  {e.severity:<9}  {rj:>13}"
            )
        print("\n  ~ = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)")

    if len(phases) > 1:
        print(f"\n{sep}")
        print("  SESSION PHASES  (cadence regimes, keyed on global frame index)")
        print(sep)
        col = f"  {'#':>3}  {'GLOBAL IDX':>14}  {'MEAN Δ':>9}  {'EFF. Hz':>8}  {'JITTER':>9}"
        print(col)
        print(f"  {'─' * 3}  {'─' * 14}  {'─' * 9}  {'─' * 8}  {'─' * 9}")
        for idx, ph in enumerate(phases, 1):
            frame_range = f"{ph.start_frame}–{ph.end_frame}"
            print(
                f"  {idx:>3}  {frame_range:>14}  {ph.mean_delta:>8.4f}ms"
                f"  {ph.effective_hz:>7.1f}Hz  {ph.jitter:>8.4f}ms"
            )

    print(f"\n{sep}")
    print(f"  VERDICT: {_verdict(multiplier, jitter, avg_sync)}")
    print(f"{'━' * W}\n")


if __name__ == "__main__":
    args = sys.argv[1:]
    use_md = "--markdown" in args
    if use_md:
        args.remove("--markdown")

    if not args:
        print("Usage: python analyze.py <frame_log.json> [--markdown]")
        sys.exit(1)

    analyze_frame_log(args[0], markdown=use_md)
