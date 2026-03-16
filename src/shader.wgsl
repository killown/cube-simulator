// Cube simulator, original raymarcher with per-cube colour and unique inner shape.
// Inner shapes are resolved via a secondary march inside a confirmed cube hit,
// keeping the primary SDF clean and stable.

struct Uniforms {
    color:         vec4<f32>,
    cube_count:    u32,
    size:          f32,
    speed:         f32,
    steps:         u32,
    fps_data:      vec4<f32>,
    adv_data:      vec4<f32>,   // [jitter, dropped, ftv, _pad]
    time:          f32,
    stutter_decay: f32,         // 1.0 on dropped frame,         decays ~30 frames
    pacing_decay:  f32,         // EMA vblank_mul pressure,      decays ~45 frames
    gpu_time_ms:   f32,
    sync_score:    f32,
    cpu_time_ms:   f32,
    slack_ms:      f32,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) v_idx: u32) -> VertexOutput {
    var out: VertexOutput;
    let pos = array<vec2<f32>, 4>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0),
        vec2(-1.0,  1.0), vec2(1.0,  1.0),
    );
    out.clip_position = vec4<f32>(pos[v_idx], 0.0, 1.0);
    out.uv = pos[v_idx];
    return out;
}

fn rot(a: f32) -> mat2x2<f32> {
    let s = sin(a); let c = cos(a);
    return mat2x2<f32>(c, s, -s, c);
}

fn uhash(n: u32) -> u32 {
    var x = n;
    x ^= x << 13u; x *= 1664525u; x ^= x >> 17u; x *= 1013904223u; x ^= x << 5u;
    return x;
}
fn hash11(p: f32) -> f32 {
    return f32(uhash(bitcast<u32>(p))) / 4294967296.0;
}
fn hash21(p: vec2<f32>) -> f32 {
    return f32(uhash(bitcast<u32>(p.x * 127.1 + p.y * 311.7))) / 4294967296.0;
}

fn cube_color(cube_i: f32) -> vec3<f32> {
    let hue = fract(cube_i * 0.61803398875);
    let h6  = hue * 6.0;
    let hi  = i32(h6) % 6;
    let f   = fract(h6);
    let q   = 1.0 - f;
    var rgb: vec3<f32>;
    if      (hi == 0) { rgb = vec3(1.0, f,   0.0); }
    else if (hi == 1) { rgb = vec3(q,   1.0, 0.0); }
    else if (hi == 2) { rgb = vec3(0.0, 1.0, f  ); }
    else if (hi == 3) { rgb = vec3(0.0, q,   1.0); }
    else if (hi == 4) { rgb = vec3(f,   0.0, 1.0); }
    else              { rgb = vec3(1.0, 0.0, q  ); }
    return mix(rgb, vec3(0.7), 0.15) * (u.color.rgb * 2.0 + 0.5);
}

// ── Cube-local transform ──────────────────────────────────────────────────────
//
// Transforms a world-space point into the local frame of cube i.
// Used identically by the primary SDF and the secondary inner march.

fn cube_local(p: vec3<f32>, fi: f32, t: f32) -> vec3<f32> {
    let speed = u.speed;
    let offset = vec3(
        sin(t * 0.5 * speed + fi * 1.047) * 3.5,
        cos(t * 0.7 * speed + fi * 0.800) * 2.0,
        sin(t * 0.3 * speed + fi * 2.100) * 1.5,
    );
    var q = p - offset;
    let q_xz = rot(t * speed * (0.2  + fi * 0.10)) * q.xz; q.x = q_xz.x; q.z = q_xz.y;
    let q_yz = rot(t * speed * (0.15 + fi * 0.05)) * q.yz; q.y = q_yz.x; q.z = q_yz.y;
    return q;
}

// ── Primary scene SDF, outer shell only, no inner shapes ────────────────────

struct SceneHit { d: f32, cube_i: f32 }

fn map_full(p: vec3<f32>, t: f32) -> SceneHit {
    var best = SceneHit(1e10, 0.0);
    for (var i = 0u; i < u.cube_count; i++) {
        let fi = f32(i);
        let q  = cube_local(p, fi, t);
        let a      = abs(q);
        let cube   = max(a.x, max(a.y, a.z)) - u.size;
        let sphere = length(q) - (u.size * 1.4);
        let d      = max(-sphere, cube);
        if (d < best.d) { best = SceneHit(d, fi); }
    }
    return best;
}

fn map(p: vec3<f32>, t: f32) -> f32 { return map_full(p, t).d; }

// ── Inner shape SDFs (evaluated in cube-local space) ─────────────────────────

fn sd_box_inner(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}
fn sd_torus(p: vec3<f32>, R: f32, r: f32) -> f32 {
    let q = vec2(length(p.xz) - R, p.y);
    return length(q) - r;
}
fn sd_capsule(p: vec3<f32>, h: f32, r: f32) -> f32 {
    let qy = clamp(p.y, -h, h);
    return length(p - vec3(0.0, qy, 0.0)) - r;
}
fn sd_octahedron(p: vec3<f32>, s: f32) -> f32 {
    let q = abs(p);
    return (q.x + q.y + q.z - s) * 0.57735027;
}
fn sd_cross(p: vec3<f32>, arm: f32, thick: f32) -> f32 {
    let q  = abs(p);
    let da = max(q.y, q.z) - thick;
    let db = max(q.x, q.z) - thick;
    let dc = max(q.x, q.y) - thick;
    return min(min(
        max(da, q.x - arm),
        max(db, q.y - arm)),
        max(dc, q.z - arm));
}

/// Returns the SDF of the inner symbol for cube `cube_i` at local position `q`.
/// The symbol spins on its own axis independently from the outer cube.
fn inner_sdf(q_cube: vec3<f32>, cube_i: f32, t: f32) -> f32 {
    let h0 = hash11(cube_i * 3.71 + 1.13);
    let h1 = hash11(cube_i * 5.37 + 2.91);
    let h2 = hash11(cube_i * 7.13 + 0.57);

    // Independent inner spin.
    var q = q_cube;
    let spin = 0.5 + h2 * 0.9;
    let qxz = rot(t * spin * (0.50 + h0 * 0.4)) * q.xz; q.x = qxz.x; q.z = qxz.y;
    let qyz = rot(t * spin * (0.35 + h1 * 0.3)) * q.yz; q.y = qyz.x; q.z = qyz.y;

    let s    = u.size * 0.38;
    let kind = i32(h0 * 5.0);   // 5 distinct shapes
    if      (kind == 0) { return length(q) - s; }                          // sphere
    else if (kind == 1) { return sd_box_inner(q, vec3(s * 0.65)); }        // cube
    else if (kind == 2) { return sd_torus(q, s * 0.52, s * 0.22); }       // torus
    else if (kind == 3) { return sd_capsule(q, s * 0.50, s * 0.25); }     // capsule
    else if (kind == 4) { return sd_octahedron(q, s * 1.05); }            // octahedron
    else                { return sd_cross(q, s * 0.70, s * 0.22); }       // cross
}

// ── Secondary march, runs only inside a confirmed cube hit ──────────────────
//
// The ray entry/exit along the cube axis-aligned extent gives us a tight
// interval [t_enter, t_exit] so the march is bounded and cheap.

struct InnerHit { hit: bool, t: f32 }

fn march_inner(ro: vec3<f32>, rd: vec3<f32>, cube_i: f32, t_scene: f32, time: f32) -> InnerHit {
    // Recompute cube-local entry point and bound the march to the cube interior.
    let fi     = cube_i;
    let speed  = u.speed;
    let offset = vec3(
        sin(time * 0.5 * speed + fi * 1.047) * 3.5,
        cos(time * 0.7 * speed + fi * 0.800) * 2.0,
        sin(time * 0.3 * speed + fi * 2.100) * 1.5,
    );

    // Walk back along the ray to find where we entered the cube's bounding
    // sphere, gives a safe t_start before the confirmed surface.
    let oc       = ro - offset;
    let R        = u.size * 1.42;
    let b_coef   = dot(oc, rd);
    let c_coef   = dot(oc, oc) - R * R;
    let disc     = b_coef * b_coef - c_coef;
    if (disc < 0.0) { return InnerHit(false, 0.0); }
    let t_enter  = max(-b_coef - sqrt(disc), 0.0);
    let t_exit   = t_scene;   // stop at the outer surface we already hit

    var tm = t_enter;
    for (var i = 0; i < 24; i++) {
        if (tm >= t_exit) { break; }
        let wp = ro + rd * tm;
        let lp = cube_local(wp, fi, time);
        let d  = inner_sdf(lp, fi, time);
        if (d < 0.002) { return InnerHit(true, tm); }
        tm += max(d, 0.002);
    }
    return InnerHit(false, 0.0);
}

// ── OSD ───────────────────────────────────────────────────────────────────────
//
// Font: 3×5 pixel bitmap. Each glyph is a 15-bit mask stored as an i32.
// Bit layout (row-major, bit 14 = top-left):
//   col:  0  1  2
//   row0: 14 13 12
//   row1: 11 10  9
//   row2:  8  7  6
//   row3:  5  4  3
//   row4:  2  1  0
//
// Number layout (left-aligned, x=0 is the most-significant digit):
//   [d0][gap][d1][gap][d2][gap][d3][DOT_PAD][.][DOT_PAD][f0][gap][f1]
//
// This means numbers start immediately after the label separator with no
// fixed-width padding, so "1" and "120" both begin at the same column.

const CHAR_W: f32     = 3.0;  // glyph cell width
const CHAR_H: f32     = 5.0;  // glyph cell height
const CHAR_GAP: f32   = 2.0;  // gap between glyphs
const ROW_STRIDE: f32 = 7.0;  // vertical row pitch
// X where the number field starts (3 label chars + 1 separator gap).
const NUM_X_OFF: f32  = (CHAR_W + CHAR_GAP) * 3.3 + CHAR_GAP;
const OSD_SCALE: f32  = 4.0;  // screen pixels per logical OSD pixel
// Symmetric padding around the decimal point.
const DOT_PAD: f32    = 3.0;

fn sd_char(uv: vec2<f32>, bits: i32) -> f32 {
    if (uv.x < 0.0 || uv.x >= CHAR_W || uv.y < 0.0 || uv.y >= CHAR_H) { return 0.0; }
    let bit_idx = u32((4 - i32(uv.y)) * 3 + i32(uv.x));
    if ((bits & (1 << bit_idx)) != 0) {
        let d = length(fract(uv) - vec2(0.5)) - 0.4;
        if (d < 0.0) { return 1.0; }
    }
    return 0.0;
}

/// Renders a float left-aligned at `origin`.
///
/// Digits are written left-to-right: most-significant integer digit at x=0,
/// then the decimal point (with DOT_PAD clearance on both sides), then
/// fractional digits. `decimals` is 0, 1, or 2.
fn number(val: f32, decimals: i32, origin: vec2<f32>, frag: vec2<f32>) -> f32 {
    let digits = array<i32, 10>(
        31599, 9879, 31183, 31207, 23524, 29671, 29679, 30994, 31727, 31719
    );
    const MINUS: i32 = 128;
    const DOT: i32   = 2;
    const STEP: f32  = CHAR_W + CHAR_GAP;  // one digit column width

    let b  = frag - origin;
    let av = abs(val);
    let ival = i32(av);
    var out = 0.0;

    // Count integer digits to know how many columns the integer part occupies.
    var int_digits: i32;
    if      (ival >= 1000) { int_digits = 4; }
    else if (ival >= 100)  { int_digits = 3; }
    else if (ival >= 10)   { int_digits = 2; }
    else                   { int_digits = 1; }

    // Render integer digits left-to-right starting at x=0.
    if (int_digits >= 4) {
        out = max(out, sd_char(b - vec2(STEP * 0.0, 0.0), digits[(ival / 1000) % 10]));
    }
    if (int_digits >= 3) {
        let col = f32(int_digits - 3);
        out = max(out, sd_char(b - vec2(STEP * col, 0.0), digits[(ival / 100) % 10]));
    }
    if (int_digits >= 2) {
        let col = f32(int_digits - 2);
        out = max(out, sd_char(b - vec2(STEP * col, 0.0), digits[(ival / 10) % 10]));
    }
    {
        let col = f32(int_digits - 1);
        out = max(out, sd_char(b - vec2(STEP * col, 0.0), digits[ival % 10]));
    }

    // Minus sign one column left of the first digit.
    if (val < 0.0) {
        out = max(out, sd_char(b + vec2(STEP, 0.0), MINUS));
    }

    if (decimals <= 0) { return out; }

    // Decimal point: DOT_PAD after the last integer digit, DOT_PAD before frac.
    let dot_x = STEP * f32(int_digits) - CHAR_GAP + DOT_PAD;
    out = max(out, sd_char(b - vec2(dot_x, 0.0), DOT));

    // Fractional digits start DOT_PAD after the dot centre.
    let frac_x = dot_x + DOT_PAD;
    if (decimals >= 1) {
        out = max(out, sd_char(b - vec2(frac_x, 0.0),
                               digits[i32(av * 10.0) % 10]));
    }
    if (decimals >= 2) {
        out = max(out, sd_char(b - vec2(frac_x + STEP, 0.0),
                               digits[i32(av * 100.0) % 10]));
    }

    return out;
}

/// Renders one OSD row: a 3-char label then a float value, all at row `row`.
fn osd_row(row: f32, c0: i32, c1: i32, c2: i32, val: f32, dec: i32, frag: vec2<f32>) -> f32 {
    const STEP: f32 = CHAR_W + CHAR_GAP;
    let oy  = row * ROW_STRIDE;
    let b   = frag - vec2(0.0, oy);
    var out = 0.0;
    out = max(out, sd_char(b,                    c0));
    out = max(out, sd_char(b - vec2(STEP, 0.0),  c1));
    out = max(out, sd_char(b - vec2(STEP*2.0, 0.0), c2));
    out = max(out, number(val, dec, vec2(NUM_X_OFF, oy), frag));
    return out;
}

fn osd_mask(frag: vec2<f32>) -> f32 {
    var d = 0.0;
    d = max(d, osd_row(0.0, 29385, 31689, 29671, u.fps_data.x,  0, frag)); // FPS
    d = max(d, osd_row(1.0, 24429, 29847, 24557, u.fps_data.y,  0, frag)); // MIN
    d = max(d, osd_row(2.0, 24429, 11245, 23213, u.fps_data.z,  0, frag)); // MAX
    d = max(d, osd_row(3.0,  4687, 31599, 23418, u.fps_data.w,  0, frag)); // LOW
    d = max(d, osd_row(4.0, 26926, 29847, 29842, u.adv_data.x,  2, frag)); // JIT
    d = max(d, osd_row(5.0, 24429, 29671, 15211, u.adv_data.y,  0, frag)); // MSD
    d = max(d, osd_row(6.0, 29385, 29842, 23378, u.adv_data.z,  1, frag)); // FTV
    d = max(d, osd_row(7.0, 29263, 31689, 23407, u.cpu_time_ms, 2, frag)); // CPU
    d = max(d, osd_row(8.0, 29551, 31689, 23407, u.gpu_time_ms, 2, frag)); // GPU
    d = max(d, osd_row(9.0, 29671, 23506, 24557, u.sync_score,  1, frag)); // SYN
    d = max(d, osd_row(10.0, 29671, 4687, 11245, u.slack_ms,    2, frag)); // SLA
    return step(0.5, d);
}

// ── Stutter / pacing markers ─────────────────────────────────────────────────
//
// RED  (stutter_decay):  filled diamond + all-edge border.
//   Fires when EMA(vblank_mul) > 1.15, a sustained bad delivery regime.
//   Lingers ~45 frames so a run of dropped scanouts leaves a long red ghost.
//
// YELLOW (pacing_decay): hollow ring + left/right edge bars only.
//   Fires on any single vblank_mul > 1, a momentary ping, not an alarm.
//   Fades quickly (~30 frames). Visually distinct from red: no fill, different
//   edges, so screen recordings can tell the two apart at a glance.
//   When both fire simultaneously they add to orange-white ("worst case").
//
// UV convention: in.uv is in [-1,1] clip space (x right, y up).

fn stutter_marker(uv: vec2<f32>, decay: f32) -> vec3<f32> {
    if (decay <= 0.0) { return vec3(0.0); }

    let corner  = vec2(0.82, 0.88);
    let delta   = abs(uv - corner);
    // Rotated-square (L∞ in rotated frame) for a diamond silhouette.
    let rot45   = vec2(delta.x + delta.y, abs(delta.x - delta.y)) * 0.7071;
    let radius  = 0.055 * decay;
    let diamond = step(rot45.x, radius) * step(rot45.y, radius);

    // All-edge border: thin pulse on all four sides.
    let border_w  = 0.012;
    let edge_dist = min(1.0 - abs(uv.x), 1.0 - abs(uv.y));
    let on_edge   = step(edge_dist, border_w) * sqrt(decay) * 0.7;

    return vec3(1.0, 0.08, 0.04) * max(diamond, on_edge);   // saturated red
}

fn pacing_marker(uv: vec2<f32>, decay: f32) -> vec3<f32> {
    if (decay <= 0.0) { return vec3(0.0); }

    // Hollow ring, visually distinct from the filled red diamond.
    let corner  = vec2(0.82, 0.88);
    let dist    = length(uv - corner);
    let outer_r = 0.055;
    // Ring thins as decay falls, disappearing smoothly rather than snapping off.
    let inner_r = outer_r - 0.018 * decay;
    let ring    = step(inner_r, dist) * step(dist, outer_r) * decay;

    // Left and right edge bars only, horizontal complement to red's all-edge border.
    // Using opposite axis makes the two trivially distinguishable on video captures.
    let border_w = 0.010;
    let lr_edge  = step(1.0 - border_w, abs(uv.x)) * sqrt(decay) * 0.65;

    return vec3(1.0, 0.82, 0.0) * max(ring, lr_edge);   // amber-yellow
}

// ── Main ─────────────────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t  = u.time;
    let uv = in.uv * vec2(1.77, 1.0);
    let ro = vec3(0.0, 0.0, 10.0);
    let rd = normalize(vec3(uv, -1.8));

    // Primary march, outer shells only.
    var total  = 0.0;
    var min_d  = 1e10;
    var hit_t  = -1.0;
    var hit_sh = SceneHit(1e10, 0.0);
    for (var i = 0u; i < u.steps; i++) {
        let sh = map_full(ro + rd * total, t);
        if (sh.d < min_d) { min_d = sh.d; hit_t = total; hit_sh = sh; }
        total += max(sh.d, 0.001);
        if (total > 30.0) { break; }
    }

    let hit   = min_d < 0.002;
    let p     = ro + rd * hit_t;
    let grain = hash21(in.uv + vec2(fract(t * 1.618), fract(t * 2.618))) * 2.0 - 1.0;

    var color: vec3<f32>;
    if (!hit) {
        color = mix(vec3(0.01, 0.02, 0.05), vec3(0.05, 0.08, 0.15), in.uv.y * 0.5 + 0.5)
              + grain * 0.04;
    } else {
        let eps = max(u.size * 0.01, 0.001);
        let k   = vec2(1.0, -1.0);
        let n   = normalize(
            k.xyy * map(p + k.xyy * eps, t) +
            k.yyx * map(p + k.yyx * eps, t) +
            k.yxy * map(p + k.yxy * eps, t) +
            k.xxx * map(p + k.xxx * eps, t),
        );
        let light    = max(dot(n, normalize(vec3(1.0, 2.0, 1.0))), 0.2);
        let base_col = cube_color(hit_sh.cube_i);

        // Secondary march inside this cube looking for the inner symbol.
        let inner = march_inner(ro, rd, hit_sh.cube_i, hit_t, t);
        if (inner.hit) {
            // Shade the inner shape surface.
            let ip  = ro + rd * inner.t;
            let ilp = cube_local(ip, hit_sh.cube_i, t);
            let ie  = eps * 0.5;
            let in_n = normalize(vec3(
                inner_sdf(ilp + vec3( ie, 0.0, 0.0), hit_sh.cube_i, t) -
                inner_sdf(ilp - vec3( ie, 0.0, 0.0), hit_sh.cube_i, t),
                inner_sdf(ilp + vec3(0.0,  ie, 0.0), hit_sh.cube_i, t) -
                inner_sdf(ilp - vec3(0.0,  ie, 0.0), hit_sh.cube_i, t),
                inner_sdf(ilp + vec3(0.0, 0.0,  ie), hit_sh.cube_i, t) -
                inner_sdf(ilp - vec3(0.0, 0.0,  ie), hit_sh.cube_i, t),
            ));
            let i_light = max(dot(in_n, normalize(vec3(1.0, 2.0, 1.0))), 0.25);
            // Complementary hue: half a golden-ratio step away.
            let comp    = cube_color(hit_sh.cube_i + 3.5);
            color = comp * i_light * 1.3 + grain * 0.02;
        } else {
            color = base_col * light + grain * 0.03;
        }
    }

    let osd = osd_mask((in.clip_position.xy - vec2(8.0, 8.0)) / OSD_SCALE);
    let osd_col = mix(color, vec3(0.0, 1.0, 0.5), osd);
    // Additive composite: red drop flash + yellow sustained-pressure flash.
    // Both clamped to [0,1] so they never blow out HDR surfaces.
    let stutter = stutter_marker(in.uv, u.stutter_decay);
    let pacing  = pacing_marker(in.uv, u.pacing_decay);
    return vec4(clamp(osd_col + stutter + pacing, vec3(0.0), vec3(1.0)), 1.0);
}
