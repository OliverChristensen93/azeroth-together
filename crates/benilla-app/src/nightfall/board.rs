//! The inn's door and the raid board (spec §8.4): stepping into the door's ring opens it. It shows
//! which map (Duskwood), the contracts taken and what is done of them, the map table's intel when
//! it is built (which exits open when, from the raid the host plans), the party with who is
//! ready, and Ready. Alone, Ready starts the raid; hosting, the raid starts once everyone present
//! is ready; joined, the host starts it. Stepping out of the ring takes Ready back.
//!
//! Test switch: `NIGHTFALL_BOARD=1` holds the board open wherever the hero stands, for a shot.
//!
//! Ready itself is Task 6's lock ([`InnState::ready`], [`Net::party_ready`]): this board is only
//! where it is pressed. Autoplay (`NIGHTFALL_AUTOPLAY`) sets Ready and starts the raid in
//! [`super::session::lobby`] and [`super::run::start_run`] without standing at the door.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use nightfall_sim::{Content, ExitMark, HeroPose, InnContent, Profile, RaidRoll, V2};

use super::exits::exit_list;
use super::menu::StartRequest;
use super::session::{InnState, Net};
use super::stash::Stash;
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL};
use super::{content, inn, Phase};
use crate::game_pick::GameMode;

/// Whether a hero at `pose` stands in the door's ring.
pub(super) fn at_door(pose: Option<HeroPose>, door: V2, radius: f32) -> bool {
    pose.is_some_and(|p| p.pos.distance(door) <= radius)
}

/// The exits as a raid about to begin has them, for the map table.
pub(super) fn intel_marks(roll: &RaidRoll) -> Vec<ExitMark> {
    roll.exits
        .iter()
        .enumerate()
        .map(|(i, p)| ExitMark {
            exit: i as u8,
            open: p.open && p.opens_at <= 0.0,
            opens_in: p.open.then_some(p.opens_at),
        })
        .collect()
}

/// Whether the board is open (our hero is in the door's ring) and Ready, after our hero's place
/// is known: stepping out of the ring takes Ready back, so a hero who walks off is not left
/// holding the party's start.
fn door_step(was_open: bool, here: bool, ready: bool) -> (bool, bool) {
    (here, ready && !(was_open && !here))
}

/// Ready after a press on the button: a joined player can be ready only once welcomed (until
/// then the host has not heard of them); anyone may take it back.
fn ready_after_press(ready: bool, is_client: bool, welcomed: bool) -> bool {
    if ready {
        false
    } else {
        !is_client || welcomed
    }
}

/// How a line of the board looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Head,
    Plain,
    Done,
    Dim,
}

pub(super) type Line = (String, Tone);

/// The board's lines. `party` is everyone in the inn with their readiness, us first; `waiting` is
/// what the party still waits for, in words (none once nothing does).
fn board_lines(
    content: &Content,
    inn: &InnContent,
    profile: &Profile,
    planned: Option<&RaidRoll>,
    party: &[(String, bool)],
    waiting: Option<String>,
) -> Vec<Line> {
    let mut lines = vec![(format!("The raid: {}", content.map.name), Tone::Head)];
    let taken = profile.taken(inn);
    if taken.is_empty() {
        lines.push((
            "No contract taken: the inn's people have work.".into(),
            Tone::Dim,
        ));
    }
    for c in taken {
        lines.push((format!("Contract: {}", inn.contracts[c].name), Tone::Plain));
        for (line, met) in profile.objective_lines(c, inn, content) {
            lines.push((
                format!("  {line}"),
                if met { Tone::Done } else { Tone::Dim },
            ));
        }
    }
    if let (true, Some(roll)) = (profile.inn.intel, planned) {
        lines.push(("The map table:".into(), Tone::Plain));
        for l in exit_list(&intel_marks(roll), content).lines() {
            lines.push((format!("  {l}"), Tone::Dim));
        }
    }
    lines.push(("The party".into(), Tone::Plain));
    for (name, ready) in party {
        let tone = if *ready { Tone::Done } else { Tone::Dim };
        lines.push((
            format!("  {name}{}", if *ready { "  ·  ready" } else { "" }),
            tone,
        ));
    }
    if let Some(w) = waiting {
        lines.push((w, Tone::Dim));
    }
    lines
}

/// Whether the board is open: our hero is in the door's ring.
#[derive(Resource, Default)]
pub(super) struct BoardOpen(pub bool);

#[derive(Component)]
pub(super) struct BoardRoot;

#[derive(Component)]
pub(super) struct BoardBody;

#[derive(Component)]
pub(super) struct ReadyButton;

#[derive(Resource)]
pub(super) struct BoardFont(Handle<Font>);

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            BoardRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                bottom: Val::Px(120.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            // The root spans the window: it must let clicks through to what is under it.
            FocusPolicy::Pass,
            GlobalZIndex(420),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                BoardBody,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    min_width: Val::Px(380.0),
                    max_width: Val::Px(560.0),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(EDGE),
            ));
        });
    commands.insert_resource(BoardFont(font));
}

/// `NIGHTFALL_BOARD`, read once: the board stays open wherever the hero stands.
fn test_open() -> bool {
    static OPEN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OPEN.get_or_init(|| std::env::var_os("NIGHTFALL_BOARD").is_some())
}

/// The board opens as our hero steps into the door's ring and closes as they step out, which
/// takes Ready back.
pub(super) fn door(mut state: ResMut<InnState>, mut open: ResMut<BoardOpen>) {
    let place = &inn().place;
    let here = test_open() || at_door(state.pose, place.door, place.door_radius);
    let (now_open, now_ready) = door_step(open.0, here, state.ready);
    if open.0 != now_open {
        open.0 = now_open;
    }
    if state.ready != now_ready {
        state.reset();
    }
}

/// Shown while Nightfall's inn is up and our hero is at the door.
pub(super) fn show(
    mode: Res<State<GameMode>>,
    phase: Res<State<Phase>>,
    open: Res<BoardOpen>,
    mut roots: Query<&mut Visibility, With<BoardRoot>>,
) {
    let on = *mode.get() == GameMode::Nightfall && *phase.get() == Phase::Inn && open.0;
    let want = if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
}

/// The board's lines, built again when they change (looked at twice a second while it is open).
#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild(
    mut commands: Commands,
    open: Res<BoardOpen>,
    stash: Res<Stash>,
    net: Res<Net>,
    state: Res<InnState>,
    real: Res<Time<Real>>,
    font: Option<Res<BoardFont>>,
    body: Query<Entity, With<BoardBody>>,
    mut last: Local<(f32, bool, Vec<Line>)>,
) {
    if !open.0 {
        return;
    }
    let now = real.elapsed_secs();
    if now - last.0 < 0.5 && !open.is_changed() {
        return;
    }
    last.0 = now;
    let content = content();
    let inn = inn();
    let class_name = |key: &str| {
        content
            .class(key)
            .map_or_else(|| key.to_owned(), |i| content.classes[i].name.clone())
    };
    let mut party = vec![("You".to_owned(), state.ready)];
    for f in net.friends() {
        party.push((
            format!("{}  ·  {}", f.hero.name, class_name(&f.hero.class)),
            f.ready,
        ));
    }
    let waiting = if net.is_client() {
        Some("The host starts the raid once everyone is ready.".to_owned())
    } else if state.ready {
        net.first_unready()
            .map(|n| format!("Waiting for {n} to be ready…"))
    } else if net.is_solo() {
        Some("Ready sets out at once.".to_owned())
    } else {
        Some("The raid starts once everyone is ready.".to_owned())
    };
    let lines = board_lines(
        &content,
        &inn,
        &stash.profile,
        net.planned_raid().as_ref(),
        &party,
        waiting,
    );
    if last.2 == lines && last.1 == state.ready {
        return;
    }
    let (Ok(body), Some(font)) = (body.single(), font) else {
        return;
    };
    let font = &font.0;
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        for (l, tone) in &lines {
            let (size, color) = match tone {
                Tone::Head => (15.0, GOLD),
                Tone::Plain => (13.0, Color::WHITE),
                Tone::Done => (12.0, Color::srgb(0.3, 1.0, 0.3)),
                Tone::Dim => (12.0, GREY),
            };
            b.spawn(text(font, l, size, color));
        }
        widgets::text_button(
            b,
            font,
            ReadyButton,
            if state.ready { "Not Ready" } else { "Ready" },
        );
    });
    last.1 = state.ready;
    last.2 = lines;
}

/// Ready pressed: ready, or not any more.
pub(super) fn clicks(
    buttons: Query<&Interaction, (Changed<Interaction>, With<ReadyButton>)>,
    net: Res<Net>,
    mut state: ResMut<InnState>,
) {
    if buttons.iter().any(|i| *i == Interaction::Pressed) {
        let now = ready_after_press(state.ready, net.is_client(), net.welcomed());
        if state.ready != now {
            state.ready = now;
        }
    }
}

/// Whether the board asks for the raid to start: it is open, we are ready, we are not joined (the
/// host starts for a joined player), and the party is ready.
fn board_starts(open: bool, ready: bool, is_client: bool, party_ready: bool) -> bool {
    open && ready && !is_client && party_ready
}

/// Alone and ready, or hosting with everyone present ready: the raid is asked for. A joined
/// player waits for their host's start.
pub(super) fn start(
    open: Res<BoardOpen>,
    net: Res<Net>,
    state: Res<InnState>,
    mut start: ResMut<StartRequest>,
) {
    if board_starts(open.0, state.ready, net.is_client(), net.party_ready(true)) {
        if !start.0 {
            start.0 = true;
        }
    } else if !state.ready && start.0 {
        // Ready taken back: a start held for a friend's loadout goes with it.
        start.0 = false;
    }
}

/// The inn (or Nightfall) left: the board is shut.
pub(super) fn forget(mut open: ResMut<BoardOpen>) {
    open.0 = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::ExitPlan;

    #[test]
    fn the_door_and_the_tables_intel() {
        let at = |x: f32| {
            Some(HeroPose {
                pos: V2::new(x, 0.0),
                facing: V2::new(1.0, 0.0),
                z: 0.0,
            })
        };
        assert!(at_door(at(1.5), V2::ZERO, 2.0));
        assert!(!at_door(at(2.5), V2::ZERO, 2.0));
        assert!(!at_door(None, V2::ZERO, 2.0));
        let roll = RaidRoll {
            seed: 1,
            entry: 0,
            exits: vec![
                ExitPlan {
                    open: true,
                    opens_at: 0.0,
                },
                ExitPlan {
                    open: true,
                    opens_at: 1080.0,
                },
                ExitPlan {
                    open: false,
                    opens_at: 0.0,
                },
            ],
        };
        assert_eq!(
            intel_marks(&roll),
            vec![
                ExitMark {
                    exit: 0,
                    open: true,
                    opens_in: Some(0.0)
                },
                ExitMark {
                    exit: 1,
                    open: false,
                    opens_in: Some(1080.0)
                },
                ExitMark {
                    exit: 2,
                    open: false,
                    opens_in: None
                },
            ]
        );
    }

    #[test]
    fn stepping_out_of_the_ring_takes_ready_back() {
        // In the ring: open, ready kept. Stepping out: shut, ready gone.
        assert_eq!(door_step(false, true, false), (true, false));
        assert_eq!(door_step(true, true, true), (true, true));
        assert_eq!(door_step(true, false, true), (false, false));
        // Never at the door: a Ready set another way (autoplay) stands.
        assert_eq!(door_step(false, false, true), (false, true));
    }

    #[test]
    fn the_board_starts_the_raid_only_for_a_ready_party_not_joined() {
        // Alone: ready at the door starts; not ready, or away from it, does not.
        assert!(board_starts(true, true, false, true));
        assert!(!board_starts(true, false, false, true));
        assert!(!board_starts(false, true, false, true));
        // Hosting: one friend not ready stops it (a pick not yet echoed is `start_run`'s hold).
        assert!(!board_starts(true, true, false, false));
        // Joined: never, whatever the party.
        assert!(!board_starts(true, true, true, true));
    }

    #[test]
    fn a_joined_player_readies_only_once_welcomed() {
        assert!(ready_after_press(false, false, false), "alone or hosting");
        assert!(!ready_after_press(false, true, false), "joined, not yet welcomed");
        assert!(ready_after_press(false, true, true));
        assert!(!ready_after_press(true, true, true), "taking it back");
        assert!(!ready_after_press(true, true, false), "taking it back, always");
    }

    #[test]
    fn the_board_lists_the_contract_the_intel_and_the_party() {
        let content = super::super::shipped_content();
        let inn = super::super::shipped_inn();
        let mut p = Profile::default();
        let c = (0..inn.contracts.len())
            .find(|&c| p.can_take(c, &inn).is_ok())
            .expect("a contract can be taken");
        p.take_contract(c, &inn).unwrap();
        let roll = RaidRoll::standard(&content, 1);
        let party = vec![("You".to_owned(), true), ("Friend  ·  Mage".to_owned(), false)];
        // Without the map table, no exits are shown.
        let plain = board_lines(&content, &inn, &p, Some(&roll), &party, None);
        assert_eq!(plain[0].0, format!("The raid: {}", content.map.name));
        assert!(plain.iter().any(|(l, _)| *l == format!("Contract: {}", inn.contracts[c].name)));
        assert!(!plain.iter().any(|(l, _)| l.starts_with("The map table")));
        assert!(plain.contains(&("  You  ·  ready".to_owned(), Tone::Done)));
        assert!(plain.contains(&("  Friend  ·  Mage".to_owned(), Tone::Dim)));
        // With it, they are.
        p.inn.intel = true;
        let seen = board_lines(&content, &inn, &p, Some(&roll), &party, Some("Wait".into()));
        assert!(seen.iter().any(|(l, _)| l.starts_with("The map table")));
        assert!(seen.iter().any(|(l, _)| l.ends_with(": open")));
        assert_eq!(seen.last().unwrap().0, "Wait");
        // No contract taken says so.
        let none = board_lines(&content, &inn, &Profile::default(), None, &party, None);
        assert!(none.iter().any(|(l, _)| l.starts_with("No contract taken")));
    }
}
