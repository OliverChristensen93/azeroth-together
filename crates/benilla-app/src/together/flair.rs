//! Some life on the card screens, after Megabonk and Vampire Survivors: behind the hand a slowly
//! turning burst of light rays and a pulsing glow, in the colour of what dealt it (gold for a
//! level-up or treasure, blue for a titan shrine, purple for an altar or an augment); the cards
//! pop in one after another; and a chest's treasure comes with a shower of gold coins.
//!
//! The screen's own dimming lives here, under the rays, so the level-up screen
//! ([`super::levelup`]) draws its cards over them on a clear background.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use together_sim::{InteractableKind, Source};

use super::levelup::{CardButton, TextureIcon};
use super::Sim;

/// The rays' and glow's square, in the 768-unit canvas, and how many rays.
const BURST: f32 = 1200.0;
const RAYS: usize = 18;
/// Radians a second the rays turn.
const TURN: f32 = 0.12;
/// A card's pop: seconds to grow, from this scale, each card this much after the one before.
const POP_SECS: f32 = 0.28;
const POP_FROM: f32 = 0.55;
const POP_STAGGER: f32 = 0.07;
/// The coin shower: how many, and how long one lives.
const COINS: usize = 42;
const COIN_SECS: f32 = 2.8;

#[derive(Component)]
pub(super) struct FlairRoot;

#[derive(Component)]
pub(super) struct Rays;

#[derive(Component)]
pub(super) struct Glow;

/// A card growing into place: when it appeared and how long it waits.
#[derive(Component)]
pub(super) struct PopIn {
    born: f32,
    delay: f32,
}

/// A falling coin: when it began, where it falls from (x, as a share of the screen), its speed,
/// sway and spin.
#[derive(Component)]
pub(super) struct Coin {
    born: f32,
    x: f32,
    fall: f32,
    sway: f32,
    spin: f32,
    size: f32,
}

#[derive(Component)]
pub(super) struct CoinLayer;

/// What the screen last showed: the source of its hand, so a new hand restarts the flair.
#[derive(Default)]
pub(super) struct Seen {
    source: Option<Source>,
    seed: u64,
}

/// The rays' and glow's colour for what dealt the hand.
fn tint(source: Source) -> Color {
    match source {
        Source::LevelUp => Color::srgb(1.0, 0.8, 0.3),
        Source::Augment(_) => Color::srgb(0.75, 0.45, 1.0),
        Source::Interactable(kind) => match kind {
            InteractableKind::TitanShrine => Color::srgb(0.4, 0.7, 1.0),
            InteractableKind::CursedAltar => Color::srgb(0.7, 0.3, 1.0),
            InteractableKind::ChallengeIdol => Color::srgb(1.0, 0.4, 0.3),
            _ => Color::srgb(1.0, 0.82, 0.25),
        },
    }
}

fn rays(color: Color) -> BackgroundGradient {
    let step = std::f32::consts::TAU / RAYS as f32;
    let mut stops = Vec::with_capacity(RAYS * 2 + 1);
    // Each ray a soft-edged spoke with a dark gap either side.
    for k in 0..RAYS {
        let a = k as f32 * step;
        stops.push(AngularColorStop::new(color.with_alpha(0.0), a));
        stops.push(AngularColorStop::new(color.with_alpha(0.0), a + step * 0.3));
        stops.push(AngularColorStop::new(
            color.with_alpha(0.42),
            a + step * 0.5,
        ));
        stops.push(AngularColorStop::new(color.with_alpha(0.0), a + step * 0.7));
    }
    stops.push(AngularColorStop::new(
        color.with_alpha(0.0),
        std::f32::consts::TAU,
    ));
    BackgroundGradient::from(ConicGradient::new(UiPosition::CENTER, stops))
}

/// The glow at the middle and the dark that swallows the rays toward the edges.
fn glow(color: Color, strength: f32) -> BackgroundGradient {
    BackgroundGradient::from(RadialGradient::new(
        UiPosition::CENTER,
        RadialGradientShape::ClosestSide,
        vec![
            ColorStop::new(color.with_alpha(0.30 * strength), Val::Percent(0.0)),
            ColorStop::new(color.with_alpha(0.08 * strength), Val::Percent(35.0)),
            ColorStop::new(Color::srgba(0.0, 0.0, 0.0, 0.0), Val::Percent(70.0)),
            ColorStop::new(Color::srgba(0.0, 0.0, 0.0, 0.5), Val::Percent(100.0)),
        ],
    ))
}

pub(super) fn setup(mut commands: Commands) {
    fn square<M: Component>(marker: M) -> impl Bundle {
        (
            marker,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(BURST),
                height: Val::Px(BURST),
                margin: UiRect {
                    left: Val::Px(-BURST / 2.0),
                    top: Val::Px(-BURST / 2.0),
                    ..default()
                },
                ..default()
            },
            FocusPolicy::Pass,
        )
    }
    // Under the level-up screen (300): the dimming, the rays, the glow.
    commands
        .spawn((
            FlairRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            FocusPolicy::Pass,
            GlobalZIndex(299),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((square(Rays), rays(Color::WHITE)))
                .insert(UiTransform::IDENTITY);
            root.spawn((square(Glow), glow(Color::WHITE, 1.0)));
        });
    // Over it: the coins.
    commands.spawn((
        CoinLayer,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            overflow: Overflow::clip(),
            ..default()
        },
        FocusPolicy::Pass,
        bevy::picking::Pickable::IGNORE,
        GlobalZIndex(305),
    ));
}

/// A little pseudo-random number in 0..1 from `seed`, moving it on.
fn rand(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed >> 40) as f32 / (1u64 << 24) as f32
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn animate(
    mut commands: Commands,
    sim: Option<Res<Sim>>,
    time: Res<Time<Real>>,
    mut root: Query<&mut Visibility, With<FlairRoot>>,
    mut rays_q: Query<(&mut UiTransform, &mut BackgroundGradient), (With<Rays>, Without<Glow>)>,
    mut glow_q: Query<&mut BackgroundGradient, (With<Glow>, Without<Rays>)>,
    new_cards: Query<(Entity, &CardButton), Added<CardButton>>,
    mut pops: Query<(Entity, &PopIn, &mut UiTransform), (Without<Rays>, Without<Coin>)>,
    layer: Query<Entity, With<CoinLayer>>,
    mut coins: Query<(Entity, &Coin, &mut Node, &mut UiTransform), (Without<Rays>, Without<PopIn>)>,
    mut seen: Local<Seen>,
) {
    let now = time.elapsed_secs();
    let source = sim
        .as_ref()
        .and_then(|s| s.world.hero(s.me))
        .filter(|h| h.alive)
        .and_then(|h| h.offers.first())
        .map(|o| o.source);
    let want = if source.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut root {
        if *v != want {
            *v = want;
        }
    }
    if seen.seed == 0 {
        seen.seed = 0x9E37_79B9_7F4A_7C15 ^ (now.to_bits() as u64);
    }
    // A new hand: its colours, and for treasure, the coins.
    if source != seen.source {
        if let Some(src) = source {
            let color = tint(src);
            for (_, mut g) in &mut rays_q {
                *g = rays(color);
            }
            for mut g in &mut glow_q {
                *g = glow(color, 1.0);
            }
            let treasure = matches!(
                src,
                Source::Interactable(InteractableKind::Chest | InteractableKind::Cache)
            );
            if treasure {
                if let Ok(layer) = layer.single() {
                    let mut seed = seen.seed;
                    commands.entity(layer).with_children(|l| {
                        for k in 0..COINS {
                            let coin = Coin {
                                born: now + k as f32 * 0.035,
                                x: rand(&mut seed),
                                fall: 260.0 + rand(&mut seed) * 260.0,
                                sway: (rand(&mut seed) - 0.5) * 120.0,
                                spin: (rand(&mut seed) - 0.5) * 9.0,
                                size: 16.0 + rand(&mut seed) * 12.0,
                            };
                            l.spawn((
                                ImageNode {
                                    color: Color::srgba(0.2, 0.2, 0.2, 0.0),
                                    ..default()
                                },
                                TextureIcon("INV_Misc_Coin_01"),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Percent(coin.x * 100.0),
                                    top: Val::Px(-40.0),
                                    width: Val::Px(coin.size),
                                    height: Val::Px(coin.size),
                                    ..default()
                                },
                                UiTransform::IDENTITY,
                                FocusPolicy::Pass,
                                bevy::picking::Pickable::IGNORE,
                                coin,
                            ));
                        }
                    });
                    seen.seed = seed;
                }
            }
        }
        seen.source = source;
    }
    // The rays turn and the glow breathes.
    for (mut tf, _) in &mut rays_q {
        tf.rotation = Rot2::radians(now * TURN);
    }
    if let Some(src) = source {
        let breathe = 0.8 + 0.2 * (now * 2.2).sin();
        for mut g in &mut glow_q {
            *g = glow(tint(src), breathe);
        }
    }
    // Cards pop in, one after another.
    for (e, card) in &new_cards {
        commands.entity(e).insert((
            PopIn {
                born: now,
                delay: f32::from(card_index(card)) * POP_STAGGER,
            },
            UiTransform {
                scale: Vec2::splat(POP_FROM),
                ..UiTransform::IDENTITY
            },
        ));
    }
    for (e, pop, mut tf) in &mut pops {
        let t = ((now - pop.born - pop.delay) / POP_SECS).clamp(0.0, 1.0);
        // Ease out with a little overshoot.
        let s = 1.0 - (1.0 - t).powi(3);
        let over = (t * std::f32::consts::PI).sin() * 0.06;
        let scale = POP_FROM + (1.0 - POP_FROM) * s + over;
        tf.scale = Vec2::splat(scale);
        if t >= 1.0 {
            tf.scale = Vec2::ONE;
            commands.entity(e).remove::<PopIn>();
        }
    }
    // Coins fall, sway and spin, then go.
    for (e, coin, mut node, mut tf) in &mut coins {
        let age = now - coin.born;
        if age > COIN_SECS {
            commands.entity(e).despawn();
            continue;
        }
        let age = age.max(0.0);
        node.top = Val::Px(-40.0 + coin.fall * age + 90.0 * age * age);
        tf.translation = Val2::px((age * 2.0).sin() * coin.sway, 0.0);
        tf.rotation = Rot2::radians(age * coin.spin);
    }
}

fn card_index(card: &CardButton) -> u8 {
    card.0
}
