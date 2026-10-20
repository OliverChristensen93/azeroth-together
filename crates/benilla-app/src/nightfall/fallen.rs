//! A fallen player's own words (spec §8.1): a line low on the screen while their hero is down
//! ("You are down: 32 s."), being revived, dead, or reviving a friend; WoW's death popup kept
//! away, since there is no spirit to release; and, for test runs, `NIGHTFALL_FRAIL=<player>`,
//! which drops that hero 15 seconds into the run on the host, to see a fall and a revive.

use std::sync::OnceLock;

use benilla_ui::script::UiScript;
use bevy::prelude::*;
use nightfall_sim::{CombatRules, Hero, Life, PlayerId, DT};

use super::run::Sim;
use super::NightfallEntity;

/// Seconds into a run `NIGHTFALL_FRAIL`'s hero falls.
pub(super) const FRAIL_AT: f32 = 15.0;
/// Seconds between closing WoW's death popup again while dead.
const POPUP_EVERY: f32 = 0.5;
/// Closes WoW's death popup if it is up.
const HIDE_POPUP: &str =
    r#"if StaticPopup_Visible and StaticPopup_Visible("DEATH") then StaticPopup_Hide("DEATH") end"#;

/// What the line says for our hero, if anything: down (and how long they have), being revived,
/// dead, or reviving a downed friend within reach.
pub(super) fn status(me: &Hero, heroes: &[Hero], r: &CombatRules) -> Option<String> {
    match me.fight.life {
        Life::Downed { revive, .. } if revive > 0.0 => Some(format!(
            "Being revived... {:.0}%",
            (revive / r.revive_for * 100.0).min(100.0)
        )),
        Life::Downed { left, .. } => Some(format!(
            "You are down: {:.0} s. A friend who stands by you can get you up.",
            left.ceil()
        )),
        Life::Dead => Some("You are dead. Your body lies where you fell.".into()),
        Life::Up => heroes
            .iter()
            .find(|h| {
                h.player != me.player
                    && h.side == me.side
                    && matches!(h.fight.life, Life::Downed { .. })
                    && h.pos.distance(me.pos) <= r.revive_reach
            })
            .map(|h| format!("Reviving {}...", h.name)),
    }
}

/// Whether the step that brought the run to `elapsed` seconds crossed `at`.
pub(super) fn crossed(at: f32, elapsed: f32) -> bool {
    elapsed >= at && elapsed - DT < at
}

/// `NIGHTFALL_FRAIL`'s player, if set.
pub(super) fn frail_player() -> Option<PlayerId> {
    static PLAYER: OnceLock<Option<PlayerId>> = OnceLock::new();
    *PLAYER.get_or_init(|| std::env::var("NIGHTFALL_FRAIL").ok()?.trim().parse().ok())
}

/// The blow that drops a frail hero: all their health, no more. (Not "as much as there is": a
/// huge number would show in the combat text, and in the log, as it is.)
pub(super) fn frail_blow(hero: &Hero) -> f32 {
    hero.max_hp
}

/// The line's node.
#[derive(Component)]
pub(super) struct StatusLine;

/// The line follows our hero: made with the run (it goes with the run's other entities), shown
/// while there is something to say.
pub(super) fn status_line(
    mut commands: Commands,
    sim: Res<Sim>,
    assets: Res<AssetServer>,
    mut roots: Query<(&mut Visibility, &Children), With<StatusLine>>,
    mut texts: Query<&mut Text>,
) {
    let Ok((mut visibility, children)) = roots.single_mut() else {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    bottom: Val::Px(150.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                GlobalZIndex(300),
                Visibility::Hidden,
                StatusLine,
                NightfallEntity,
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(""),
                    TextFont {
                        font: crate::char_select::wow_font(&assets),
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.82, 0.0)),
                    TextShadow::default(),
                ));
            });
        return;
    };
    let said = sim
        .world
        .hero(sim.me)
        .and_then(|me| status(me, &sim.world.heroes, &sim.world.content().rules.combat));
    let want = if said.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != want {
        *visibility = want;
    }
    if let Some(said) = said {
        for &child in children {
            if let Ok(mut text) = texts.get_mut(child) {
                if text.0 != said {
                    text.0 = said.clone();
                }
            }
        }
    }
}

/// While our hero is dead, WoW's "Release Spirit" popup is closed: there is no spirit to release,
/// and its button would ask a server that is not there. Closed the moment they die, then again
/// every [`POPUP_EVERY`] seconds in case the interface raises it late; nothing runs while they
/// are not dead, and a new run (a hero who is up) starts it over.
pub(super) fn death_popup(
    script: Option<NonSend<UiScript>>,
    sim: Res<Sim>,
    real: Res<Time<Real>>,
    mut state: Local<(bool, f32)>,
) {
    let dead = sim
        .world
        .hero(sim.me)
        .is_some_and(|h| h.fight.life == Life::Dead);
    let (was_dead, last) = &mut *state;
    let now = real.elapsed_secs();
    let fell = dead && !*was_dead;
    *was_dead = dead;
    if !fell && (!dead || now - *last < POPUP_EVERY) {
        return;
    }
    *last = now;
    if let Some(script) = script {
        // A failing script would fail the same way every 0.5 s: said once.
        if let Err(e) = script.run(HIDE_POPUP) {
            warn_once!("nightfall: closing WoW's death popup failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::{HeroFight, Stance, V2};

    fn hero(player: PlayerId, x: f32, life: Life) -> Hero {
        Hero {
            player,
            class: 0,
            name: format!("Hero{player}"),
            race: 1,
            sex: 0,
            pos: V2::new(x, 0.0),
            z: 0.0,
            facing: V2::new(1.0, 0.0),
            hp: 0.0,
            max_hp: 1000.0,
            speed: 7.0,
            stance: Stance::Still,
            stealthed: false,
            visibility: 35.0,
            noise: 0.0,
            fight: HeroFight {
                life,
                ..HeroFight::default()
            },
            carry: Default::default(),
            side: nightfall_sim::Side::Party,
        }
    }

    #[test]
    fn the_line_says_how_a_hero_stands() {
        let r = CombatRules::default();
        let down = hero(
            0,
            0.0,
            Life::Downed {
                left: 31.2,
                revive: 0.0,
            },
        );
        assert_eq!(
            status(&down, &[], &r).unwrap(),
            "You are down: 32 s. A friend who stands by you can get you up."
        );
        let reviving = hero(
            0,
            0.0,
            Life::Downed {
                left: 20.0,
                revive: 2.5,
            },
        );
        assert_eq!(status(&reviving, &[], &r).unwrap(), "Being revived... 50%");
        assert!(status(&hero(0, 0.0, Life::Dead), &[], &r)
            .unwrap()
            .starts_with("You are dead"));
        let me = hero(1, 2.0, Life::Up);
        let friend = hero(
            0,
            0.0,
            Life::Downed {
                left: 20.0,
                revive: 0.5,
            },
        );
        assert_eq!(
            status(&me, &[friend.clone(), me.clone()], &r).unwrap(),
            "Reviving Hero0..."
        );
        let far = hero(1, 10.0, Life::Up);
        assert_eq!(status(&far, &[friend.clone(), far.clone()], &r), None);
        // A downed rival beside an up hero is no friend to revive.
        let mut foe = friend;
        foe.side = nightfall_sim::Side::Rival(0);
        assert_eq!(status(&me, &[foe, me.clone()], &r), None);
        // Up and alone: nothing to say.
        assert_eq!(status(&me, &[me.clone()], &r), None);
    }

    #[test]
    fn the_frail_hook_falls_once() {
        assert!(!crossed(FRAIL_AT, FRAIL_AT - DT));
        assert!(crossed(FRAIL_AT, FRAIL_AT + DT * 0.5));
        assert!(!crossed(FRAIL_AT, FRAIL_AT + DT * 1.5));
    }

    #[test]
    fn the_frail_blow_is_the_heros_health_not_a_huge_number() {
        let h = hero(0, 0.0, Life::Up);
        assert_eq!(frail_blow(&h), 1000.0);
        assert!(frail_blow(&h).is_finite() && frail_blow(&h) < 1e6);
    }

    /// A hosted run with one hero (player 0, a rogue) and a status line node already made, as
    /// the system makes it (so the test never asks for WoW's font).
    fn line_stage() -> (bevy::app::App, Entity, Entity) {
        let mut app = bevy::app::App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        let mut sim = nightfall_sim::World::new(super::super::shipped_content());
        sim.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        app.insert_resource(Sim::for_test(sim));
        let line = app
            .world_mut()
            .spawn((Node::default(), Visibility::Hidden, StatusLine))
            .id();
        let text = app.world_mut().spawn(Text::new("")).id();
        app.world_mut().entity_mut(line).add_child(text);
        (app, line, text)
    }

    fn run_line(app: &mut bevy::app::App) {
        use bevy::ecs::system::RunSystemOnce;
        app.world_mut().run_system_once(status_line).unwrap();
    }

    fn life(app: &mut bevy::app::App, life: Life) {
        app.world_mut().resource_mut::<Sim>().world.heroes[0]
            .fight
            .life = life;
    }

    #[test]
    fn the_line_shows_while_down_or_revived_and_goes_with_the_revive() {
        let (mut app, line, text) = line_stage();
        run_line(&mut app);
        assert_eq!(
            app.world().get::<Visibility>(line),
            Some(&Visibility::Hidden)
        );

        life(
            &mut app,
            Life::Downed {
                left: 30.0,
                revive: 0.0,
            },
        );
        run_line(&mut app);
        assert_eq!(
            app.world().get::<Visibility>(line),
            Some(&Visibility::Inherited)
        );
        assert!(app
            .world()
            .get::<Text>(text)
            .unwrap()
            .0
            .starts_with("You are down: 30 s"));

        life(
            &mut app,
            Life::Downed {
                left: 30.0,
                revive: 2.5,
            },
        );
        run_line(&mut app);
        assert_eq!(
            app.world().get::<Text>(text).unwrap().0,
            "Being revived... 50%"
        );

        life(&mut app, Life::Up);
        run_line(&mut app);
        assert_eq!(
            app.world().get::<Visibility>(line),
            Some(&Visibility::Hidden)
        );
    }

    /// A UiScript whose popup is up, and counts the times it is closed.
    fn popup_stage(life: Life) -> bevy::app::App {
        let mut app = bevy::app::App::new();
        app.add_plugins(MinimalPlugins);
        let mut sim = nightfall_sim::World::new(super::super::shipped_content());
        sim.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        sim.heroes[0].fight.life = life;
        app.insert_resource(Sim::for_test(sim));
        let script = UiScript::new().unwrap();
        script
            .run(
                r#"closed = 0
                StaticPopup_Visible = function(which) return which == "DEATH" and "popup" end
                StaticPopup_Hide = function(which) if which == "DEATH" then closed = closed + 1 end end"#,
            )
            .unwrap();
        app.world_mut().insert_non_send_resource(script);
        app
    }

    fn closed(app: &bevy::app::App) -> i32 {
        app.world()
            .non_send_resource::<UiScript>()
            .eval("return closed")
            .unwrap()
    }

    #[test]
    fn the_death_popup_is_closed_on_the_fall_not_every_frame_and_a_new_run_starts_without_it() {
        let mut app = popup_stage(Life::Up);
        let id = app.world_mut().register_system(death_popup);
        let frame = |app: &mut bevy::app::App| app.world_mut().run_system(id).unwrap();
        frame(&mut app);
        frame(&mut app);
        assert_eq!(closed(&app), 0, "alive: left alone");

        app.world_mut().resource_mut::<Sim>().world.heroes[0]
            .fight
            .life = Life::Dead;
        frame(&mut app);
        assert_eq!(closed(&app), 1, "closed the frame they die");
        for _ in 0..5 {
            frame(&mut app);
        }
        assert_eq!(closed(&app), 1, "not again within the same moments");

        // A new run: a new world, its hero up, then dead once more.
        let mut sim = nightfall_sim::World::new(super::super::shipped_content());
        sim.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        app.world_mut().insert_resource(Sim::for_test(sim));
        frame(&mut app);
        assert_eq!(closed(&app), 1, "the new run starts without it");
        app.world_mut().resource_mut::<Sim>().world.heroes[0]
            .fight
            .life = Life::Dead;
        frame(&mut app);
        assert_eq!(closed(&app), 2, "and its own death closes it once");
    }

    #[test]
    fn the_death_popup_is_closed_again_every_half_second_while_dead() {
        use std::time::Duration;

        let mut app = popup_stage(Life::Dead);
        let id = app.world_mut().register_system(death_popup);
        let frame = |app: &mut bevy::app::App, advance: f32| {
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs_f32(advance));
            app.world_mut().run_system(id).unwrap();
        };
        frame(&mut app, 0.0);
        assert_eq!(closed(&app), 1, "closed as they are found dead");
        frame(&mut app, POPUP_EVERY * 0.4);
        assert_eq!(closed(&app), 1, "too soon");
        frame(&mut app, POPUP_EVERY * 0.4);
        assert_eq!(closed(&app), 1, "still too soon (0.8 of the wait)");
        frame(&mut app, POPUP_EVERY * 0.4);
        assert_eq!(closed(&app), 2, "a half second on, closed again");
        frame(&mut app, POPUP_EVERY * 1.1);
        assert_eq!(closed(&app), 3, "and again");
    }

    #[test]
    fn the_frail_blow_downs_a_hero_who_is_up() {
        let mut world = nightfall_sim::World::new(super::super::shipped_content());
        world.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        let blow = frail_blow(world.hero(0).unwrap());
        assert!(matches!(world.hero(0).unwrap().fight.life, Life::Up));
        world.hurt_hero(0, blow);
        assert!(
            matches!(world.hero(0).unwrap().fight.life, Life::Downed { .. }),
            "{:?}",
            world.hero(0).unwrap().fight.life
        );
        assert!(world.hero(0).unwrap().hp.is_finite());
    }
}
