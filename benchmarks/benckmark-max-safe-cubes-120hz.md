# Compositor Benchmark Results

**GPU:** AMD Radeon RX 9060 XT (RADV GFX1200)
**Display:** 1920x1080 @ 120Hz (VRR: On)
**Shader:** HIGH-END — `shader.wgsl` (Raymarched SDF)
**Frame budget:** 8.33ms (120Hz)

---

## Summary Table

| Compositor | Mode    | Safe Cubes | Trigger | Trigger At | Score | Grade |
| ---------- | ------- | ---------- | ------- | ---------- | ----- | ----- |
| Wayfire    | Mailbox | 30         | Yellow  | 31 cubes   | 4240  | A     |
| Wayfire    | Fifo    | 51         | Yellow  | 52 cubes   | 7180  | S     |
| Sway       | Mailbox | 51         | Yellow  | 52 cubes   | 7180  | S     |
| Sway       | Fifo    | 51         | Yellow  | 52 cubes   | 7180  | S     |
| Hyprland   | Mailbox | 30         | Yellow  | 31 cubes   | 4240  | A     |
| Hyprland   | Fifo    | 45         | Red     | 46 cubes   | 4960  | S     |

---

## Per-Compositor Breakdown

### Wayfire

| Mode    | Safe Cubes | Trigger     | Score | Grade |
| ------- | ---------- | ----------- | ----- | ----- |
| Mailbox | 30         | Yellow @ 31 | 4240  | A     |
| Fifo    | 51         | Yellow @ 52 | 7180  | S     |

- Fifo is significantly better: +21 safe cubes, +2940 points
- Mailbox hits a vblank miss at 31 cubes — early compared to Fifo
- Fifo held the longest clean run of all tested configurations

### Sway

| Mode    | Safe Cubes | Trigger     | Score | Grade |
| ------- | ---------- | ----------- | ----- | ----- |
| Mailbox | 51         | Yellow @ 52 | 7180  | S     |
| Fifo    | 51         | Yellow @ 52 | 7180  | S     |

- Both modes are identical in score and safe cube count
- Most consistent compositor: Mailbox and Fifo behave the same

### Hyprland

| Mode    | Safe Cubes | Trigger     | Score | Grade |
| ------- | ---------- | ----------- | ----- | ----- |
| Mailbox | 30         | Yellow @ 31 | 4240  | A     |
| Fifo    | 45         | Red @ 46    | 4960  | S     |

- Fifo holds longer (+15 cubes) but ends with a **Red** trigger (sustained EMA pressure), not a clean Yellow miss
- Red at 46 is a harder failure than Yellow at 52, Hyprland Fifo degrades under load rather than hitting a single transient miss

---

## Rankings

### By score (highest first)

| Rank | Compositor | Mode    | Score | Grade |
| ---- | ---------- | ------- | ----- | ----- |
| 1    | Wayfire    | Fifo    | 7180  | S     |
| 1    | Sway       | Mailbox | 7180  | S     |
| 1    | Sway       | Fifo    | 7180  | S     |
| 4    | Hyprland   | Fifo    | 4960  | S     |
| 5    | Wayfire    | Mailbox | 4240  | A     |
| 5    | Hyprland   | Mailbox | 4240  | A     |

### By safe cube count (highest first)

| Rank | Compositor | Mode    | Safe Cubes |
| ---- | ---------- | ------- | ---------- |
| 1    | Wayfire    | Fifo    | 51         |
| 1    | Sway       | Mailbox | 51         |
| 1    | Sway       | Fifo    | 51         |
| 4    | Hyprland   | Fifo    | 45         |
| 5    | Wayfire    | Mailbox | 30         |
| 5    | Hyprland   | Mailbox | 30         |

---

> Scores are comparable only between runs using the same shader (`shader.wgsl`),
> `--bench-secs`, and `--bench-max` values. All runs above used identical parameters.
