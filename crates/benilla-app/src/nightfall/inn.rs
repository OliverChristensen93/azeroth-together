//! The inn between raids (spec §8.4): its own scene, the hub `hub.ron` places on a map, at a warm
//! evening light and no fog, a contrast to the night outside. Our hero walks it with WoW's own
//! controller, kept inside its rooms as a raid keeps them inside its map, dressed in the loadout
//! they will bring; friends who joined walk it too, drawn where their clients say they stand.

use std::collections::{HashMap, HashSet};

use benilla_protocol::messages::ObjectFields;
use bevy::prelude::*;
use nightfall_sim::{inventory_of, Bounds, Content, EquipSlot, HeroPose, PlayerId, V2};

use super::gear::pieces_worn;
use super::menu::Selection;
use super::session::{self, InnState, Net};
use super::stash::Stash;
use super::units::{self, UnitKind};
use super::{content, dress, inn};
use crate::creature_anim::{move_flags, MovementState};
use crate::names::NameCache;
use crate::net::{GuidIndex, ObjectStore, SelfGuid};

/// Inn bodies' guid serials: ours, then each friend's after it.
const INN_SERIAL_BASE: u64 = 0x40;

/// Everything the inn drew, gone when it is left.
#[derive(Component)]
pub(super) struct InnBody;

/// A body just made, its gear still to write once its store exists.
#[derive(Component)]
pub(super) struct NeedsDress(Vec<(u16, u32)>);

/// Which entities draw us and our friends in the inn.
#[derive(Resource, Default)]
pub(super) struct InnViews {
    me: Option<Entity>,
    /// The class and the worn gear ours was dressed in.
    dressed: Option<(usize, Vec<(EquipSlot, u16)>)>,
    /// Each friend's body and the class, race and sex it was made for.
    friends: HashMap<PlayerId, (Entity, Look)>,
    /// The look of each friend whose body could not be made (nothing to show for their race and
    /// sex): not tried again every frame, only when they pick another look.
    refused: HashMap<PlayerId, Look>,
    /// The seat our hero was put down for (see [`seat_spot`]).
    seat: PlayerId,
}

/// What a friend's body was made for: a class key, a race and a sex.
type Look = (String, u8, u8);

/// Whether a body made for `have` still shows a hero of this class, race and sex.
fn same_look(have: &Look, class: &str, race: u8, sex: u8) -> bool {
    have.0 == class && have.1 == race && have.2 == sex
}

/// Whether a body for this look was already tried for the friend and could not be made.
fn was_refused(refused: &HashMap<PlayerId, Look>, player: PlayerId, look: &Look) -> bool {
    refused.get(&player) == Some(look)
}

/// How far from the hub's spawn point the other seats stand, in yards.
const SEAT_RING: f32 = 2.0;

/// Where seat `player` appears: the first on the spawn point itself, the others around it on a
/// ring, so a party does not stand inside one another. A seat that would fall outside the rooms
/// moves in toward the spawn point until it is inside.
fn seat_spot(spawn: V2, bounds: &Bounds, player: PlayerId) -> V2 {
    if player == 0 {
        return spawn;
    }
    let angle = f32::from(player - 1) * std::f32::consts::TAU / 3.0 + 0.5;
    let mut radius = SEAT_RING;
    while radius > 0.25 {
        let spot = spawn + V2::from_angle(angle, radius);
        if bounds.contains(spot) {
            return spot;
        }
        radius *= 0.75;
    }
    spawn
}

/// A friend's body a step toward where they stand: quick enough to keep up at ten poses a
/// second, smooth between them.
pub(super) fn glide(from: Vec3, to: Vec3, dt: f32) -> Vec3 {
    from.lerp(to, (dt * 8.0).min(1.0))
}

/// The engine's settle hold on our hero, as after a teleport: gravity off until the streamer says
/// the world is there, or the stall budget runs out (the deadline must be set, or the hold ends at
/// once).
fn hold(player: &mut crate::player::Player, time: &Time) {
    player.settling = true;
    player.settle_since = time.elapsed_secs();
    player.settle_deadline = time.elapsed_secs() + crate::player::SETTLE_TIMEOUT;
}

/// Whether a hero at height `y` has fallen through the inn's floor (`spawn_z` is the floor's): the
/// ground floor is flat and there is no cellar, so well below it is the terrain under the building,
/// which a hero reaches when the building's collision arrives late.
fn fell_through(y: f32, spawn_z: f32) -> bool {
    y < spawn_z - 8.0
}

/// The worn gear the loadout shows.
fn loadout_worn(stash: &Stash, content: &Content) -> Vec<(EquipSlot, u16)> {
    let (inv, _) = inventory_of(&stash.profile.loadout, content);
    EquipSlot::ALL
        .iter()
        .filter_map(|&s| inv.equip[s.index()].map(|st| (s, st.item)))
        .collect()
}

/// Our hero in the inn, made again whenever the class or the loadout's gear changes (where they
/// stand kept); each friend's body from the lobby, gone when they leave it.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync(
    mut commands: Commands,
    selection: Res<Selection>,
    stash: Res<Stash>,
    net: Res<Net>,
    mut views: ResMut<InnViews>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    mut items: ResMut<crate::items::Items>,
    mut self_guid: ResMut<SelfGuid>,
    mut names: ResMut<NameCache>,
    mut index: ResMut<GuidIndex>,
    mut player: ResMut<crate::player::Player>,
    time: Res<Time>,
) {
    let Some(cc) = char_create.as_deref() else {
        return;
    };
    let content = content();
    let inn = inn();
    let level = content.rules.hero_level;
    let worn = loadout_worn(&stash, &content);
    let want = (selection.class, worn.clone());
    // Where a seat stands in the inn: the spawn point for the first, a ring around it for the rest.
    let s = inn.place.spawn;
    let spot_of = |seat: PlayerId| {
        let g = seat_spot(s.ground(), &inn.place.bounds, seat);
        units::wow_to_bevy([g.x, g.y, s.z])
    };
    let seat = net.my_player();
    // The host seats a joiner after they arrived: if they have not moved from where they first
    // stood, they move to their own place rather than stand inside the host.
    if seat != views.seat {
        if views.me.is_some() && player.active && player.pos.distance(spot_of(views.seat)) < 0.5 {
            player.pos = spot_of(seat);
        }
        views.seat = seat;
    }
    if views.dressed.as_ref() != Some(&want) {
        let spawn = spot_of(seat);
        let at = if views.me.is_some() && player.active {
            player.pos
        } else {
            spawn
        };
        let had = views.me.take();
        if let Some(e) = had {
            index.0.retain(|_, v| *v != e);
            commands.entity(e).despawn();
        }
        let class = &content.classes[selection.class];
        let yaw = units::yaw_of(V2::from_angle(inn.place.facing.to_radians(), 1.0));
        let Some(e) = dress::spawn_player_body(
            &mut commands,
            &mut items,
            cc,
            class,
            (class.race, class.sex),
            INN_SERIAL_BASE,
            level,
            (units::FACTION_PLAYER, false),
            at,
            yaw,
        ) else {
            return;
        };
        let outfit_of = |cid: u8| dress::pieces(cc.0.start_outfit(class.race, cid, class.sex), &[]);
        let base = outfit_of(class.class_id);
        let (pieces, empty) = pieces_worn(&worn, &content, &base, outfit_of);
        let mut pairs = dress::wear(&mut items, &pieces);
        pairs.extend(
            empty
                .into_iter()
                .map(|w| (dress::visible_entry_field(w), 0)),
        );
        commands.entity(e).insert((InnBody, NeedsDress(pairs)));
        let guid = units::unit_guid(UnitKind::Player, INN_SERIAL_BASE);
        index.0.insert(guid, e);
        names.insert_player(guid, session::hero_name(), None);
        self_guid.0 = Some(guid);
        // A hero first put down holds, gravity off, until the ground around it has streamed in
        // (the engine's own settle, as after a teleport), or it would fall through the world.
        if had.is_none() {
            hold(&mut player, &time);
        }
        player.active = true;
        player.pos = at;
        views.me = Some(e);
        views.dressed = Some(want);
    }
    let friends = net.friends();
    for f in &friends {
        let (Some(pose), Some(ci)) = (f.pose, content.class(&f.hero.class)) else {
            continue;
        };
        let look = (f.hero.class.clone(), f.hero.race, f.hero.sex);
        if was_refused(&views.refused, f.player, &look) {
            continue;
        }
        match views.friends.get(&f.player) {
            Some((_, have)) if same_look(have, &f.hero.class, f.hero.race, f.hero.sex) => continue,
            // They picked another class (or took the seat of one who left): draw them anew.
            Some(&(old, _)) => {
                index.0.retain(|_, v| *v != old);
                commands.entity(old).despawn();
                views.friends.remove(&f.player);
            }
            None => {}
        }
        let class = &content.classes[ci];
        let serial = INN_SERIAL_BASE + 1 + u64::from(f.player);
        let at = units::ground_to_bevy(pose.pos, pose.z);
        let Some(e) = dress::spawn_player_body(
            &mut commands,
            &mut items,
            cc,
            class,
            (f.hero.race, f.hero.sex),
            serial,
            level,
            (units::FACTION_PLAYER, false),
            at,
            units::yaw_of(pose.facing),
        ) else {
            views.refused.insert(f.player, look);
            continue;
        };
        commands.entity(e).insert(InnBody);
        let guid = units::unit_guid(UnitKind::Player, serial);
        index.0.insert(guid, e);
        names.insert_player(guid, f.hero.name.clone(), None);
        views.friends.insert(
            f.player,
            (e, (f.hero.class.clone(), f.hero.race, f.hero.sex)),
        );
    }
    views
        .refused
        .retain(|p, _| friends.iter().any(|f| f.player == *p));
    views.friends.retain(|p, (e, _)| {
        let keep = friends.iter().any(|f| f.player == *p);
        if !keep {
            index.0.retain(|_, v| v != e);
            commands.entity(*e).despawn();
        }
        keep
    });
}

/// A body made last frame gets its gear now that its store exists.
pub(super) fn dress_new(
    mut commands: Commands,
    mut bodies: Query<(Entity, &NeedsDress, &mut ObjectStore)>,
) {
    for (e, d, mut store) in &mut bodies {
        store.0.merge(ObjectFields::from_pairs(&d.0));
        commands.entity(e).remove::<NeedsDress>();
    }
}

/// Where WoW's controller left our hero, kept inside the inn's rooms, as the lobby's pose.
pub(super) fn feed(
    views: Res<InnViews>,
    real: Res<Time<Real>>,
    mut bodies: Query<&mut Transform>,
    mut player: ResMut<crate::player::Player>,
    mut state: ResMut<InnState>,
    time: Res<Time>,
    mut last_trace: Local<f32>,
) {
    let inn = inn();
    let Some(mut tf) = views
        .me
        .and_then(|e| bodies.get_mut(e).ok())
        .filter(|_| player.active)
    else {
        state.pose = None;
        return;
    };
    // Fallen through the world (the settle hold timed out on a slow load): back to the spawn, held
    // again until the ground is there.
    if fell_through(tf.translation.y, inn.place.spawn.z) {
        let s = inn.place.spawn;
        let back = units::wow_to_bevy([s.x, s.y, s.z]);
        tf.translation = back;
        player.pos = back;
        hold(&mut player, &time);
    }
    let at = V2::new(-tf.translation.z, -tf.translation.x);
    let pos = inn.place.bounds.clamp(at);
    if pos != at {
        let back = units::ground_to_bevy(pos, tf.translation.y);
        tf.translation = back;
        player.pos = back;
    }
    if super::run::trace_pos() && real.elapsed_secs() - *last_trace >= 2.0 {
        *last_trace = real.elapsed_secs();
        let t = tf.translation;
        info!(
            "nightfall: hero in the inn at ({:.1}, {:.1}, {:.1})",
            -t.z, -t.x, t.y
        );
    }
    let yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
    state.pose = Some(HeroPose {
        pos,
        facing: V2::new(yaw.cos(), yaw.sin()),
        z: tf.translation.y,
    });
}

/// Each friend's body glides to where their client says they stand, turned their way, walking
/// while it moves.
pub(super) fn place_friends(
    net: Res<Net>,
    views: Res<InnViews>,
    time: Res<Time<Real>>,
    mut bodies: Query<(&mut Transform, &mut MovementState)>,
) {
    let dt = time.delta_secs().max(1e-3);
    for f in net.friends() {
        let (Some(pose), Some(&(e, _))) = (f.pose, views.friends.get(&f.player)) else {
            continue;
        };
        let Ok((mut tf, mut mv)) = bodies.get_mut(e) else {
            continue;
        };
        let before = tf.translation;
        tf.translation = glide(before, units::ground_to_bevy(pose.pos, pose.z), dt);
        let speed = before.distance(tf.translation) / dt;
        let turn = Quat::from_rotation_y(units::yaw_of(pose.facing));
        tf.rotation = tf.rotation.slerp(turn, (dt * 8.0).min(1.0));
        let flags = if speed > 0.3 { move_flags::FORWARD } else { 0 };
        if (mv.speed - speed).abs() > 0.05 || mv.flags != flags {
            mv.speed = speed;
            mv.flags = flags;
        }
    }
}

/// Leaving the inn (a raid begins, or Nightfall is left): everything it drew goes, and the
/// controller lets go.
pub(super) fn leave(
    mut commands: Commands,
    bodies: Query<Entity, With<InnBody>>,
    mut views: ResMut<InnViews>,
    mut index: ResMut<GuidIndex>,
    mut self_guid: ResMut<SelfGuid>,
    mut player: ResMut<crate::player::Player>,
) {
    let gone: HashSet<Entity> = bodies.iter().collect();
    for &e in &gone {
        commands.entity(e).despawn();
    }
    index.0.retain(|_, e| !gone.contains(e));
    *views = InnViews::default();
    self_guid.0 = None;
    player.active = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_seat_in_the_inn_has_its_own_spot_inside_the_rooms() {
        let inn = super::super::shipped_inn();
        let spawn = inn.place.spawn.ground();
        let spots: Vec<V2> = (0..super::super::session::MAX_PLAYERS as u8)
            .map(|p| seat_spot(spawn, &inn.place.bounds, p))
            .collect();
        assert_eq!(spots[0], spawn, "the first seat stands on the spawn point");
        for (i, a) in spots.iter().enumerate() {
            assert!(inn.place.bounds.contains(*a), "seat {i} is inside");
            for b in &spots[i + 1..] {
                assert!(a.distance(*b) > 1.0, "seats stand apart: {a:?} {b:?}");
            }
        }
        // A spawn point at a wall still seats everyone inside, moved in toward the point.
        let wall = V2::new(spawn.x, spawn.y);
        let tight = Bounds::new(vec![
            V2::new(wall.x - 0.5, wall.y - 0.5),
            V2::new(wall.x + 0.5, wall.y - 0.5),
            V2::new(wall.x + 0.5, wall.y + 0.5),
            V2::new(wall.x - 0.5, wall.y + 0.5),
        ])
        .unwrap();
        for p in 0..4 {
            assert!(tight.contains(seat_spot(wall, &tight, p)));
        }
    }

    #[test]
    fn a_friends_body_that_could_not_be_made_is_not_tried_again_until_the_look_changes() {
        let mut refused: HashMap<PlayerId, Look> = HashMap::new();
        let look: Look = ("mage".into(), 99, 1);
        assert!(!was_refused(&refused, 1, &look));
        refused.insert(1, look.clone());
        assert!(was_refused(&refused, 1, &look));
        assert!(!was_refused(&refused, 2, &look), "another friend");
        assert!(
            !was_refused(&refused, 1, &("mage".into(), 1, 1)),
            "a new look"
        );
    }

    #[test]
    fn a_friend_glides_toward_where_they_stand_and_stops_on_it() {
        let to = Vec3::new(8.0, 0.0, 0.0);
        let one = glide(Vec3::ZERO, to, 1.0 / 60.0);
        assert!(one.x > 0.0 && one.x < 8.0);
        assert_eq!(glide(Vec3::ZERO, to, 1.0), to);
    }

    #[test]
    fn a_hero_far_below_the_floor_has_fallen_through() {
        assert!(!fell_through(56.0, 56.3));
        assert!(!fell_through(50.0, 56.3), "a step down is not a fall");
        assert!(fell_through(44.3, 56.3), "the terrain under the inn is");
        assert!(fell_through(-100.0, 56.3));
    }

    #[test]
    fn a_friend_is_drawn_anew_when_their_class_race_or_sex_changes() {
        let have: Look = ("mage".into(), 1, 0);
        assert!(same_look(&have, "mage", 1, 0));
        assert!(!same_look(&have, "rogue", 1, 0));
        assert!(!same_look(&have, "mage", 2, 0));
        assert!(!same_look(&have, "mage", 1, 1));
    }
}
