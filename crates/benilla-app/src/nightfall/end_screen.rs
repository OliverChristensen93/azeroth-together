//! The end of a raid on screen (spec §8.1): once we are out, a line that we wait for the others;
//! once every hero is out or dead, each hero's outcome, time and kills and what they carried out,
//! and Continue (alone or hosting) back to the menu. A joined player waits for their host.

use bevy::prelude::*;
use nightfall_sim::{Content, Fate, HeroOutcome, Life};

use super::exits::clock;
use super::run::{EndRun, Sim};
use super::session::{Net, Role};
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL};
use super::{content, Phase};

/// What became of a hero, in a line.
pub(super) fn fate_line(o: &HeroOutcome) -> String {
    let kills = match o.kills {
        1 => "1 kill".to_string(),
        n => format!("{n} kills"),
    };
    match &o.fate {
        Fate::Extracted { exit } => {
            format!("{}: out by {exit} at {}, {kills}", o.name, clock(o.time))
        }
        Fate::Died => format!("{}: died at {}, {kills}", o.name, clock(o.time)),
        Fate::Missing => format!("{}: missing at {}, {kills}", o.name, clock(o.time)),
    }
}

/// What a hero keeps, in a line: what they carried out (and the stacks that found no slot in it),
/// what they took back from their remains, and whether a body waits for them.
pub(super) fn carried_line(o: &HeroOutcome, content: &Content) -> String {
    let name = |item: u16, count: u16| {
        let n = content
            .items
            .get(usize::from(item))
            .map_or("something", |d| d.name.as_str());
        if count > 1 {
            format!("{n} ×{count}")
        } else {
            n.to_string()
        }
    };
    let kept: Vec<String> = o
        .carried
        .stacks()
        .iter()
        .map(|(_, s)| name(s.item, s.count))
        .chain(o.overflow.iter().map(|s| name(s.item, s.count)))
        .collect();
    let mut line = if kept.is_empty() {
        "Kept nothing".to_string()
    } else {
        format!("Kept: {}", kept.join(", "))
    };
    if !o.recovered.is_empty() {
        let back: Vec<String> = o.recovered.iter().map(|s| name(s.item, s.count)).collect();
        line.push_str(&format!("  ·  Recovered: {}", back.join(", ")));
    }
    if o.recovery.is_some() {
        line.push_str("  ·  Their body waits for them in their next raid");
    }
    line
}

/// The dimmed, full-screen layer the end screen is drawn on; hidden while the raid is played.
#[derive(Component)]
pub(super) struct EndRoot;

/// The panel in the middle of [`EndRoot`], whose lines [`show`] builds again when the screen changes.
#[derive(Component)]
pub(super) struct EndBody;

/// The Continue button (alone or hosting): back to the menu.
#[derive(Component)]
pub(super) struct EndContinue;

/// The WoW font, kept so [`show`] can set the lines without asking the asset server each time.
#[derive(Resource)]
pub(super) struct EndFont(Handle<Font>);

/// Builds the end screen's layers once, hidden: [`EndRoot`] with an empty [`EndBody`] in it.
pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            EndRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(430),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                EndBody,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(EDGE),
            ));
        });
    commands.insert_resource(EndFont(font));
}

/// What shows: every outcome once the raid is over; "waiting" once we are out and others are not;
/// nothing while we play.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Screen {
    None,
    /// The names still to come out or fall, and whether we are joined (then Abandon Raid leaves
    /// the session, which the line says).
    Waiting(Vec<String>, bool),
    Over(Vec<HeroOutcome>, bool),
}

/// The screen the raid asks for: `joined` is whether our host, not we, takes us on from here.
fn screen_of(sim: Option<&Sim>, joined: bool) -> Screen {
    let Some(s) = sim else {
        return Screen::None;
    };
    match &s.over {
        Some(all) => Screen::Over(all.clone(), joined),
        // Out of the raid (the heroes of a joined mirror are none before its first state, which is
        // not "out"), with others still to come out or fall.
        None if s.world.hero(s.me).is_none() && !s.world.heroes.is_empty() => Screen::Waiting(
            s.world
                .heroes
                .iter()
                .filter(|h| h.fight.life != Life::Dead)
                .map(|h| h.name.clone())
                .collect(),
            joined,
        ),
        None => Screen::None,
    }
}

/// The screen as the raid stands, built again whenever that changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    mut commands: Commands,
    sim: Option<Res<Sim>>,
    phase: Res<State<Phase>>,
    net: Res<Net>,
    font: Option<Res<EndFont>>,
    mut roots: Query<&mut Visibility, With<EndRoot>>,
    body: Query<Entity, With<EndBody>>,
    mut shown: Local<Option<Screen>>,
) {
    let content = content();
    let playing = sim.as_deref().filter(|_| *phase.get() == Phase::Playing);
    let screen = screen_of(playing, matches!(net.role, Role::Client(_)));
    if shown.as_ref() == Some(&screen) {
        return;
    }
    let (Ok(body), Some(font)) = (body.single(), font) else {
        return;
    };
    let want = if screen == Screen::None {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
    let font = &font.0;
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| match &screen {
        Screen::None => {}
        Screen::Waiting(names, joined) => {
            b.spawn(text(font, "You got out.", 22.0, GOLD));
            b.spawn(text(
                font,
                format!("Waiting for the others: {}", names.join(", ")),
                13.0,
                GREY,
            ));
            if *joined {
                b.spawn(text(
                    font,
                    "Abandon Raid now leaves the session.",
                    11.0,
                    GREY,
                ));
            }
        }
        Screen::Over(all, joined) => {
            b.spawn(text(font, "The raid is over", 22.0, GOLD));
            for o in all {
                b.spawn(text(font, fate_line(o), 14.0, Color::WHITE));
                b.spawn(text(font, carried_line(o, &content), 11.0, GREY));
            }
            if *joined {
                b.spawn(text(font, "Waiting for the host…", 12.0, GREY));
            } else {
                widgets::text_button(b, font, EndContinue, "Continue");
            }
        }
    });
    *shown = Some(screen);
}

/// Continue: alone or hosting, back to the menu, the party with us.
pub(super) fn continue_click(
    buttons: Query<&Interaction, (Changed<Interaction>, With<EndContinue>)>,
    mut net: ResMut<Net>,
    mut end: ResMut<EndRun>,
    sim: Res<Sim>,
) {
    // A test run (`NIGHTFALL_AUTOPLAY`) presses it itself, for the host or alone, so raids follow
    // one another.
    let auto = super::run::autoplay() && !net.is_client() && sim.over.is_some();
    if auto || buttons.iter().any(|i| *i == Interaction::Pressed) {
        net.broadcast_ended();
        end.0 = true;
    }
}

/// Out of the raid (extracted, the others still in): WoW's controller lets go of the body that
/// is no more. Before the first state a joined mirror has no heroes at all, which is not "out".
/// Once the raid is over the controller stays off too, so no death popup comes up over the end
/// screen.
pub(super) fn out_of_raid(sim: Res<Sim>, mut player: ResMut<crate::player::Player>) {
    let out = sim.world.hero(sim.me).is_none() && !sim.world.heroes.is_empty();
    if (out || sim.over.is_some()) && player.active {
        player.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::{Inventory, RaidRoll, Stack, World};

    /// A hosted raid of three heroes, players 0 (ours), 1 and 2.
    fn raid_of_three() -> Sim {
        let content = super::super::shipped_content();
        let mut world = World::new(content.clone());
        world
            .begin_raid(RaidRoll::standard(&content, 1))
            .expect("the standard roll fits the map");
        world.add_hero(0, "rogue", "Us", 1, 0).unwrap();
        world.add_hero(1, "mage", "Friend", 1, 0).unwrap();
        world.add_hero(2, "mage", "Fallen", 1, 0).unwrap();
        Sim::for_test(world)
    }

    #[test]
    fn no_heroes_is_no_screen_not_waiting() {
        let content = super::super::shipped_content();
        let sim = Sim::for_test(World::new(content));
        assert_eq!(screen_of(Some(&sim), false), Screen::None);
        assert_eq!(screen_of(None, false), Screen::None);
    }

    #[test]
    fn out_with_others_still_up_waits_for_the_living() {
        let mut sim = raid_of_three();
        assert_eq!(screen_of(Some(&sim), false), Screen::None);
        // Player 2 has fallen, we (0) have walked out: only 1 is waited for.
        sim.world.heroes[2].fight.life = Life::Dead;
        sim.world.heroes.retain(|h| h.player != 0);
        assert_eq!(
            screen_of(Some(&sim), false),
            Screen::Waiting(vec!["Friend".to_string()], false)
        );
        // A joined player is told that Abandon leaves the session.
        assert_eq!(
            screen_of(Some(&sim), true),
            Screen::Waiting(vec!["Friend".to_string()], true)
        );
    }

    #[test]
    fn a_finished_raid_shows_every_outcome() {
        let mut sim = raid_of_three();
        sim.over = Some(vec![outcome(Fate::Missing, 0)]);
        assert_eq!(
            screen_of(Some(&sim), false),
            Screen::Over(vec![outcome(Fate::Missing, 0)], false)
        );
    }

    fn outcome(fate: Fate, kills: u32) -> HeroOutcome {
        HeroOutcome {
            player: 0,
            name: "Valeera".into(),
            fate,
            time: 754.0,
            kills,
            carried: Inventory::default(),
            overflow: Vec::new(),
            recovered: Vec::new(),
            recovery: None,
            slain: Vec::new(),
        }
    }

    #[test]
    fn a_fate_reads_as_a_line() {
        let out = outcome(
            Fate::Extracted {
                exit: "the bridge".into(),
            },
            3,
        );
        assert_eq!(
            fate_line(&out),
            "Valeera: out by the bridge at 12:34, 3 kills"
        );
        assert_eq!(
            fate_line(&outcome(Fate::Died, 1)),
            "Valeera: died at 12:34, 1 kill"
        );
        assert_eq!(
            fate_line(&outcome(Fate::Missing, 0)),
            "Valeera: missing at 12:34, 0 kills"
        );
    }

    #[test]
    fn a_dead_heros_line_says_their_body_waits() {
        let content = super::super::shipped_content();
        let mut out = outcome(Fate::Died, 2);
        out.recovery = Some(nightfall_sim::Recovery {
            map: "duskwood".into(),
            at: nightfall_sim::V3::new(1.0, 2.0, 3.0),
            items: Vec::new(),
        });
        assert_eq!(
            carried_line(&out, &content),
            "Kept nothing  ·  Their body waits for them in their next raid"
        );
        assert_eq!(fate_line(&out), "Valeera: died at 12:34, 2 kills");
    }

    #[test]
    fn stacks_with_no_slot_are_listed_as_kept_beside_the_recovered() {
        let content = super::super::shipped_content();
        let name = |i: usize| content.items[i].name.clone();
        let mut out = outcome(Fate::Missing, 0);
        out.overflow = vec![Stack::new(0, 1), Stack::new(1, 3)];
        out.recovered = vec![Stack::new(2, 2)];
        assert_eq!(
            carried_line(&out, &content),
            format!(
                "Kept: {}, {} ×3  ·  Recovered: {} ×2",
                name(0),
                name(1),
                name(2)
            )
        );
    }
}
