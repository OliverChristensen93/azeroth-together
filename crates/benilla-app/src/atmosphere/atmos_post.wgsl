// The atmosphere's post pass on the world camera, before the HUD and on the gamma-byte image
// (the FFXGlow combine decodes it later). It reads the scene's depth, so it knows how far away
// and how high every pixel is. In order, each 0 for the identity:
// - polish: a light sharpen (contrast-adaptive, after AMD's CAS idea), vibrance and contrast;
// - ambient occlusion: soft contact shadow where surfaces meet, from depth (position and normal
//   rebuilt per pixel, a rotated disc of samples);
// - height fog: a thin, low layer of ground mist that pools in hollows and drifts, plus a light
//   aerial haze, both integrated along each pixel's ray, lit toward the sun;
// - sun shafts: a radial blur toward the sun of bright sky only (depth 0), so trees and roofs
//   in front of the sun cut dark lanes through the rays; the moon's are fainter;
// - the storm grade, the lightning flash and (opt-in) raindrops on the lens.
// Techniques after the ideas listed by Corfirean/modern-wow-renderer (sun shafts from bright sky,
// height fog with ground mist, depth-derived occlusion); written here, no code taken.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::View

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct Params {
    // xy the light's position in uv (may be off screen), z its strength (0 = no shafts),
    // w the threshold of luminance that counts as bright sky.
    light: vec4<f32>,
    // rgb the light's colour, w the screen's aspect (width / height).
    color: vec4<f32>,
    // x the flash, y the storm grade, z the shafts' reach, w the decay per tap.
    mood: vec4<f32>,
    // x rain on the lens (0 = none), y seconds.
    lens: vec4<f32>,
    // rgb the fog's colour, w the ground mist's density per yard at its floor (0 = none).
    fog_color: vec4<f32>,
    // x the mist's floor height, y its falloff per yard, z the haze's density per yard, w seconds.
    fog: vec4<f32>,
    // x occlusion strength (0 = none), y its radius in yards, zw the mist's drift per second.
    ao: vec4<f32>,
    // xyz camera to sun, w how much the fog glows toward the sun.
    sun: vec4<f32>,
    // x polish amount (0 = none), y sharpness, z vibrance, w contrast.
    polish: vec4<f32>,
}
@group(0) @binding(2) var<uniform> params: Params;
#ifdef MULTISAMPLED
@group(0) @binding(3) var depth_texture: texture_depth_multisampled_2d;
#else
@group(0) @binding(3) var depth_texture: texture_depth_2d;
#endif
@group(0) @binding(4) var<uniform> view: View;

const TAPS: i32 = 40;
const HALO: f32 = 2.2;
const AO_SAMPLES: i32 = 12;
const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);
const FLASH_TINT: vec3<f32> = vec3<f32>(0.80, 0.86, 1.0);
const STORM_TINT: vec3<f32> = vec3<f32>(0.93, 0.97, 1.04);
// How far a sky pixel's ray is taken for the haze, yards.
const SKY_DISTANCE: f32 = 600.0;

fn dims() -> vec2<f32> {
    return vec2<f32>(textureDimensions(screen_texture));
}

fn depth_at(uv: vec2<f32>) -> f32 {
    let size = vec2<i32>(textureDimensions(depth_texture));
    let p = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - 1);
    return textureLoad(depth_texture, p, 0);
}

// The world position of `uv` at NDC depth `depth` (reverse-Z: 1 near, 0 infinitely far).
fn world_at(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let w = view.world_from_clip * ndc;
    return w.xyz / w.w;
}

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// Smooth value noise, 0..1.
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// How much of a sample is bright sky, 0..1.
fn bright(c: vec3<f32>, threshold: f32) -> f32 {
    let l = dot(c, LUMA);
    return smoothstep(threshold, min(threshold + 0.25, 1.0), l);
}

// The optical depth of an exponential height layer (density `a` at height `h0`, falling off
// by `b` per yard) along the ray from `origin` over `ray`.
fn layer(origin: vec3<f32>, ray: vec3<f32>, a: f32, b: f32, h0: f32) -> f32 {
    let len = length(ray);
    // Below the floor the layer is as thick as at it: a mist does not thicken underground.
    let start = a * exp(-b * max(origin.y - h0, 0.0));
    let k = b * ray.y;
    var shape = 1.0;
    if (abs(k) > 1e-3) {
        shape = (1.0 - exp(-k)) / k;
    }
    return start * len * max(shape, 0.0);
}

// Contact shadow at the world point `p` seen at `uv`, 0..1 (1 = open).
fn occlusion(uv: vec2<f32>, p: vec2<f32>, pos: vec3<f32>, dist: f32) -> f32 {
    let px = 1.0 / dims();
    // The normal from the nearer neighbour on each axis, so edges do not smear.
    let r = world_at(uv + vec2<f32>(px.x, 0.0), depth_at(uv + vec2<f32>(px.x, 0.0)));
    let l = world_at(uv - vec2<f32>(px.x, 0.0), depth_at(uv - vec2<f32>(px.x, 0.0)));
    let d = world_at(uv + vec2<f32>(0.0, px.y), depth_at(uv + vec2<f32>(0.0, px.y)));
    let u = world_at(uv - vec2<f32>(0.0, px.y), depth_at(uv - vec2<f32>(0.0, px.y)));
    let dx = select(pos - l, r - pos, length(r - pos) < length(pos - l));
    let dy = select(pos - u, d - pos, length(d - pos) < length(pos - u));
    var n = normalize(cross(dy, dx));
    let to_eye = view.world_position - pos;
    if (dot(n, to_eye) < 0.0) {
        n = -n;
    }
    let radius = params.ao.y;
    // The radius on screen: the projection's focal length over the distance, in uv.
    let focal = view.clip_from_view[1][1] * 0.5;
    let r_uv = min(radius * focal / max(dist, 0.5), 0.12);
    // Interleaved gradient noise turns the disc per pixel.
    let ign = fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
    var occluded = 0.0;
    for (var i = 0; i < AO_SAMPLES; i++) {
        let t = (f32(i) + 0.5) / f32(AO_SAMPLES);
        let angle = (f32(i) * 2.39996 + ign * 6.28318);
        let off = vec2<f32>(cos(angle), sin(angle)) * r_uv * sqrt(t) * vec2<f32>(1.0 / params.color.w, 1.0);
        let su = uv + off;
        let sd = depth_at(su);
        if (sd <= 0.0) {
            continue;
        }
        let v = world_at(su, sd) - pos;
        let len = length(v);
        let facing = max(dot(n, v / max(len, 1e-4)) - 0.15, 0.0);
        let range = 1.0 - smoothstep(radius * 0.6, radius * 1.6, len);
        occluded += facing * range;
    }
    return clamp(1.0 - occluded / f32(AO_SAMPLES) * 2.0, 0.0, 1.0);
}

// The polish: CAS-like sharpen from the four neighbours, vibrance and a soft S-curve.
fn polish(uv: vec2<f32>, c: vec3<f32>) -> vec3<f32> {
    let px = 1.0 / dims();
    let n = textureSampleLevel(screen_texture, screen_sampler, uv - vec2<f32>(0.0, px.y), 0.0).rgb;
    let s = textureSampleLevel(screen_texture, screen_sampler, uv + vec2<f32>(0.0, px.y), 0.0).rgb;
    let w = textureSampleLevel(screen_texture, screen_sampler, uv - vec2<f32>(px.x, 0.0), 0.0).rgb;
    let e = textureSampleLevel(screen_texture, screen_sampler, uv + vec2<f32>(px.x, 0.0), 0.0).rgb;
    let lo = min(c, min(min(n, s), min(w, e)));
    let hi = max(c, max(max(n, s), max(w, e)));
    // Less sharpening where the local contrast is already high.
    let amp = sqrt(clamp(min(lo, 1.0 - hi) / max(hi, vec3<f32>(1e-4)), vec3<f32>(0.0), vec3<f32>(1.0)));
    let wgt = -amp * mix(0.125, 0.2, params.polish.y);
    var out = clamp((c + (n + s + w + e) * wgt) / (1.0 + 4.0 * wgt), vec3<f32>(0.0), vec3<f32>(1.0));
    let l = dot(out, LUMA);
    let sat = max(out.r, max(out.g, out.b)) - min(out.r, min(out.g, out.b));
    out = mix(vec3<f32>(l), out, 1.0 + params.polish.z * (1.0 - sat));
    let curved = out * out * (3.0 - 2.0 * out);
    out = mix(out, curved, params.polish.w);
    return mix(c, clamp(out, vec3<f32>(0.0), vec3<f32>(1.0)), params.polish.x);
}

// One layer of lens drops (see `lens` in the uniform): bend (xy) and cover (z).
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
    let life = fract(t * (0.08 + 0.06 * h) + h * 7.0);
    let alive = smoothstep(0.0, 0.08, life) * (1.0 - smoothstep(0.75, 1.0, life));
    var d = f - center;
    d.y *= select(1.0, 0.8, d.y > 0.0);
    let m = (1.0 - smoothstep(r * 0.65, r, length(d))) * alive;
    return vec3<f32>(d / r * m, m);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let src = textureSampleLevel(screen_texture, screen_sampler, uv, 0.0);
    var c = src.rgb;
    let depth = depth_at(uv);
    let sky = depth <= 0.0;
    let eye = view.world_position;
    var pos: vec3<f32>;
    if (sky) {
        pos = eye + normalize(world_at(uv, 1.0) - eye) * SKY_DISTANCE;
    } else {
        pos = world_at(uv, depth);
    }
    let ray = pos - eye;
    let dist = length(ray);

    if (params.polish.x > 0.0) {
        c = polish(uv, c);
    }

    if (params.ao.x > 0.0 && !sky && dist < 90.0) {
        let ao = occlusion(uv, in.position.xy, pos, dist);
        let fade = 1.0 - smoothstep(50.0, 90.0, dist);
        c *= mix(1.0, ao, params.ao.x * fade);
    }

    let mist = params.fog_color.w;
    let haze = params.fog.z;
    if (mist > 0.0 || haze > 0.0) {
        var depth_fog = layer(eye, ray, haze, 0.01, params.fog.x);
        if (!sky && mist > 0.0) {
            // The mist drifts in patches: its density follows a slow noise over the ground.
            let drift = params.ao.zw * params.fog.w;
            let patches = noise(pos.xz * 0.045 + drift) * 0.7 + noise(pos.xz * 0.11 - drift * 1.7) * 0.3;
            depth_fog += layer(eye, ray, mist, params.fog.y, params.fog.x) * (0.35 + 1.1 * patches);
        }
        var amount = 1.0 - exp(-depth_fog);
        amount = min(amount, select(0.9, 0.45, sky));
        // Lit toward the sun: the fog in the sun's direction glows its colour.
        let toward = pow(max(dot(ray / max(dist, 1e-4), params.sun.xyz), 0.0), 6.0) * params.sun.w;
        let col = mix(params.fog_color.rgb, params.color.rgb, toward * 0.6);
        c = mix(c, col, amount);
    }

    let strength = params.light.z;
    if (strength > 0.0) {
        let to_light = params.light.xy - uv;
        let stride = to_light * (params.mood.z / f32(TAPS));
        let jitter = fract(sin(dot(uv, vec2<f32>(12.9898, 78.233))) * 43758.5453);
        var tap = uv + stride * jitter;
        var weight = 1.0;
        var sum = 0.0;
        var norm = 0.0;
        for (var i = 0; i < TAPS; i++) {
            let inside = all(tap >= vec2<f32>(0.0)) && all(tap <= vec2<f32>(1.0));
            let at = clamp(tap, vec2<f32>(0.0), vec2<f32>(1.0));
            // Only open sky shines: terrain, trees and roofs are dark lanes.
            let open = inside && depth_at(at) <= 0.0;
            let s = textureSampleLevel(screen_texture, screen_sampler, at, 0.0).rgb;
            let off = (tap - params.light.xy) * vec2<f32>(params.color.w, 1.0);
            let near = exp(-length(off) * HALO);
            sum += select(0.0, bright(s, params.light.w) * near, open) * weight;
            norm += weight;
            weight *= params.mood.w;
            tap += stride;
        }
        let shafts = sum / max(norm, 1e-4);
        let d = length(vec2<f32>(to_light.x * params.color.w, to_light.y));
        let falloff = 1.0 - smoothstep(0.15, 1.1, d);
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

    let lens = params.lens.x;
    if (lens > 0.0) {
        let a = drop_layer(uv, params.color.w, 7.0, params.lens.y, lens);
        let b = drop_layer(uv, params.color.w, 13.0, params.lens.y + 31.0, lens);
        let bend = a.xy * 0.035 + b.xy * 0.02;
        let cover = max(a.z, b.z);
        if (cover > 0.0) {
            let seen = textureSampleLevel(screen_texture, screen_sampler, clamp(uv - bend, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0).rgb;
            let rim = clamp(a.y + b.y, 0.0, 1.0) * 0.12;
            c = mix(c, seen * 0.92 + vec3<f32>(rim), cover);
        }
    }

    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), src.a);
}