// shader_low.wgsl
// Low-end rasterised variant.  Identical Uniforms layout and OSD/marker
// subsystems as shader.wgsl.  The per-pixel SDF raymarching loop is replaced
// by an analytic ray-vs-OBB intersection: the ray is transformed into each
// cube's local frame and tested against an axis-aligned box there, which is
// a closed-form O(1) operation per cube with no iterative march.
//
// Visual output: geometrically correct rotating coloured cubes with flat
// face shading, a depth-sorted front-face normal tint, and the full OSD +
// stutter/pacing marker overlay.

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

// ── Shared helpers (identical signatures to shader.wgsl) ────────────────────

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

// ── Cube-local transform (exact copy from shader.wgsl) ───────────────────────

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

// ── Analytic ray-vs-OBB intersection ─────────────────────────────────────────
//
// Strategy: transform both ray origin and direction into cube-local space
// (where the cube is an AABB [-size, +size]^3), then apply the standard
// slab test.  Returns the entry t along the ray, or -1.0 on miss.
//
// This replaces the entire SDF march: one closed-form solve per cube, O(1),
// no loop, no step accumulation.

struct ObbHit {
    /// Entry distance along the ray; negative means no hit.
    t:      f32,
    /// Local-space hit point (used to derive the face normal).
    lp:     vec3<f32>,
}

fn ray_obb(ro: vec3<f32>, rd: vec3<f32>, fi: f32, t_scene: f32) -> ObbHit {
    // Transform the ray into cube-local space by applying cube_local() to both
    // a point on the ray and to the origin.  Because cube_local is an affine
    // rigid transform (translate + two planar rotations) we can reconstruct the
    // local direction as the difference of two transformed points.
    let lo = cube_local(ro, fi, t_scene);
    // A point one unit along the ray; subtract lo to get the local direction.
    let ld = cube_local(ro + rd, fi, t_scene) - lo;

    let s = u.size;
    // Slab test: compute per-axis entry/exit distances.
    // Guard against division by near-zero with a small epsilon.
    let inv = vec3(
        select(1.0 / ld.x, 1e9, abs(ld.x) < 1e-7),
        select(1.0 / ld.y, 1e9, abs(ld.y) < 1e-7),
        select(1.0 / ld.z, 1e9, abs(ld.z) < 1e-7),
    );
    let t0 = (-vec3(s) - lo) * inv;
    let t1 = ( vec3(s) - lo) * inv;
    let tmin3 = min(t0, t1);
    let tmax3 = max(t0, t1);
    let tenter = max(tmin3.x, max(tmin3.y, tmin3.z));
    let texit  = min(tmax3.x, min(tmax3.y, tmax3.z));

    if (texit < 0.0 || tenter > texit) {
        return ObbHit(-1.0, vec3(0.0));
    }
    let lp = lo + ld * tenter;
    return ObbHit(tenter, lp);
}

// Derives the outward face normal from a local-space hit point.
// The dominant axis of lp (clamped to [-size,+size]^3) identifies the face.
fn obb_normal(lp: vec3<f32>) -> vec3<f32> {
    let a = abs(lp);
    if (a.x >= a.y && a.x >= a.z) { return vec3(sign(lp.x), 0.0, 0.0); }
    if (a.y >= a.x && a.y >= a.z) { return vec3(0.0, sign(lp.y), 0.0); }
    return vec3(0.0, 0.0, sign(lp.z));
}

// ── Scene traversal ──────────────────────────────────────────────────────────

struct SceneResult {
    hit:    bool,
    t:      f32,
    cube_i: f32,
    lp:     vec3<f32>,
}

fn intersect_scene(ro: vec3<f32>, rd: vec3<f32>, t: f32) -> SceneResult {
    var best_t  = 1e10;
    var best_i  = 0.0;
    var best_lp = vec3(0.0);
    var any_hit = false;

    for (var i = 0u; i < u.cube_count; i++) {
        let fi  = f32(i);
        let hit = ray_obb(ro, rd, fi, t);
        if (hit.t > 0.0 && hit.t < best_t) {
            best_t  = hit.t;
            best_i  = fi;
            best_lp = hit.lp;
            any_hit = true;
        }
    }
    return SceneResult(any_hit, best_t, best_i, best_lp);
}

// ── OSD (identical to shader.wgsl) ───────────────────────────────────────────

const CHAR_W: f32     = 3.0;
const CHAR_H: f32     = 5.0;
const CHAR_GAP: f32   = 2.0;
const ROW_STRIDE: f32 = 7.0;
const NUM_X_OFF: f32  = (CHAR_W + CHAR_GAP) * 3.3 + CHAR_GAP;
const OSD_SCALE: f32  = 4.0;
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

fn number(val: f32, decimals: i32, origin: vec2<f32>, frag: vec2<f32>) -> f32 {
    let digits = array<i32, 10>(
        31599, 9879, 31183, 31207, 23524, 29671, 29679, 30994, 31727, 31719
    );
    const MINUS: i32 = 128;
    const DOT: i32   = 2;
    const STEP: f32  = CHAR_W + CHAR_GAP;

    let b  = frag - origin;
    let av = abs(val);
    let ival = i32(av);
    var out = 0.0;

    var int_digits: i32;
    if      (ival >= 1000) { int_digits = 4; }
    else if (ival >= 100)  { int_digits = 3; }
    else if (ival >= 10)   { int_digits = 2; }
    else                   { int_digits = 1; }

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
    if (val < 0.0) {
        out = max(out, sd_char(b + vec2(STEP, 0.0), MINUS));
    }
    if (decimals <= 0) { return out; }

    let dot_x = STEP * f32(int_digits) - CHAR_GAP + DOT_PAD;
    out = max(out, sd_char(b - vec2(dot_x, 0.0), DOT));
    let frac_x = dot_x + DOT_PAD;
    if (decimals >= 1) {
        out = max(out, sd_char(b - vec2(frac_x, 0.0),        digits[i32(av * 10.0)  % 10]));
    }
    if (decimals >= 2) {
        out = max(out, sd_char(b - vec2(frac_x + STEP, 0.0), digits[i32(av * 100.0) % 10]));
    }
    return out;
}

fn osd_row(row: f32, c0: i32, c1: i32, c2: i32, val: f32, dec: i32, frag: vec2<f32>) -> f32 {
    const STEP: f32 = CHAR_W + CHAR_GAP;
    let oy  = row * ROW_STRIDE;
    let b   = frag - vec2(0.0, oy);
    var out = 0.0;
    out = max(out, sd_char(b,                       c0));
    out = max(out, sd_char(b - vec2(STEP,     0.0), c1));
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
    return step(0.5, d);
}

// ── Stutter / pacing markers (identical to shader.wgsl) ─────────────────────

fn stutter_marker(uv: vec2<f32>, decay: f32) -> vec3<f32> {
    if (decay <= 0.0) { return vec3(0.0); }
    let corner  = vec2(0.82, 0.88);
    let delta   = abs(uv - corner);
    let rot45   = vec2(delta.x + delta.y, abs(delta.x - delta.y)) * 0.7071;
    let radius  = 0.055 * decay;
    let diamond = step(rot45.x, radius) * step(rot45.y, radius);
    let border_w  = 0.012;
    let edge_dist = min(1.0 - abs(uv.x), 1.0 - abs(uv.y));
    let on_edge   = step(edge_dist, border_w) * sqrt(decay) * 0.7;
    return vec3(1.0, 0.08, 0.04) * max(diamond, on_edge);
}

fn pacing_marker(uv: vec2<f32>, decay: f32) -> vec3<f32> {
    if (decay <= 0.0) { return vec3(0.0); }
    let corner  = vec2(0.82, 0.88);
    let dist    = length(uv - corner);
    let outer_r = 0.055;
    let inner_r = outer_r - 0.018 * decay;
    let ring    = step(inner_r, dist) * step(dist, outer_r) * decay;
    let border_w = 0.010;
    let lr_edge  = step(1.0 - border_w, abs(uv.x)) * sqrt(decay) * 0.65;
    return vec3(1.0, 0.82, 0.0) * max(ring, lr_edge);
}

// ── Main ─────────────────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t  = u.time;
    // Exact same camera as shader.wgsl.
    let uv = in.uv * vec2(1.77, 1.0);
    let ro = vec3(0.0, 0.0, 10.0);
    let rd = normalize(vec3(uv, -1.8));

    let result = intersect_scene(ro, rd, t);

    let grain = hash21(in.uv + vec2(fract(t * 1.618), fract(t * 2.618))) * 2.0 - 1.0;

    var color: vec3<f32>;
    if (!result.hit) {
        color = mix(vec3(0.01, 0.02, 0.05), vec3(0.05, 0.08, 0.15), in.uv.y * 0.5 + 0.5)
              + grain * 0.04;
    } else {
        // Derive world-space normal from local-space dominant face, then light it.
        let ln        = obb_normal(result.lp);
        // Rotate the local normal back to world space using the same two-axis
        // rotation so it points correctly for the key-light dot product.
        var wn        = ln;
        let fi        = result.cube_i;
        let rx_a      = t * u.speed * (0.2  + fi * 0.10);
        let ry_a      = t * u.speed * (0.15 + fi * 0.05);
        // Inverse (transpose) of rot() to go local → world.
        let rx_inv    = rot(-rx_a);
        let ry_inv    = rot(-ry_a);
        let wn_yz     = ry_inv * wn.yz; wn.y = wn_yz.x; wn.z = wn_yz.y;
        let wn_xz     = rx_inv * wn.xz; wn.x = wn_xz.x; wn.z = wn_xz.y;

        let light     = max(dot(wn, normalize(vec3(1.0, 2.0, 1.0))), 0.2);
        let base_col  = cube_color(fi);
        color = base_col * light + grain * 0.03;
    }

    let osd     = osd_mask((in.clip_position.xy - vec2(8.0, 8.0)) / OSD_SCALE);
    let osd_col = mix(color, vec3(0.0, 1.0, 0.5), osd);
    let stutter = stutter_marker(in.uv, u.stutter_decay);
    let pacing  = pacing_marker(in.uv, u.pacing_decay);
    return vec4(clamp(osd_col + stutter + pacing, vec3(0.0), vec3(1.0)), 1.0);
}
