"""Frame log analyser for the wgpu compositor benchmark.

Parses NDJSON frame logs produced by ``write_frame_log_row`` and emits a
structured telemetry report covering:

- Global pacing: delivery cadence, jitter, vblank-budget distribution
- Phase drift: ns-precision histogram, percentile table, PLL suitability
- Compositor bottleneck classification: GPU-overrun vs compositor-hold,
  derived from the ``slack_ms`` / ``cpu_frame_ms`` cross-reference
- Ping-pong (double-buffer phase lock) detection via ``ipc_delta_ms``
- Clustered stutter events with per-event recovery analysis
- Session-phase segmentation keyed on vblank multiplier regime

Usage::

    python frame_log_statistics.py <frame_log.json> [--markdown]
"""

import json
import sys
import statistics
import os
from dataclasses import dataclass, field
from typing import Optional


# ---------------------------------------------------------------------------
# Data structures
# ---------------------------------------------------------------------------


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

    @property
    def dominant_vblank_mul(self) -> Optional[int]:
        """Most common vblank multiplier in this phase, or None if unavailable."""
        muls = [f["vblank_mul"] for f in self.frames if "vblank_mul" in f]
        if not muls:
            return None
        return max(set(muls), key=muls.count)

    @property
    def mean_sync(self) -> float:
        scores = [f["sync"] for f in self.frames if "sync" in f]
        return statistics.mean(scores) if scores else 0.0


@dataclass
class StutterEvent:
    """A discrete frame delivery anomaly or sustained cluster thereof.

    A cluster groups consecutive anomalous frames within ``merge_gap``
    frames of each other into one compound event. ``vblanks_missed``
    counts integer whole-vblank skips (delta > 2× ideal); frames with
    fractional-vblank delivery (e.g. 1.5×) are flagged separately via
    ``fractional_slip``.
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


@dataclass
class BottleneckStats:
    """Compositor bottleneck classification derived from ``slack_ms`` and ``cpu_frame_ms``.

    Each frame with both fields present is classified into one of three categories:

    - **GPU overrun**: ``slack_ms`` is near zero relative to ``ideal_ms``,
      meaning the GPU was still executing at vblank time.
    - **Compositor hold**: ``slack_ms - cpu_frame_ms > ideal_ms * 0.5``,
      meaning the GPU finished early but the compositor delayed the flip —
      indicative of ``max_render_time`` policy or deep buffer queues.
    - **Healthy**: neither condition is true.
    """

    total_classified: int
    gpu_overrun_count: int
    compositor_hold_count: int
    healthy_count: int
    mean_hold_gap_ms: Optional[float]
    p95_hold_gap_ms: Optional[float]
    mean_slack_ms: Optional[float]
    p99_slack_ms: Optional[float]

    @property
    def gpu_overrun_pct(self) -> float:
        return (
            100.0 * self.gpu_overrun_count / self.total_classified
            if self.total_classified
            else 0.0
        )

    @property
    def compositor_hold_pct(self) -> float:
        return (
            100.0 * self.compositor_hold_count / self.total_classified
            if self.total_classified
            else 0.0
        )

    @property
    def healthy_pct(self) -> float:
        return (
            100.0 * self.healthy_count / self.total_classified
            if self.total_classified
            else 0.0
        )


@dataclass
class PingPongResult:
    """Result of double-buffer ping-pong detection via ``ipc_delta_ms``.

    A ping-pong pattern is a systematic alternation of fast and slow frames
    (e.g. 7 ms / 11 ms / 7 ms / 11 ms) that cancels out in rolling jitter
    averages but produces visible judder. Detected by measuring the rate of
    sign-flips in consecutive ``ipc_delta_ms`` values.
    """

    detected: bool
    sign_flip_rate: float
    """Fraction of consecutive ipc_delta_ms pairs with opposite signs.
    Values above ~0.70 indicate systematic alternation."""
    mean_fast_ms: Optional[float]
    mean_slow_ms: Optional[float]
    spread_ms: Optional[float]
    """Mean fast/slow separation; > 2 ms is perceptually salient."""


# ---------------------------------------------------------------------------
# Loader
# ---------------------------------------------------------------------------


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


# ---------------------------------------------------------------------------
# Analysis functions
# ---------------------------------------------------------------------------


def _vblank_distribution(frames: list[dict]) -> dict[int, int]:
    """Counts frames by their vblank multiplier.

    Args:
        frames: Full ordered list of frame records.

    Returns:
        Dict mapping vblank_mul → frame count, sorted ascending by key.
    """
    dist: dict[int, int] = {}
    for f in frames:
        mul = f.get("vblank_mul")
        if mul is not None:
            dist[mul] = dist.get(mul, 0) + 1
    return dict(sorted(dist.items()))


def _drift_percentiles(frames: list[dict]) -> dict[str, float]:
    """Computes phase drift percentiles in nanoseconds from ``drift_ns``.

    Falls back to ``drift_ms * 1_000_000`` when ``drift_ns`` is absent.
    Nanosecond precision matters when evaluating whether the drift signal
    is suitable for feeding a compositor's repaint-timer PLL.

    Args:
        frames: Full ordered list of frame records.

    Returns:
        Dict with keys min, p1, p5, p25, p50, p75, p95, p99, max (all in ns).
    """
    values_ns = sorted(
        f.get("drift_ns", int(f["drift_ms"] * 1_000_000)) for f in frames
    )
    n = len(values_ns)

    def pct(p: float) -> float:
        idx = max(0, min(n - 1, int(p / 100.0 * n)))
        return float(values_ns[idx])

    return {
        "min": float(values_ns[0]),
        "p1": pct(1),
        "p5": pct(5),
        "p25": pct(25),
        "p50": pct(50),
        "p75": pct(75),
        "p95": pct(95),
        "p99": pct(99),
        "max": float(values_ns[-1]),
    }


def _classify_bottleneck(frames: list[dict], ideal_ms: float) -> BottleneckStats:
    """Classifies each frame with slack_ms + cpu_frame_ms into GPU/compositor/healthy.

    A frame is **GPU overrun** when ``slack_ms < ideal_ms * 0.25``, meaning
    the GPU was almost certainly still executing at vblank time.

    A frame is **compositor hold** when ``slack_ms - cpu_frame_ms > ideal_ms * 0.5``,
    meaning the GPU finished the work well before presentation, the extra
    latency belongs to the compositor's scheduling policy.

    Args:
        frames: Full ordered list of frame records.
        ideal_ms: Monitor vblank period in milliseconds.

    Returns:
        BottleneckStats with per-category counts and hold-gap distribution.
    """
    classified = [
        f
        for f in frames
        if f.get("slack_ms") is not None and f.get("cpu_frame_ms") is not None
    ]

    if not classified:
        return BottleneckStats(0, 0, 0, 0, None, None, None, None)

    gpu_overrun, compositor_hold, healthy = [], [], []
    hold_gaps: list[float] = []
    slacks = [f["slack_ms"] for f in classified]

    gpu_threshold = ideal_ms * 0.25
    hold_threshold = ideal_ms * 0.5

    for f in classified:
        slack = f["slack_ms"]
        cpu = f["cpu_frame_ms"]
        hold_gap = slack - cpu

        if slack < gpu_threshold:
            gpu_overrun.append(f)
        elif hold_gap > hold_threshold:
            compositor_hold.append(f)
            hold_gaps.append(hold_gap)
        else:
            healthy.append(f)

    sorted_gaps = sorted(hold_gaps)
    sorted_slacks = sorted(slacks)

    return BottleneckStats(
        total_classified=len(classified),
        gpu_overrun_count=len(gpu_overrun),
        compositor_hold_count=len(compositor_hold),
        healthy_count=len(healthy),
        mean_hold_gap_ms=statistics.mean(hold_gaps) if hold_gaps else None,
        p95_hold_gap_ms=sorted_gaps[int(0.95 * len(sorted_gaps))]
        if sorted_gaps
        else None,
        mean_slack_ms=statistics.mean(slacks),
        p99_slack_ms=sorted_slacks[int(0.99 * len(sorted_slacks))],
    )


def _detect_ping_pong(frames: list[dict]) -> PingPongResult:
    """Detects double-buffer phase-lock ping-pong via ``ipc_delta_ms``.

    A sign-flip rate above 0.70 in consecutive ``ipc_delta_ms`` values
    indicates the compositor is alternating frame delivery between two
    fixed cadences, the classic double-buffer ping-pong fingerprint.

    Args:
        frames: Full ordered list of frame records.

    Returns:
        PingPongResult with detection flag and characterisation metrics.
    """
    ipcs = [f["ipc_delta_ms"] for f in frames if f.get("ipc_delta_ms") is not None]

    if len(ipcs) < 10:
        return PingPongResult(False, 0.0, None, None, None)

    sign_flips = sum(1 for i in range(1, len(ipcs)) if ipcs[i] * ipcs[i - 1] < 0)
    flip_rate = sign_flips / (len(ipcs) - 1)

    if flip_rate < 0.70:
        return PingPongResult(False, flip_rate, None, None, None)

    # Characterise the two cadences by splitting deltas on above/below median.
    deltas = [f["delta_ms"] for f in frames]
    median_d = statistics.median(deltas)
    fast = [d for d in deltas if d <= median_d]
    slow = [d for d in deltas if d > median_d]

    mean_fast = statistics.mean(fast) if fast else None
    mean_slow = statistics.mean(slow) if slow else None
    spread = (
        (mean_slow - mean_fast)
        if mean_fast is not None and mean_slow is not None
        else None
    )

    return PingPongResult(
        detected=True,
        sign_flip_rate=flip_rate,
        mean_fast_ms=mean_fast,
        mean_slow_ms=mean_slow,
        spread_ms=spread,
    )


def _detect_stutter_events(
    frames: list[dict],
    ideal_ms: float,
    stutter_threshold_x: float = 1.5,
    merge_gap: int = 5,
    recovery_window: int = 8,
) -> list[StutterEvent]:
    """Identifies and clusters frame delivery anomalies.

    A frame is anomalous if its delta >= ``stutter_threshold_x`` × ideal_ms.
    Consecutive anomalous frames whose global indices are within ``merge_gap``
    of each other are merged into a single compound StutterEvent so that
    sustained cadence degradations are not reported as dozens of independent
    minor events.

    Distinguishes whole-vblank misses (delta > 2× ideal, ``vblanks_missed > 0``)
    from fractional-vblank slips (1.5×–2× ideal, ``fractional_slip = True``).

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


# ---------------------------------------------------------------------------
# Labels and verdicts
# ---------------------------------------------------------------------------


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


def _sync_label(avg_sync: float) -> str:
    if avg_sync >= 90:
        return "EXCELLENT"
    if avg_sync >= 70:
        return "GOOD"
    if avg_sync >= 50:
        return "MARGINAL"
    return "POOR"


def _bottleneck_verdict(bn: BottleneckStats) -> str:
    """One-line compositor bottleneck summary."""
    if bn.total_classified == 0:
        return "Insufficient data (slack_ms not present in log)"
    dominant = max(
        ("GPU overrun", bn.gpu_overrun_pct),
        ("Compositor hold", bn.compositor_hold_pct),
        ("Healthy", bn.healthy_pct),
        key=lambda x: x[1],
    )
    return f"{dominant[0]} dominant ({dominant[1]:.1f}% of classified frames)"


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


# ---------------------------------------------------------------------------
# Report renderers
# ---------------------------------------------------------------------------


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
    drift_pct: dict[str, float],
    vblank_dist: dict[int, int],
    bottleneck: BottleneckStats,
    ping_pong: PingPongResult,
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

    print("## Vblank Budget Distribution\n")
    print("| Vblank × | Frames | Share |")
    print("|---|---|---|")
    for mul, count in vblank_dist.items():
        pct = 100.0 * count / len(frames)
        label = "on-time" if mul == 1 else f"{mul - 1} dropped"
        print(f"| {mul}× ({label}) | {count} | {pct:.1f}% |")
    print()

    print("## Phase Drift\n")
    print("| Metric | Value |")
    print("|---|---|")
    print(f"| Avg Phase Drift | {avg_drift:+.4f} ms |")
    print(f"| Max Phase Drift | {max_drift:+.4f} ms |")
    print(f"| Drift Std Dev | {drift_stdev:.4f} ms |")
    print(f"| Avg Sync Score | {avg_sync:.2f} % ({_sync_label(avg_sync)}) |\n")

    print("### Drift Percentiles (nanoseconds)\n")
    print("| Percentile | ns |")
    print("|---|---|")
    for label, val in drift_pct.items():
        print(f"| {label} | {val:+.0f} ns |")
    print()

    print("## Compositor Bottleneck Analysis\n")
    if bottleneck.total_classified == 0:
        print(
            "*`slack_ms` not present in log, bottleneck classification unavailable.*\n"
        )
    else:
        print(f"**{_bottleneck_verdict(bottleneck)}**\n")
        print(f"- Frames classified: {bottleneck.total_classified}")
        print(
            f"- GPU overrun: {bottleneck.gpu_overrun_count} ({bottleneck.gpu_overrun_pct:.1f}%)"
        )
        print(
            f"- Compositor hold: {bottleneck.compositor_hold_count} ({bottleneck.compositor_hold_pct:.1f}%)"
        )
        print(f"- Healthy: {bottleneck.healthy_count} ({bottleneck.healthy_pct:.1f}%)")
        if bottleneck.mean_hold_gap_ms is not None:
            print(f"- Mean hold gap: {bottleneck.mean_hold_gap_ms:.4f} ms")
            print(f"- P95 hold gap: {bottleneck.p95_hold_gap_ms:.4f} ms")
        if bottleneck.mean_slack_ms is not None:
            print(f"- Mean slack: {bottleneck.mean_slack_ms:.4f} ms")
            print(f"- P99 slack: {bottleneck.p99_slack_ms:.4f} ms")
        print()

    print("## Double-Buffer Ping-Pong\n")
    if ping_pong.detected:
        print(f"**DETECTED** (sign-flip rate: {ping_pong.sign_flip_rate:.2f})\n")
        print(f"- Mean fast cadence: {ping_pong.mean_fast_ms:.4f} ms")
        print(f"- Mean slow cadence: {ping_pong.mean_slow_ms:.4f} ms")
        print(f"- Frame-time spread: {ping_pong.spread_ms:.4f} ms")
        print()
        print("> Systematic fast/slow alternation indicates the compositor is locked")
        print("> to two fixed delivery slots, visible judder even at nominal FPS.\n")
    else:
        print(f"Not detected (sign-flip rate: {ping_pong.sign_flip_rate:.2f})\n")

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
        print("| # | GLOBAL IDX | MEAN Δ | EFF. Hz | JITTER | DOM. VBLANK× | SYNC |")
        print("|---|---|---|---|---|---|---|")
        for idx, ph in enumerate(phases, 1):
            frame_range = f"{ph.start_frame}–{ph.end_frame}"
            dom = f"{ph.dominant_vblank_mul}×" if ph.dominant_vblank_mul else "—"
            print(
                f"| {idx} | {frame_range} | {ph.mean_delta:.4f} ms | "
                f"{ph.effective_hz:.1f} Hz | {ph.jitter:.4f} ms | {dom} | {ph.mean_sync:.1f}% |"
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


def _print_terminal_report(
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
    drift_pct: dict[str, float],
    vblank_dist: dict[int, int],
    bottleneck: BottleneckStats,
    ping_pong: PingPongResult,
    stutter_events: list[StutterEvent],
    phases: list[SessionPhase],
) -> None:
    """Prints the telemetry report to stdout in aligned plain-text format."""
    W = 72
    sep = "─" * W

    print(f"\n{'━' * W}")
    print(f"  TELEMETRY REPORT  ·  {os.path.basename(file_path)}")
    print(f"{'━' * W}")

    duration_s = sum(f["delta_ms"] for f in frames) / 1000.0
    print(f"\n  TARGET            {target_hz:.1f} Hz  ({ideal:.4f} ms/frame)")
    print(f"  FRAMES ANALYSED   {len(frames)}")
    print(f"  SESSION DURATION  {duration_s:.2f} s\n")

    # ── Global Pacing ───────────────────────────────────────────────────────
    print(sep)
    print("  GLOBAL PACING")
    print(sep)
    print(
        f"  Avg Delivery Time     {avg_delta:8.4f} ms   [{_performance_label(multiplier)}]"
    )
    print(f"  V-Sync Multiplier     {multiplier:8.2f} x")
    print(f"  Jitter (IFI delta)    {jitter:8.4f} ms   [{_jitter_label(jitter)}]")

    # ── Vblank Budget Distribution ──────────────────────────────────────────
    print(sep)
    print("  VBLANK BUDGET DISTRIBUTION")
    print(sep)
    for mul, count in vblank_dist.items():
        pct = 100.0 * count / len(frames)
        bar_len = int(pct / 2)
        label = "on-time" if mul == 1 else f"{mul - 1} dropped"
        bar = "█" * bar_len
        print(f"  {mul:>2}× ({label:<10})  {count:>6} frames  {pct:5.1f}%  {bar}")

    # ── Phase Drift ─────────────────────────────────────────────────────────
    print(sep)
    print("  PHASE DRIFT")
    print(sep)
    print(f"  Avg Phase Drift       {avg_drift:+8.4f} ms")
    print(f"  Max Phase Drift       {max_drift:+8.4f} ms")
    print(f"  Drift Std Dev         {drift_stdev:8.4f} ms")
    print(f"  Avg Sync Score        {avg_sync:8.2f} %   [{_sync_label(avg_sync)}]")
    print()
    print(f"  {'Percentile':<10}  {'drift_ns':>14}   {'drift_ms':>10}")
    print(f"  {'─' * 10}  {'─' * 14}   {'─' * 10}")
    for label, val_ns in drift_pct.items():
        print(f"  {label:<10}  {val_ns:>+14.0f} ns   {val_ns / 1_000_000:>+10.4f} ms")

    # ── Compositor Bottleneck ───────────────────────────────────────────────
    print(f"\n{sep}")
    print("  COMPOSITOR BOTTLENECK ANALYSIS")
    print(sep)
    if bottleneck.total_classified == 0:
        print("  slack_ms not present, classification unavailable\n")
    else:
        print(f"  Classified frames      {bottleneck.total_classified}")
        print(
            f"  GPU overrun            {bottleneck.gpu_overrun_count:>6}  ({bottleneck.gpu_overrun_pct:5.1f}%)"
        )
        print(
            f"  Compositor hold        {bottleneck.compositor_hold_count:>6}  ({bottleneck.compositor_hold_pct:5.1f}%)"
        )
        print(
            f"  Healthy                {bottleneck.healthy_count:>6}  ({bottleneck.healthy_pct:5.1f}%)"
        )
        if bottleneck.mean_hold_gap_ms is not None:
            print(f"  Mean hold gap         {bottleneck.mean_hold_gap_ms:8.4f} ms")
            print(f"  P95  hold gap         {bottleneck.p95_hold_gap_ms:8.4f} ms")
        if bottleneck.mean_slack_ms is not None:
            print(f"  Mean slack            {bottleneck.mean_slack_ms:8.4f} ms")
            print(f"  P99  slack            {bottleneck.p99_slack_ms:8.4f} ms")
        print(f"\n  [{_bottleneck_verdict(bottleneck)}]")

    # ── Ping-Pong ───────────────────────────────────────────────────────────
    print(f"\n{sep}")
    print("  DOUBLE-BUFFER PING-PONG")
    print(sep)
    if ping_pong.detected:
        print(f"  DETECTED  (sign-flip rate: {ping_pong.sign_flip_rate:.2f})")
        print(f"  Mean fast cadence     {ping_pong.mean_fast_ms:8.4f} ms")
        print(f"  Mean slow cadence     {ping_pong.mean_slow_ms:8.4f} ms")
        print(f"  Frame-time spread     {ping_pong.spread_ms:8.4f} ms")
        print()
        print("  Systematic alternation: compositor locked to two delivery slots.")
        print("  Visible judder likely even at nominal FPS.")
    else:
        print(f"  Not detected  (sign-flip rate: {ping_pong.sign_flip_rate:.2f})")

    # ── Stutter Events ──────────────────────────────────────────────────────
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

    # ── Session Phases ──────────────────────────────────────────────────────
    if len(phases) > 1:
        print(f"\n{sep}")
        print("  SESSION PHASES  (cadence regimes, keyed on global frame index)")
        print(sep)
        col = (
            f"  {'#':>3}  {'GLOBAL IDX':>14}  {'MEAN Δ':>9}"
            f"  {'EFF. Hz':>8}  {'JITTER':>9}  {'DOM.×':>6}  {'SYNC':>6}"
        )
        print(col)
        print(
            f"  {'─' * 3}  {'─' * 14}  {'─' * 9}  {'─' * 8}  {'─' * 9}  {'─' * 6}  {'─' * 6}"
        )
        for idx, ph in enumerate(phases, 1):
            frame_range = f"{ph.start_frame}–{ph.end_frame}"
            dom = f"{ph.dominant_vblank_mul}×" if ph.dominant_vblank_mul else "—"
            print(
                f"  {idx:>3}  {frame_range:>14}  {ph.mean_delta:>8.4f}ms"
                f"  {ph.effective_hz:>7.1f}Hz  {ph.jitter:>8.4f}ms"
                f"  {dom:>6}  {ph.mean_sync:>5.1f}%"
            )

    # ── Verdict ─────────────────────────────────────────────────────────────
    print(f"\n{sep}")
    print(f"  VERDICT: {_verdict(multiplier, jitter, avg_sync)}")
    print(f"{'━' * W}\n")


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def analyze_frame_log(file_path: str, markdown: bool = False) -> None:
    """Prints a telemetry report for a frame log produced by the wgpu benchmark.

    Covers global pacing stats, vblank budget distribution, phase drift with
    nanosecond-precision percentile table, compositor bottleneck classification
    (GPU overrun vs compositor hold via slack_ms cross-reference), double-buffer
    ping-pong detection, clustered stutter events with recovery analysis, and a
    per-phase cadence breakdown.

    Args:
        file_path: Path to the NDJSON frame log file.
        markdown: When True, emit Markdown instead of plain-text terminal output.
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

    drift_pct = _drift_percentiles(frames)
    vblank_dist = _vblank_distribution(frames)
    bottleneck = _classify_bottleneck(frames, ideal)
    ping_pong = _detect_ping_pong(frames)
    stutter_events = _detect_stutter_events(frames, ideal)
    phases = _segment_phases(frames, ideal)

    common_args = dict(
        file_path=file_path,
        frames=frames,
        ideal=ideal,
        target_hz=target_hz,
        avg_delta=avg_delta,
        jitter=jitter,
        avg_sync=avg_sync,
        avg_drift=avg_drift,
        max_drift=max_drift,
        drift_stdev=drift_stdev,
        multiplier=multiplier,
        drift_pct=drift_pct,
        vblank_dist=vblank_dist,
        bottleneck=bottleneck,
        ping_pong=ping_pong,
        stutter_events=stutter_events,
        phases=phases,
    )

    if markdown:
        _print_markdown_report(**common_args)
    else:
        _print_terminal_report(**common_args)


if __name__ == "__main__":
    args = sys.argv[1:]
    use_md = "--markdown" in args
    if use_md:
        args.remove("--markdown")

    if not args:
        print("Usage: python frame_log_statistics.py <frame_log.json> [--markdown]")
        sys.exit(1)

    analyze_frame_log(args[0], markdown=use_md)
