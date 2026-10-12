struct Globals {
    size: vec2<f32>,
    pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct VertexIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) rect: vec4<f32>,
    @location(4) radius: f32,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) rect: vec4<f32>,
    @location(3) radius: f32,
};

@vertex
fn vs_main(v: VertexIn) -> VertexOut {
    var out: VertexOut;
    let ndc = vec2<f32>(v.pos.x / globals.size.x * 2.0 - 1.0, 1.0 - v.pos.y / globals.size.y * 2.0);
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = v.uv;
    out.color = v.color;
    out.rect = v.rect;
    out.radius = v.radius;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    var c = textureSample(tex, samp, in.uv) * in.color;
    if (in.radius > 0.0) {
        let half = in.rect.zw * 0.5;
        let q = abs(in.clip.xy - (in.rect.xy + half)) - half + vec2<f32>(in.radius);
        let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - in.radius;
        c.a = c.a * clamp(0.5 - d, 0.0, 1.0);
    }
    return c;
}
