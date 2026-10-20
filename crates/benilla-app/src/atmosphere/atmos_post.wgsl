// The atmosphere's post pass on the world camera, before the HUD and on the gamma-byte image
// (the FFXGlow combine decodes it later), in three parts, each 0 for the identity:
// - sun shafts: a radial blur toward the sun of the bright sky only (a luminance threshold), so
//   trees and roofs in front of the sun cut dark lanes through the rays; the moon's are fainter;
// - the storm grade: a little colour drained and cooled while it storms;
// - the lightning flash: a cold white lift over everything;
// - raindrops on the lens: beads that bend the picture behind them, grow, slide and dry.
//   Lens drops after the common procedural technique (cells of hashed drops), written here.
// Rays only from bright sky near the sun follow Corfirean/modern-wow-renderer's idea (no code).

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct Params {
    // xy the light's position in uv (may be off screen), z its strength (0 = no shafts),
    // w the threshold of luminance that counts as bright sky.
    light: vec4<f32>,
    // rgb the shafts' colour, w the screen's aspect (width / height).
    color: vec4<f32>,
    // x the flash, y the storm grade, z the shafts' reach (fraction of the way to the light
    // sampled), w the decay per tap.
    mood: vec4<f32>,
    // x how much rain is on the lens (0 = none), y seconds (wrapped).
    lens: vec4<f32>,
}
@group(0) @binding(2) var<uniform> params: Params;

const TAPS: i32 = 40;
// How fast the emitting halo around the light falls off, per screen height.
const HALO: f32 = 2.2;
const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);
const FLASH_TINT: vec3<f32> = vec3<f32>(0.80, 0.86, 1.0);
const STORM_TINT: vec3<f32> = vec3<f32>(0.93, 0.97, 1.04);

// How much of a sample is bright sky, 0..1.
fn bright(c: vec3<f32>, threshold: f32) -> f32 {
    let l = dot(c, LUMA);
    return smoothstep(threshold, min(threshold + 0.25, 1.0), l);
}

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// One layer of drops: cells `scale` per screen height, each holding at most one drop, a whole
// column sliding down slowly. Returns the drop's bend (xy) and its cover (z, 0..1).
fn drop_layer(uv: vec2<f32>, aspect: f32, scale: f32, t: f32, amount: f32) -> vec3<f32> {
    var p = vec2<f32>(uv.x * aspect, uv.y) * scale;
    let col = floor(p.x);
    p.y -= t * (0.04 + 0.08 * hash21(vec2<f32>(col, scale)));
    let cell = floor(p);
    let f = fract(p) - 0.5;
    let h = hash21(cell + vec2<f32>(scale, 0.0));
    if (h > amount * 0.6) {
        return vec3<f32>(0.0);
    }
    let center = (vec2<f32>(hash21(cell + 1.3), hash21(cell + 2.7)) - 0.5) * 0.5;
    let r = 0.12 + 0.14 * hash21(cell + 4.1);
    // Each drop lives a few seconds: it beads up, holds and dries.
    let life = fract(t * (0.08 + 0.06 * h) + h * 7.0);
    let alive = smoothstep(0.0, 0.08, life) * (1.0 - smoothstep(0.75, 1.0, life));
    var d = f - center;
    // A little fuller at the bottom, as a drop sags.
    d.y *= select(1.0, 0.8, d.y > 0.0);
    let m = (1.0 - smoothstep(r * 0.65, r, length(d))) * alive;
    return vec3<f32>(d / r * m, m);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let src = textureSampleLevel(screen_texture, screen_sampler, in.uv, 0.0);
    var c = src.rgb;

    let lens = params.lens.x;
    if (lens > 0.0) {
        let a = drop_layer(in.uv, params.color.w, 7.0, params.lens.y, lens);
        let b = drop_layer(in.uv, params.color.w, 13.0, params.lens.y + 31.0, lens);
        let bend = a.xy * 0.035 + b.xy * 0.02;
        let cover = max(a.z, b.z);
        if (cover > 0.0) {
            // A drop shows the world behind it bent and a touch dimmer, its lower rim lit.
            let seen = textureSampleLevel(screen_texture, screen_sampler, clamp(in.uv - bend, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0).rgb;
            let rim = clamp(a.y + b.y, 0.0, 1.0) * 0.12;
            c = mix(c, seen * 0.92 + vec3<f32>(rim), cover);
        }
    }

    let strength = params.light.z;
    if (strength > 0.0) {
        let to_light = params.light.xy - in.uv;
        let stride = to_light * (params.mood.z / f32(TAPS));
        // A per-pixel jitter of the first tap trades the taps' banding for fine noise.
        let jitter = fract(sin(dot(in.uv, vec2<f32>(12.9898, 78.233))) * 43758.5453);
        var uv = in.uv + stride * jitter;
        var weight = 1.0;
        var sum = 0.0;
        var norm = 0.0;
        for (var i = 0; i < TAPS; i++) {
            let inside = all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
            let s = textureSampleLevel(screen_texture, screen_sampler, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0).rgb;
            // Sky near the light shines most: a halo weight, so open sky far off adds little.
            let off = (uv - params.light.xy) * vec2<f32>(params.color.w, 1.0);
            let near = exp(-length(off) * HALO);
            sum += select(0.0, bright(s, params.light.w) * near, inside) * weight;
            norm += weight;
            weight *= params.mood.w;
            uv += stride;
        }
        let shafts = sum / max(norm, 1e-4);
        // Rays fade with the distance from the light on screen (aspect-corrected).
        let d = length(vec2<f32>(to_light.x * params.color.w, to_light.y));
        let falloff = 1.0 - smoothstep(0.15, 1.1, d);
        // Screen blend, so the sky near the sun does not burn out.
        let add = params.color.rgb * shafts * strength * falloff;
        c = 1.0 - (1.0 - c) * (1.0 - clamp(add, vec3<f32>(0.0), vec3<f32>(1.0)));
    }

    let storm = params.mood.y;
    if (storm > 0.0) {
        let l = dot(c, LUMA);
        c = mix(c, mix(vec3<f32>(l), c, 0.72) * STORM_TINT, storm);
    }

    let flash = params.mood.x;
    if (flash > 0.0) {
        c = mix(c, max(c, FLASH_TINT * (0.55 + 0.45 * dot(c, LUMA))), flash * 0.5) + FLASH_TINT * flash * 0.12;
    }

    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), src.a);
}
