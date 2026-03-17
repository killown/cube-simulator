import json
import sys
import matplotlib.pyplot as plt

if len(sys.argv) < 2:
    print("Usage: python cadence_probe.py <logfile.json>")
    sys.exit(1)

log_file = sys.argv[1]
data = []

with open(log_file, "r") as f:
    for line in f:
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        try:
            data.append(json.loads(line))
        except json.JSONDecodeError:
            continue

frames = [e["frame"] for e in data]
sync = [e["sync"] for e in data]
delta = [e["delta_ms"] for e in data]
drift = [e["drift_ms"] for e in data]
vblank = [e["vblank_mul"] for e in data]
gpu = [e.get("gpu_time_ms", 0.0) for e in data]
cpu = [e.get("cpu_frame_ms", 0.0) for e in data]
slack = [e.get("slack_ms", 0.0) for e in data]
ipc = [e.get("ipc_delta_ms", 0.0) for e in data]
m_driver = [e.get("micro_driver_ms", 0.0) for e in data]
m_total = [e.get("micro_total_ms", 0.0) for e in data]

fig, axes = plt.subplots(9, 1, figsize=(16, 22), sharex=True, facecolor="#0a0a0a")
plt.subplots_adjust(hspace=0.4)


def plot_metric(ax, x, y, label, color, ylim=None):
    ax.set_facecolor("#111111")
    ax.plot(x, y, "-o", color=color, markersize=2, linewidth=1, alpha=0.8)
    ax.set_ylabel(label, color="white", fontsize=9, fontweight="bold")
    ax.tick_params(colors="gray", labelsize=8)
    ax.grid(True, alpha=0.1, color="white")
    if ylim:
        ax.set_ylim(ylim)


plot_metric(axes[0], frames, sync, "Sync Score", "#00ff88", ylim=(0, 105))
plot_metric(axes[1], frames, delta, "Delta (ms)", "#00ccff")
plot_metric(axes[2], frames, vblank, "VBlank Mul", "#ff0000", ylim=(0.5, 4.5))
plot_metric(axes[3], frames, drift, "Drift (ms)", "#ff00ff")
plot_metric(axes[4], frames, ipc, "IPC Jitter", "#ffffff")
plot_metric(axes[5], frames, cpu, "CPU (ms)", "#ffff00")
plot_metric(axes[6], frames, gpu, "GPU (ms)", "#ff4400")
plot_metric(axes[7], frames, m_driver, "Driver (ms)", "#888888")
plot_metric(axes[8], frames, slack, "Slack (ms)", "#00aa00")

axes[8].set_xlabel("Frame Index", color="white")
plt.suptitle(
    f"Telemetry Analysis: {log_file}",
    color="white",
    fontsize=16,
    y=0.92,
)
plt.show()
