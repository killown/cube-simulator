import json
import sys
import statistics
import os


def analyze_frame_log(file_path):
    if not os.path.exists(file_path):
        print(f"Error: File '{file_path}' not found.")
        return

    frames = []
    with open(file_path, "r") as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            try:
                frames.append(json.loads(line))
            except json.JSONDecodeError:
                continue

    if not frames:
        print(f"Error: No valid JSON data found in '{file_path}'.")
        return

    deltas = [f["delta_ms"] for f in frames]
    drifts = [f["drift_ms"] for f in frames]
    sync_scores = [f["sync"] for f in frames]
    ideal = frames[0]["ideal_ms"]

    avg_delta = statistics.mean(deltas)
    jitter = statistics.mean(
        [abs(deltas[i] - deltas[i - 1]) for i in range(1, len(deltas))]
    )
    avg_sync = statistics.mean(sync_scores)

    avg_drift = statistics.mean(drifts)
    max_drift = max(drifts, key=abs)
    drift_standard_deviation = statistics.stdev(drifts) if len(drifts) > 1 else 0

    performance_multiplier = avg_delta / ideal

    if 0.95 <= performance_multiplier <= 1.05:
        performance_label = "PERFECT (Native Refresh)"
    elif performance_multiplier < 2.10:
        performance_label = "GOOD (Consistent Half-Rate)"
    else:
        performance_label = "PERFORMANCE LIMITED (Dropped Beats)"

    if jitter < 0.3:
        jitter_label = "LOCKED"
    elif jitter < 1.0:
        jitter_label = "STABLE"
    else:
        jitter_label = "STUTTERY"

    print(f"--- TELEMETRY REPORT: {file_path} ---")
    print(f"Monitor Target Interval:      {ideal:6.4f} ms")
    print(f"Average Delivery Time:        {avg_delta:6.4f} ms -> [{performance_label}]")
    print(
        f"V-Sync Multiplier:            {performance_multiplier:6.2f}x (Beats per frame)"
    )
    print(f"Pacing Consistency (Jitter):  {jitter:6.4f} ms -> [{jitter_label}]")
    print("-" * 65)
    print(f"Average Phase Drift:          {avg_drift:6.4f} ms")
    print(f"Maximum Phase Drift:          {max_drift:6.4f} ms")
    print(f"Drift Standard Deviation:     {drift_standard_deviation:6.4f} ms")
    print(f"Phase Alignment (Sync Score): {avg_sync:6.2f}%")
    print("-" * 65)

    if 0.95 <= performance_multiplier <= 1.05:
        if jitter < 0.5:
            print("VERDICT: NATIVE PERFORMANCE. GPU is perfectly tracking the monitor.")
        else:
            print(
                "VERDICT: NATIVE BUT JITTERY. Correct speed, but delivery spacing is uneven."
            )
    elif performance_multiplier > 2.0:
        print(
            "VERDICT: GPU BOUND. Throughput is significantly lower than refresh rate."
        )
    else:
        print("VERDICT: ACCEPTABLE. Standard presentation timing.")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python scriptname.py <filename.json>")
    else:
        analyze_frame_log(sys.argv[1])
