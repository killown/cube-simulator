> The benchmarks will be updated once the average maximum safe cube count is implemented in the cube simulator.

# Setup

| Field          | Value                                 |
| :------------- | :------------------------------------ |
| Display        | HDMI-A-1: 1920x1080 @ 165Hz (VRR: ON) |
| Frame Budget   | 6.0606ms (165.00Hz)                   |
| Surface Format | Bgra8UnormSrgb                        |
| CPU            | AMD Ryzen 5 5600X                     |
| GPU            | AMD Radeon RX 9060 XT                 |
| Driver         | Mesa 26.1.0-devel (git-1e1d8931c7)    |
| Memory         | 32 GiB                                |
| OS             | CachyOS x86_64                        |
| Kernel         | Linux 7.0.0-rc3-1-cachyos-rc          |

# Goal

These benchmarks measure frame presentation stability and consistency.

# Note

JAY and Sway with (max render time 10) consistently presents the same safe cube count and sometimes with very little variation of 1 or 2 cubes. Others can vary by up to 20 cubes, never exceeding 41. Most compositors, however, rarely reach 20 cubes in most attempts, some never exceed 30.

It doesn't measure the average maximum safe cube count, but it reveals that 99% of compositors have a big room to improve frame presentation stability when the GPU is under high load. 

# What are we measuring?

The benchmark finds how many cubes any compositor can render before it starts missing frame deadlines.

It runs the shader with 1 cube, waits a few seconds, then checks if any frame took longer than one monitor refresh cycle to appear on screen. If every frame landed on time, it adds another cube and repeats. If a frame missed its slot, meaning the GPU took too long and the compositor had to hold it until the next refresh, it stops and reports how many cubes the system could handle cleanly.

- **Yellow** — `vblank_mul > 1` on any single frame (amber ring fires). An isolated missed vblank slot.
- **Red** — `EMA(vblank_mul) > 1.15` (red diamond fires). Sustained compositor pressure, not just a spike.

The benchmark automatically sweeps cube counts from `1` up to `--bench-max`, holding each count for `--bench-secs` seconds. The first `--bench-warmup` seconds of every step are discarded so compositor startup jitter does not pollute the signal. The sweep stops and the window closes automatically when a pacing signal fires (Yellow or Red).

---

# Results

## Gnome

**FIFO**

```
✗ Trigger at     : 26 cubes
  Signal         : YELLOW — isolated vblank miss (amber ring)
  Measured for   : 0.2s before trigger

➜ Maximum safe cube count: 25
```

**Mailbox**

```
✗ Trigger at     : 31 cubes
  Signal         : YELLOW — isolated vblank miss (amber ring)
  Measured for   : 0.6s before trigger

➜ Maximum safe cube count: 30
```

---

## Jay

**FIFO**

```
✗ Trigger at     : 41 cubes
  Signal         : YELLOW — isolated vblank miss (amber ring)
  Measured for   : 1.0s before trigger

➜ Maximum safe cube count: 40
```

**Mailbox**

```
✗ Trigger at     : 42 cubes
  Signal         : RED — sustained EMA pressure (red diamond)
  Measured for   : 0.0s before trigger

➜ Maximum safe cube count: 41
```

---

## Plasma

**FIFO**

```
✗ Trigger at     : 40 cubes
  Signal         : RED — sustained EMA pressure (red diamond)
  Measured for   : 0.0s before trigger

➜ Maximum safe cube count: 39
```

**Mailbox**

```
✗ Trigger at     : 24 cubes
  Signal         : YELLOW — isolated vblank miss (amber ring)
  Measured for   : 0.5s before trigger

➜ Maximum safe cube count: 23
```

---

## Sway (max render time: 10, wlroots(await-completion))

**FIFO**

```
✗ Trigger at     : 39 cubes
  Signal         : RED — sustained EMA pressure (red diamond)
  Measured for   : 0.0s before trigger

➜ Maximum safe cube count: 38
```

**Mailbox**

```
✗ Trigger at     : 40 cubes
  Signal         : RED — sustained EMA pressure (red diamond)
  Measured for   : 0.0s before trigger

➜ Maximum safe cube count: 39
```

---

## Cosmic

**FIFO**

```
✗ Trigger at     : 1 cube
  Signal         : YELLOW — isolated vblank miss (amber ring)
  Measured for   : 0.1s before trigger

⚠ Compositor could not sustain even 1 cube without pressure.
```

**Mailbox**

```
✗ Trigger at     : 40 cubes
  Signal         : RED — sustained EMA pressure (red diamond)
  Measured for   : 0.0s before trigger

➜ Maximum safe cube count: 39
```

---

## Wayfire (Vulkan: max render time -1)

**FIFO**

```
✗ Trigger at     : 39 cubes
  Signal         : YELLOW — isolated vblank miss (amber ring)
  Measured for   : 0.4s before trigger

➜ Maximum safe cube count: 38
```

**Mailbox**

```
✗ Trigger at     : 14 cubes
  Signal         : RED — sustained EMA pressure (red diamond)
  Measured for   : 0.0s before trigger

➜ Maximum safe cube count: 13
```

---

## Wayfire (GLES2: Max render time -1)

**FIFO**

```
✗ Trigger at     : 34 cubes
  Signal         : YELLOW — isolated vblank miss (amber ring)
  Measured for   : 0.6s before trigger

➜ Maximum safe cube count: 33
```

**Mailbox**

```
✗ Trigger at     : 22 cubes
  Signal         : YELLOW — isolated vblank miss (amber ring)
  Measured for   : 0.3s before trigger

➜ Maximum safe cube count: 21
```

---

## Niri

**FIFO**

```
✗ Trigger at     : 31 cubes
  Signal         : YELLOW — isolated vblank miss (amber ring)
  Measured for   : 0.8s before trigger

➜ Maximum safe cube count: 30
```

**Mailbox**

```
✗ Trigger at     : 42 cubes
  Signal         : RED — sustained EMA pressure (red diamond)
  Measured for   : 0.0s before trigger

➜ Maximum safe cube count: 41
```
