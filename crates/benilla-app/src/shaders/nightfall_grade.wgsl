// Nightfall's grade: the world a little drained of colour, cooled toward moonlight, darkened, with
// a vignette that closes the corners in. It runs on the world camera before the HUD, which draws
// over it untouched. `a.x` is the master amount: 0 is the identity.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct Params {
    // x amount, y desaturation 0..1, z vignette strength, w darkening 0..1.
    a: vec4<f32>,
    // xyz tint multiplier, w the vignette's inner edge: 0 the centre, 1 the corners.
    b: vec4<f32>,
}
@group(0) @binding(2) var<uniform> params: Params;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let src = textureSample(screen_texture, screen_sampler, in.uv);
    let amount = params.a.x;
    if (amount <= 0.0) {
        return src;
    }
    let l = dot(src.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    var c = mix(src.rgb, vec3<f32>(l), params.a.y) * params.b.xyz * (1.0 - params.a.w);
    let d = distance(in.uv, vec2<f32>(0.5, 0.5)) * 1.41421356;
    c = c * (1.0 - params.a.z * smoothstep(params.b.w, 1.0, d));
    return vec4<f32>(mix(src.rgb, c, amount), src.a);
}
