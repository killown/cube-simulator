struct Uniforms {
    color: vec4<f32>,
    cube_count: u32,
    size: f32,
    speed: f32,
    steps: u32,
    fps_data: vec4<f32>,
    adv_data: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) time: f32,
};

@vertex
fn vs_main(@builtin(vertex_index) v_idx: u32, @builtin(instance_index) i_idx: u32) -> VertexOutput {
    var out: VertexOutput;
    let pos = array<vec2<f32>, 4>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0), vec2(1.0, 1.0));
    out.clip_position = vec4<f32>(pos[v_idx], 0.0, 1.0);
    out.uv = pos[v_idx];
    out.time = f32(i_idx) * 0.001;
    return out;
}

fn rot(a: f32) -> mat2x2<f32> {
    let s = sin(a); let c = cos(a);
    return mat2x2<f32>(c, s, -s, c);
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

fn sd_char(uv: vec2<f32>, bits: i32) -> f32 {
    if (uv.x < 0.0 || uv.x >= 3.0 || uv.y < 0.0 || uv.y >= 5.0) { return 0.0; }
    let ix = i32(uv.x);
    let iy = i32(uv.y);
    let bit_idx = u32((4 - iy) * 3 + ix);
    if ((bits & (1 << bit_idx)) != 0) {
        let local_uv = fract(uv) - 0.5;
        let d = max(abs(local_uv.x), abs(local_uv.y)) - 0.4;
        if (d < 0.0) { return 1.0; }
    }
    return 0.0;
}

fn draw_num(uv: vec2<f32>, val: i32) -> f32 {
    let digits = array<i32, 10>(31599, 9879, 31183, 31207, 23524, 29671, 29679, 30994, 31727, 31719);
    let h = (val / 100) % 10;
    let t = (val / 10) % 10;
    let u_val = val % 10;

    var d = sd_char(uv - vec2(8.0, 0.0), digits[u_val]);
    if (val >= 10) {
        d = max(d, sd_char(uv - vec2(4.0, 0.0), digits[t]));
    }
    if (val >= 100) {
        d = max(d, sd_char(uv, digits[h]));
    }
    return d;
}

fn map(p: vec3<f32>, t: f32) -> f32 {
    var d = 1e10;
    let speed = u.speed;
    for(var i = 0u; i < u.cube_count; i++) {
        let fi = f32(i);
        let offset = vec3(
            sin(t * 0.5 * speed + fi * 1.047) * 3.5,
            cos(t * 0.7 * speed + fi * 0.8) * 2.0,
            sin(t * 0.3 * speed + fi * 2.1) * 1.5
        );
        var q = p - offset;
        let r1 = rot(t * speed * (0.2 + fi * 0.1));
        let r2 = rot(t * speed * (0.15 + fi * 0.05));
        let q_xz = r1 * q.xz; q.x = q_xz.x; q.z = q_xz.y;
        let q_yz = r2 * q.yz; q.y = q_yz.x; q.z = q_yz.y;
        let a = abs(q);
        let cube = max(a.x, max(a.y, a.z)) - u.size;
        let sphere = length(q) - (u.size * 1.4);
        d = min(d, max(-sphere, cube));
    }
    return d;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = in.time;
    let uv = in.uv * vec2(1.77, 1.0);
    var ro = vec3(0.0, 0.0, 10.0);
    var rd = normalize(vec3(uv, -1.8));

    var total = 0.0; var hit = false; var p: vec3<f32>;
    for(var i=0u; i<u.steps; i++) {
        p = ro + rd * total;
        let d = map(p, t);
        if d < 0.002 { hit = true; break; }
        total += d; if total > 30.0 { break; }
    }

    var color: vec3<f32>;
    let grain = hash(in.uv + fract(t));
    if !hit {
        color = mix(vec3(0.01, 0.02, 0.05), vec3(0.05, 0.08, 0.15), in.uv.y * 0.5 + 0.5) + grain * 0.04;
    } else {
        let eps = 0.005;
        let k = vec2(1.0, -1.0);
        let n = normalize(
            k.xyy * map(p + k.xyy * eps, t) +
            k.yyx * map(p + k.yyx * eps, t) +
            k.yxy * map(p + k.yxy * eps, t) +
            k.xxx * map(p + k.xxx * eps, t)
        );
        let light = max(dot(n, normalize(vec3(1.0, 2.0, 1.0))), 0.2);
        color = u.color.rgb * light + grain * 0.03;
    }

    let scale = 110.0;
    let base_uv = vec2((in.uv.x - (-0.98)) * scale, (0.98 - in.uv.y) * scale);

    // Row 0: FPS  (F=29385, P=31689, S=29671)
    var d = max(sd_char(base_uv, 29385), max(sd_char(base_uv - vec2(4.0, 0.0), 31689), sd_char(base_uv - vec2(8.0, 0.0), 29671)));
    d = max(d, draw_num(base_uv - vec2(14.0, 0.0), i32(u.fps_data.x)));

    // Row 1: MIN  (M=24429, I=29847, N=24557)
    let r1 = base_uv - vec2(0.0, 6.0);
    d = max(d, max(sd_char(r1, 24429), max(sd_char(r1 - vec2(4.0, 0.0), 29847), sd_char(r1 - vec2(8.0, 0.0), 24557))));
    d = max(d, draw_num(r1 - vec2(14.0, 0.0), i32(u.fps_data.y)));

    // Row 2: MAX  (M=24429, A=11245, X=23213)
    let r2 = base_uv - vec2(0.0, 12.0);
    d = max(d, max(sd_char(r2, 24429), max(sd_char(r2 - vec2(4.0, 0.0), 11245), sd_char(r2 - vec2(8.0, 0.0), 23213))));
    d = max(d, draw_num(r2 - vec2(14.0, 0.0), i32(u.fps_data.z)));

    // Row 3: LOW  (L=4687, O=31599, W=23418)
    let r3 = base_uv - vec2(0.0, 18.0);
    d = max(d, max(sd_char(r3, 4687), max(sd_char(r3 - vec2(4.0, 0.0), 31599), sd_char(r3 - vec2(8.0, 0.0), 23418))));
    d = max(d, draw_num(r3 - vec2(14.0, 0.0), i32(u.fps_data.w)));

    // Row 4: JIT  (J=26926, I=29847, T=29842)
    let r4 = base_uv - vec2(0.0, 24.0);
    d = max(d, max(sd_char(r4, 26926), max(sd_char(r4 - vec2(4.0, 0.0), 29847), sd_char(r4 - vec2(8.0, 0.0), 29842))));
    d = max(d, draw_num(r4 - vec2(14.0, 0.0), i32(u.adv_data.x)));

    // Row 5: MSD  (M=24429, S=29671, D=15211)
    let r5 = base_uv - vec2(0.0, 30.0);
    d = max(d, max(sd_char(r5, 24429), max(sd_char(r5 - vec2(4.0, 0.0), 29671), sd_char(r5 - vec2(8.0, 0.0), 15211))));
    d = max(d, draw_num(r5 - vec2(14.0, 0.0), i32(u.adv_data.y)));

    // Row 6: FTV  (F=29385, T=29842, V=23378)
    // Frame Time Variance %: stddev/mean*100 over the rolling window.
    // 0% = all frames equally spaced, high % = frames bunching and
    // skipping — visually skippy even if mean FPS looks acceptable.
    let r6 = base_uv - vec2(0.0, 36.0);
    d = max(d, max(sd_char(r6, 29385), max(sd_char(r6 - vec2(4.0, 0.0), 29842), sd_char(r6 - vec2(8.0, 0.0), 23378))));
    d = max(d, draw_num(r6 - vec2(14.0, 0.0), i32(u.adv_data.z)));

    return vec4(mix(color, vec3(0.0, 1.0, 0.5), d), 1.0);
}
