# Compositor Benchmark Results


    GPU: AMD Radeon RX 9060 XT (RADV GFX1200)
    Display: 1920x1080 @ 120Hz (VRR: On)
    Shader: HIGH-END `shader.wgsl` (Raymarched SDF)
    Frame budget: 8.33ms (120Hz)

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

### Sway

| Mode    | Safe Cubes | Trigger     | Score | Grade |
| ------- | ---------- | ----------- | ----- | ----- |
| Mailbox | 51         | Yellow @ 52 | 7180  | S     |
| Fifo    | 51         | Yellow @ 52 | 7180  | S     |

---

### Hyprland

| Mode    | Safe Cubes | Trigger     | Score | Grade |
| ------- | ---------- | ----------- | ----- | ----- |
| Mailbox | 30         | Yellow @ 31 | 4240  | A     |
| Fifo    | 45         | Red @ 46    | 4960  | S     |

---

> Scores are comparable only between runs using the same shader (`shader.wgsl`),
> `--bench-secs`, and `--bench-max` values. All runs above used identical parameters.
