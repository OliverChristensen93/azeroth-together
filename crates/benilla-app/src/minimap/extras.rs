//! Blips and rings a game mode puts on the minimap itself, beyond what the reference's own
//! classifiers draw: the survivors mode marks its boss, elites, a banner's destination, treasure
//! and the arena's edge ([`crate::together`]). The mode fills [`MinimapExtras`] every frame; the
//! emitter draws it in the same pass as the landmarks, masked to the map's circle.

use bevy::math::Rect;
use bevy::prelude::*;

use crate::ui_pass::{UiQuad, UiQuadMask, UiQuads, UvRect};

use super::blips::{BlipCtx, MinimapBlipHover, BLIP_BASIS_PX};

/// Where an out-of-range blip sits, as a share of the map's radius: the rim arrows' ring.
const BLIP_EDGE_RATIO: f32 = 0.8;

/// `ObjectIcons.blp` cells: 0 the gold dot, 1 the red.
const GOLD_DOT: [f32; 4] = [0.0, 0.25, 0.0, 0.25];
const RED_DOT: [f32; 4] = [0.25, 0.5, 0.0, 0.25];
/// `POIIcons.blp` (8×8) cells: 6 the red flag, 7 the skull.
const FLAG: [f32; 4] = [0.75, 0.875, 0.0, 0.125];
const SKULL: [f32; 4] = [0.875, 1.0, 0.0, 0.125];
/// Segments in a ring.
const RING_SEGMENTS: usize = 96;

/// The art a blip wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExtraShape {
    GoldDot,
    RedDot,
    Flag,
    Skull,
}

/// One blip at a WoW world point (`x`, `y`). `size_px` is on the reference's 140.8-px basis (a
/// party dot is 10.4); `rim` keeps it on the map's edge, pointing the way, when out of range.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExtraBlip {
    pub pos: [f32; 2],
    pub shape: ExtraShape,
    pub size_px: f32,
    /// Straight-alpha tint over the art.
    pub color: [f32; 4],
    pub rim: bool,
    /// What the tooltip under the mouse says, as a tracked dot's name does; empty for none.
    pub name: String,
}

/// A circle on the ground drawn on the map: the arena's edge.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExtraRing {
    pub center: [f32; 2],
    pub radius: f32,
    pub width_px: f32,
    pub color: [f32; 4],
}

/// What the mode wants on the minimap this frame.
#[derive(Resource, Default, Clone, Debug)]
pub(crate) struct MinimapExtras {
    pub blips: Vec<ExtraBlip>,
    pub rings: Vec<ExtraRing>,
}

/// Draw `extras` into the frame `ctx`: rings first, then blips.
pub(super) fn emit(
    ctx: &BlipCtx,
    extras: &MinimapExtras,
    object_icons: Option<&Handle<Image>>,
    poi: Option<&Handle<Image>>,
    mask: Option<&UiQuadMask>,
    quads: &mut UiQuads,
    hover: &mut MinimapBlipHover,
) {
    for ring in &extras.rings {
        emit_ring(ctx, ring, mask, quads);
    }
    let edge = ctx.side * 0.5 * BLIP_EDGE_RATIO;
    for blip in &extras.blips {
        let (tex, cell) = match blip.shape {
            ExtraShape::GoldDot => (object_icons, GOLD_DOT),
            ExtraShape::RedDot => (object_icons, RED_DOT),
            ExtraShape::Flag => (poi, FLAG),
            ExtraShape::Skull => (poi, SKULL),
        };
        let Some(tex) = tex else {
            continue;
        };
        let mut off = ctx.offset([blip.pos[0], blip.pos[1], 0.0]);
        let mut size = ctx.side * (blip.size_px / BLIP_BASIS_PX);
        if off.length() > edge {
            if !blip.rim {
                continue;
            }
            off = off.normalize_or_zero() * edge;
            size *= 0.85;
        }
        let mut color = blip.color;
        color[3] *= ctx.alpha;
        let rect = Rect::from_center_size(ctx.center + off, Vec2::splat(size));
        quads.overlays.push(UiQuad {
            rect,
            z_key: ctx.z,
            texture: Some(tex.clone()),
            uv: UvRect::from_tex_coords(cell),
            color,
            ..default()
        });
        // Under the mouse: its name, in the minimap's own tooltip.
        if let (Some(c), Some(ui)) = (ctx.cursor, ctx.cursor_ui) {
            if !blip.name.is_empty() && rect.contains(c) {
                *hover = MinimapBlipHover::Landmark(blip.name.clone(), ui);
            }
        }
    }
}

/// A ring as thin quads around its circle, clipped to the map by its mask.
fn emit_ring(ctx: &BlipCtx, ring: &ExtraRing, mask: Option<&UiQuadMask>, quads: &mut UiQuads) {
    // A ring wholly outside the view, or wholly enclosing it with its line far off, draws nothing.
    let to_center = ctx.offset([ring.center[0], ring.center[1], 0.0]);
    let r_px = ring.radius * ctx.px_per_yd;
    let view = ctx.side * 0.5;
    if (to_center.length() - r_px).abs() > view * 1.5 {
        return;
    }
    let half = ring.width_px * 0.5 * (ctx.side / BLIP_BASIS_PX);
    let point = |k: usize, r: f32| {
        let a = k as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
        let w = [
            ring.center[0] + a.cos() * r,
            ring.center[1] + a.sin() * r,
            0.0,
        ];
        ctx.center + ctx.offset(w)
    };
    let inner = (ring.radius - half / ctx.px_per_yd).max(0.0);
    let outer = ring.radius + half / ctx.px_per_yd;
    let mut color = ring.color;
    color[3] *= ctx.alpha;
    for k in 0..RING_SEGMENTS {
        let corners = [
            point(k, inner),
            point(k, outer),
            point(k + 1, outer),
            point(k + 1, inner),
        ];
        let min = corners.iter().fold(Vec2::splat(f32::MAX), |m, c| m.min(*c));
        let max = corners.iter().fold(Vec2::splat(f32::MIN), |m, c| m.max(*c));
        // Only the segments that cross the map's square.
        let rect = Rect::from_corners(min, max);
        let map = Rect::from_center_size(ctx.center, Vec2::splat(ctx.side));
        if rect.intersect(map).is_empty() {
            continue;
        }
        quads.overlays.push(UiQuad {
            rect,
            z_key: ctx.z,
            texture: None,
            color,
            mask: mask.cloned(),
            corners: Some(corners),
            ..default()
        });
    }
}
