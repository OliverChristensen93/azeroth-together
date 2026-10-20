//! The inn's Playtest panel (the playtest kit): the rules the design leaves open and the knobs a
//! tester wants (what a death takes, the rivals' kits and whether they come, the signature
//! monsters, the night clock's speed, the weather) as a few buttons, each stepping its setting
//! round its values. The settings are [`nightfall_sim::Playtest`], saved in
//! `benilla-config/nightfall-playtest.txt` as Survivors' dev panel saves its own, and kept in
//! [`Net::playtest`]: the raids this player hosts or plays alone follow them, rolled again when
//! one changes so the lobby shows the raid that will be played. A joined player's panel says the
//! host's settings apply. The environment's test switches (`NIGHTFALL_WEATHER`,
//! `NIGHTFALL_MONSTERS`, `NIGHTFALL_RIVALS`) still win over the panel ([`super::night`]).
//!
//! The panel stands where the NPCs' panels do and gives way to them: opening one closes the
//! other.

use bevy::prelude::*;
use nightfall_sim::{
    next_of, Content, DeathLoss, MonstersOut, NightSpeed, Playtest, RivalKits, RivalsOut,
};

use super::npcs::InnPanel;
use super::session::Net;
use super::stash_panel::StashOpen;
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL};
use super::{content, Phase};
use crate::game_pick::GameMode;

/// Whether the Playtest panel is open (the inn bar's Playtest button toggles it).
#[derive(Resource, Default)]
pub(super) struct PlaytestOpen(pub bool);

#[derive(Component)]
pub(super) struct PlaytestRoot;

#[derive(Resource)]
pub(super) struct PlaytestFont(Handle<Font>);

/// A button of the panel: a setting to step, or Reset and Close.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Row {
    DeathLoss,
    RivalKits,
    Rivals,
    Monsters,
    NightSpeed,
    Weather,
    Reset,
    Close,
}

/// The settings' rows in the panel's order.
const SETTINGS: [Row; 6] = [
    Row::DeathLoss,
    Row::RivalKits,
    Row::Rivals,
    Row::Monsters,
    Row::NightSpeed,
    Row::Weather,
];

fn path() -> Option<std::path::PathBuf> {
    crate::local_state::home().map(|h| h.join("nightfall-playtest.txt"))
}

/// The saved settings; the defaults when there are none (or nowhere to read them).
pub(super) fn load() -> Playtest {
    path()
        .and_then(|p| crate::local_state::read_to_string(&p).ok())
        .map(|t| Playtest::from_text(&t))
        .unwrap_or_default()
}

fn save(p: &Playtest) {
    if let Some(path) = path() {
        let _ = crate::local_state::write_atomic(&path, &p.to_text());
    }
}

/// A death rule in words.
fn death_words(d: DeathLoss) -> &'static str {
    match d {
        DeathLoss::Everything => "everything is lost",
        DeathLoss::EverythingOneRecovery => "everything is lost, one recovery",
        DeathLoss::FoundOnly => "only what was found is lost",
    }
}

/// The button's text: the setting and its value now.
pub(super) fn caption(row: Row, p: &Playtest, content: &Content) -> String {
    match row {
        Row::DeathLoss => format!(
            "A death: {}",
            match p.death_loss {
                Some(d) => death_words(d).to_string(),
                None => format!(
                    "as rules.ron has it ({})",
                    death_words(content.rules.run.death_loss)
                ),
            }
        ),
        Row::RivalKits => format!(
            "Rival kits: {}",
            match p.rival_kits {
                RivalKits::Simplified => "simplified",
                RivalKits::Full => "full",
            }
        ),
        Row::Rivals => format!(
            "Rivals: {}",
            match p.rivals {
                RivalsOut::Rolled => "rolled",
                RivalsOut::Off => "none",
                RivalsOut::One => "always one party",
            }
        ),
        Row::Monsters => format!(
            "Signature monsters: {}",
            match p.monsters {
                MonstersOut::Rolled => "rolled",
                MonstersOut::All => "all of them",
                MonstersOut::None => "only the ones that always come",
            }
        ),
        Row::NightSpeed => format!(
            "Night clock: x{}",
            match p.night_speed {
                NightSpeed::X1 => 1,
                NightSpeed::X2 => 2,
                NightSpeed::X4 => 4,
            }
        ),
        Row::Weather => format!(
            "Weather: {}",
            match &p.weather {
                None => "rolled".to_string(),
                Some(key) => content
                    .rules
                    .night
                    .weather
                    .iter()
                    .find(|w| w.key == *key)
                    .map_or_else(|| format!("{key} (not in rules.ron)"), |w| w.name.clone()),
            }
        ),
        Row::Reset => "Reset all".into(),
        Row::Close => "Close".into(),
    }
}

/// What the setting does, under its button.
fn hint(row: Row) -> &'static str {
    match row {
        Row::DeathLoss => "What a dead hero's body holds for their friends and what they lose.",
        Row::RivalKits => {
            "Full: every rival plays the whole bar of its class, as the cutthroats do."
        }
        Row::Rivals => "Which parties come to the raid.",
        Row::Monsters => "Mor'Ladim, the trackers, the banshee, the riders, Stitches.",
        Row::NightSpeed => "The night's stages, exits and rivals come this much faster.",
        Row::Weather => "Replaces the raid's rolled weather.",
        Row::Reset | Row::Close => "",
    }
}

/// One press on `row`: its setting steps to the next value, Reset puts every setting back.
pub(super) fn press(row: Row, p: &mut Playtest, content: &Content) {
    match row {
        Row::DeathLoss => p.cycle_death_loss(),
        Row::RivalKits => p.rival_kits = next_of(&RivalKits::ALL, p.rival_kits),
        Row::Rivals => p.rivals = next_of(&RivalsOut::ALL, p.rivals),
        Row::Monsters => p.monsters = next_of(&MonstersOut::ALL, p.monsters),
        Row::NightSpeed => p.night_speed = next_of(&NightSpeed::ALL, p.night_speed),
        Row::Weather => p.cycle_weather(content),
        Row::Reset => *p = Playtest::default(),
        Row::Close => {}
    }
}

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands.spawn((
        PlaytestRoot,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            // Where the NPCs' panels stand: below the inn bar and WoW's player frame.
            top: Val::Px(200.0),
            width: Val::Px(400.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            max_height: Val::Vh(72.0),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(EDGE),
        GlobalZIndex(430),
        Visibility::Hidden,
    ));
    commands.insert_resource(PlaytestFont(font));
}

/// Shown at Nightfall's inn while the panel is open.
pub(super) fn show(
    mode: Res<State<GameMode>>,
    phase: Res<State<Phase>>,
    open: Res<PlaytestOpen>,
    mut roots: Query<&mut Visibility, With<PlaytestRoot>>,
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

/// This panel and an NPC's never stand together: the one opened last stays.
pub(super) fn exclusive(
    mut open: ResMut<PlaytestOpen>,
    mut panel: ResMut<InnPanel>,
    mut stash_open: ResMut<StashOpen>,
) {
    if open.0 && panel.0.is_some() {
        if open.is_changed() {
            panel.0 = None;
            stash_open.0 = false;
        } else {
            open.0 = false;
        }
    }
}

/// The panel's presses.
pub(super) fn clicks(
    buttons: Query<(&Interaction, &Row), Changed<Interaction>>,
    mut open: ResMut<PlaytestOpen>,
    mut net: ResMut<Net>,
) {
    if !open.0 {
        return;
    }
    let content = content();
    for (i, row) in &buttons {
        if *i != Interaction::Pressed {
            continue;
        }
        match row {
            Row::Close => open.0 = false,
            // A joined player has no say: the host's settings apply, and no button shows.
            _ if net.is_client() => {}
            _ => {
                let mut p = net.playtest.clone();
                press(*row, &mut p, &content);
                if p != net.playtest {
                    save(&p);
                    net.set_playtest(p);
                }
            }
        }
    }
}

/// The panel's buttons, built again whenever the settings, the role or the panel change.
pub(super) fn rebuild(
    mut commands: Commands,
    net: Res<Net>,
    open: Res<PlaytestOpen>,
    font: Option<Res<PlaytestFont>>,
    root: Query<Entity, With<PlaytestRoot>>,
    mut built: Local<Option<(Playtest, bool, bool)>>,
) {
    let (Ok(root), Some(font)) = (root.single(), font) else {
        return;
    };
    let key = (net.playtest.clone(), net.is_client(), open.0);
    if built.as_ref() == Some(&key) || !open.0 {
        return;
    }
    *built = Some(key);
    let font = &font.0;
    let content = content();
    let client = net.is_client();
    commands.entity(root).despawn_related::<Children>();
    commands.entity(root).with_children(|b| {
        b.spawn(text(font, "Playtest", 14.0, GOLD));
        if client {
            b.spawn(text(
                font,
                "You are joined: the host's playtest settings apply to this raid. Yours, \
                 saved on this computer, apply when you host or play alone.",
                11.0,
                GREY,
            ));
        } else {
            b.spawn(text(
                font,
                "For raids you host or play alone. A test switch set in the environment \
                 (NIGHTFALL_WEATHER, NIGHTFALL_MONSTERS, NIGHTFALL_RIVALS) still wins.",
                11.0,
                GREY,
            ));
            for row in SETTINGS {
                widgets::text_button(b, font, row, &caption(row, &net.playtest, &content));
                b.spawn(text(font, hint(row), 11.0, GREY));
            }
            widgets::text_button(
                b,
                font,
                Row::Reset,
                &caption(Row::Reset, &net.playtest, &content),
            );
        }
        widgets::text_button(b, font, Row::Close, "Close");
    });
}

/// The inn or Nightfall left: the panel is shut.
pub(super) fn forget(mut open: ResMut<PlaytestOpen>) {
    open.0 = false;
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn every_setting_steps_round_to_where_it_began() {
        let content = super::super::shipped_content();
        let mut p = Playtest::default();
        for row in SETTINGS {
            let before = p.clone();
            let mut seen = vec![caption(row, &p, &content)];
            loop {
                press(row, &mut p, &content);
                if p == before {
                    break;
                }
                seen.push(caption(row, &p, &content));
                assert!(seen.len() < 10, "{row:?} never comes round");
            }
            let mut distinct = seen.clone();
            distinct.sort();
            distinct.dedup();
            assert_eq!(
                distinct.len(),
                seen.len(),
                "{row:?}: each value reads differently"
            );
            assert!(seen.len() >= 2, "{row:?} has more than one value");
        }
        assert!(p.is_default());
    }

    #[test]
    fn reset_puts_every_setting_back() {
        let content = super::super::shipped_content();
        let mut p = Playtest::default();
        for row in SETTINGS {
            press(row, &mut p, &content);
        }
        assert!(!p.is_default());
        press(Row::Reset, &mut p, &content);
        assert!(p.is_default());
    }

    #[test]
    fn the_captions_say_the_values_in_words() {
        let content = super::super::shipped_content();
        let mut p = Playtest::default();
        assert_eq!(
            caption(Row::DeathLoss, &p, &content),
            "A death: as rules.ron has it (everything is lost, one recovery)"
        );
        press(Row::DeathLoss, &mut p, &content);
        assert_eq!(
            caption(Row::DeathLoss, &p, &content),
            "A death: everything is lost"
        );
        assert_eq!(caption(Row::Weather, &p, &content), "Weather: rolled");
        press(Row::Weather, &mut p, &content);
        assert_eq!(caption(Row::Weather, &p, &content), "Weather: Clear");
        p.weather = Some("hail".into());
        assert_eq!(
            caption(Row::Weather, &p, &content),
            "Weather: hail (not in rules.ron)"
        );
        press(Row::NightSpeed, &mut p, &content);
        assert_eq!(caption(Row::NightSpeed, &p, &content), "Night clock: x2");
    }

    #[test]
    fn changing_the_settings_plans_the_same_night_again_under_them() {
        let c = content_for_tests();
        let mut net = Net {
            solo_raid: Some(nightfall_sim::RaidRoll::standard(&c, 9)),
            ..Default::default()
        };
        let seed = net.solo_raid.as_ref().unwrap().seed;
        let p = Playtest {
            monsters: MonstersOut::None,
            weather: Some("rain".into()),
            ..Default::default()
        };
        net.set_playtest(p);
        let roll = net.solo_raid.clone().unwrap();
        assert_eq!(roll.seed, seed, "the same night");
        assert!(roll
            .monsters
            .iter()
            .all(|&m| c.monsters[usize::from(m)].always));
        assert_eq!(c.rules.night.weather[usize::from(roll.weather)].key, "rain");
        // Back to the defaults: the plain roll of that seed.
        net.set_playtest(Playtest::default());
        assert_eq!(
            net.solo_raid.unwrap(),
            nightfall_sim::RaidRoll::roll(&c, seed)
        );
    }

    fn content_for_tests() -> std::sync::Arc<Content> {
        let _ = super::super::CONTENT.set(super::super::shipped_content());
        content()
    }
}
