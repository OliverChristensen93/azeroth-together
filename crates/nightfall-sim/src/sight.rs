//! What each hero sees (spec §4.3): the enemies within the fog and in line of sight from the
//! hero's eyes to their chests, or from a teammate's within `share_sight_within` yards (a friend
//! at your shoulder calls out what they see). The host sends each player theirs, with a snapshot
//! that holds only the enemies near their own hero. Sub-project 3 hides the others from the
//! interface (the `Concealed` marker), and enemies' sight of heroes will use the same query.

use crate::{Enemy, Hero, PlayerId, Snapshot, World, V3};

/// How high a unit's eyes are over its floor, yards.
pub const EYE_HEIGHT: f32 = 1.7;
/// How high the point seen of a unit is over its floor: its chest.
pub const CHEST_HEIGHT: f32 = 1.2;
/// Yards past the view distance an enemy a client already draws stays drawn, so one at the edge
/// does not blink in and out. The host sends each player the enemies this far past the view, or
/// a joined client would have nothing to keep.
pub const KEEP_EXTRA: f32 = 15.0;

impl World {
    /// The ids of the enemies `player`'s hero sees, in the world's order; none for a player
    /// without a hero.
    pub fn seen_by(&self, player: PlayerId) -> Vec<u32> {
        let Some(me) = self.hero(player) else {
            return Vec::new();
        };
        let rules = &self.content().rules;
        let share = rules.world.share_sight_within;
        let fog = rules.atmosphere.fog_end;
        let watchers: Vec<&Hero> = self
            .heroes
            .iter()
            .filter(|h| h.player == player || h.pos.distance(me.pos) <= share)
            .collect();
        self.enemies
            .iter()
            .filter(|e| watchers.iter().any(|h| self.sees(h, e, fog)))
            .map(|e| e.id)
            .collect()
    }

    /// `player`'s snapshot: every hero, and the enemies within the view distance of their hero
    /// and [`KEEP_EXTRA`] past it (all of them for a player without one), so a big map's enemies
    /// never crowd a line and the client's margin for those it draws has them.
    pub fn snapshot_for(&self, player: PlayerId) -> Snapshot {
        let near = self.hero(player).map(|h| h.pos);
        let reach = self.content().rules.atmosphere.view_distance + KEEP_EXTRA;
        Snapshot {
            elapsed: self.elapsed,
            heroes: self.heroes.clone(),
            enemies: self
                .enemies
                .iter()
                .filter(|e| near.is_none_or(|p| p.distance(e.pos) <= reach))
                .cloned()
                .collect(),
        }
    }

    /// Whether `hero` sees `enemy`: within `range`, and nothing solid between the hero's eyes and
    /// its chest. Without a map (a client's mirror, a bake that failed) nothing is known to stand
    /// between them.
    ///
    /// A ray is only cast between two points on the map's grid, and only once they are within
    /// `range` of each other, so no ray is ever long; a unit off the grid (or not at a number) is
    /// unseen rather than walked towards across millions of columns that hold nothing.
    // The distance check is a negated comparison on purpose, so a NaN fails it.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn sees(&self, hero: &Hero, enemy: &Enemy, range: f32) -> bool {
        // Written so that a distance that is not a number fails it.
        if !(hero.pos.distance(enemy.pos) <= range) {
            return false;
        }
        match self.map() {
            None => true,
            Some(map) => {
                let grid = map.grid();
                grid.column_of(hero.pos).is_some()
                    && grid.column_of(enemy.pos).is_some()
                    && map.line_of_sight(
                        V3::at(hero.pos, hero.z + EYE_HEIGHT),
                        V3::at(enemy.pos, enemy.z + CHEST_HEIGHT),
                    )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::heightmap::testmap::{flat, house};
    use crate::{Content, ContentSources, Enemy, HeroPose, PlayerId, PlayerInput, World, V2};

    const CLASSES: &str = r#"[
        (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 135.0, speed: 7.0),
    ]"#;
    const MAP: &str = r#"(key: "test", name: "Test", map: 0,
        bounds: [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
        entries: [(5.0, 5.0, 0.0)])"#;
    const ENEMIES: &str = r#"[
        (key: "skeleton", name: "Skeleton", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
    ]"#;

    /// A sentry at each point, ids 1, 2, … in order; fog at 40 yd, drawn to `view` yd; with
    /// `map`, a 50 yd square of flat ground with a closed house over [20, 30]² (the content's
    /// bounds reach past it, to 100 yd).
    fn world(posts: &[(f32, f32)], view: f32, map: bool) -> World {
        let spawns: Vec<String> = posts
            .iter()
            .map(|(x, y)| {
                format!(
                    r#"(place: "Post", kind: "skeleton", behaviour: Sentry(at: ({x:.1}, {y:.1}), facing: 0.0))"#
                )
            })
            .collect();
        let spawns = format!("[{}]", spawns.join(", "));
        let rules = format!(
            r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
                fog_color: (0.1, 0.1, 0.1), view_distance: {view:.1},
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#
        );
        let content = Content::from_ron(ContentSources {
            classes: CLASSES,
            map: MAP,
            rules: &rules,
            enemies: ENEMIES,
            spawns: &spawns,
        })
        .unwrap();
        let mut w = World::new(Arc::new(content));
        if map {
            let mut b = flat(50.0);
            house(
                &mut b,
                V2::new(20.0, 20.0),
                V2::new(30.0, 30.0),
                None,
                false,
            );
            w.attach_map(Arc::new(b.finish()));
        }
        w
    }

    /// `player`'s hero (made if need be) at `(x, y)` on the ground, from the next step.
    fn put(w: &mut World, player: PlayerId, x: f32, y: f32) {
        if w.hero(player).is_none() {
            w.add_hero(player, "rogue", "Hero", 1, 0).unwrap();
        }
        w.set_input(
            player,
            PlayerInput {
                pose: Some(HeroPose {
                    pos: V2::new(x, y),
                    facing: V2::new(1.0, 0.0),
                    z: 0.0,
                }),
            },
        );
    }

    #[test]
    fn an_enemy_behind_a_house_is_not_seen_and_one_in_the_open_is() {
        let mut w = world(&[(38.0, 25.0), (12.0, 40.0)], 120.0, true);
        put(&mut w, 0, 12.0, 25.0);
        w.step();
        assert_eq!(w.seen_by(0), vec![2]);
    }

    #[test]
    fn a_teammate_close_by_shares_what_they_see() {
        let mut w = world(&[(38.0, 40.0)], 120.0, true);
        put(&mut w, 0, 12.0, 25.0);
        put(&mut w, 1, 12.0, 34.0);
        w.step();
        assert_eq!(w.seen_by(0), vec![1]);
    }

    #[test]
    fn a_teammate_far_away_does_not() {
        let mut w = world(&[(38.0, 40.0)], 120.0, true);
        put(&mut w, 0, 12.0, 25.0);
        put(&mut w, 1, 12.0, 44.0);
        w.step();
        assert!(w.seen_by(0).is_empty());
        assert_eq!(w.seen_by(1), vec![1]);
    }

    #[test]
    fn nothing_past_the_fog_is_seen() {
        let mut w = world(&[(45.0, 45.0)], 120.0, true);
        put(&mut w, 0, 5.0, 5.0);
        w.step();
        assert!(w.seen_by(0).is_empty());
    }

    #[test]
    fn without_a_map_only_distance_counts() {
        let mut w = world(&[], 120.0, false);
        put(&mut w, 0, 12.0, 25.0);
        w.step();
        let mut s = w.snapshot();
        s.enemies.push(Enemy {
            id: 9,
            kind: 0,
            pos: V2::new(38.0, 25.0),
            z: 0.0,
            facing: V2::new(1.0, 0.0),
            hp: 900.0,
            max_hp: 900.0,
            speed: 0.0,
        });
        w.apply_snapshot(s);
        assert_eq!(w.seen_by(0), vec![9]);
    }

    #[test]
    fn a_players_snapshot_leaves_out_enemies_past_the_view_and_its_margin() {
        // The view cannot be shorter than the fog (40 yd): the second post is 60 yd away, past
        // the view and the 15 yd margin; the third 47 yd, past the view but inside the margin,
        // where a client keeps drawing an enemy it already draws.
        let mut w = world(&[(12.0, 10.0), (48.0, 48.0), (48.0, 25.0)], 40.0, true);
        put(&mut w, 0, 5.0, 5.0);
        w.step();
        let ids: Vec<u32> = w.snapshot_for(0).enemies.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![1, 3]);
        assert_eq!(w.snapshot().enemies.len(), 3);
        assert_eq!(w.snapshot_for(7).enemies.len(), 3);
        assert_eq!(w.snapshot_for(0).heroes, w.snapshot().heroes);
        assert_eq!(w.snapshot_for(0).elapsed, w.snapshot().elapsed);
    }

    #[test]
    fn a_snapshot_reaches_past_the_fog_to_the_view() {
        let mut w = world(&[(45.0, 45.0)], 120.0, true);
        put(&mut w, 0, 5.0, 5.0);
        w.step();
        assert_eq!(w.snapshot_for(0).enemies.len(), 1);
        assert!(w.seen_by(0).is_empty());
    }

    #[test]
    fn a_player_without_a_hero_sees_nothing() {
        let mut w = world(&[(12.0, 40.0)], 120.0, true);
        w.step();
        assert!(w.seen_by(7).is_empty());
    }

    #[test]
    fn a_hero_or_enemy_off_the_maps_grid_is_unseen() {
        // The content's bounds reach to 100 yd; the baked map only to 50.
        let mut w = world(&[(40.0, 5.0)], 120.0, true);
        put(&mut w, 0, 30.0, 5.0);
        w.step();
        assert_eq!(w.seen_by(0), vec![1]);
        // Thirty yards off in the open, but past the grid's edge: no ray is cast.
        w.enemies[0].pos = V2::new(60.0, 5.0);
        assert!(w.seen_by(0).is_empty());
        // And the same for a hero standing there.
        w.enemies[0].pos = V2::new(40.0, 5.0);
        put(&mut w, 0, 70.0, 5.0);
        w.step();
        assert!(w.seen_by(0).is_empty());
    }

    #[test]
    fn a_hero_that_is_not_where_a_number_says_sees_nothing_and_nothing_panics() {
        let mut w = world(&[(12.0, 40.0)], 120.0, true);
        put(&mut w, 0, 12.0, 25.0);
        w.step();
        assert_eq!(w.seen_by(0), vec![1]);
        let spots = [
            V2::new(f32::NAN, 25.0),
            V2::new(12.0, f32::NAN),
            V2::new(f32::INFINITY, 25.0),
            V2::new(f32::NEG_INFINITY, f32::INFINITY),
            V2::new(3.0e38, -3.0e38),
            V2::new(f32::MAX, f32::MAX),
        ];
        for pos in spots {
            w.heroes[0].pos = pos;
            assert!(w.seen_by(0).is_empty(), "{pos:?}");
            // The snapshot keeps no enemy near a hero who is nowhere.
            assert!(w.snapshot_for(0).enemies.is_empty(), "{pos:?}");
        }
        // A hero whose height is not a number sees nothing either.
        w.heroes[0].pos = V2::new(12.0, 25.0);
        w.heroes[0].z = f32::NAN;
        assert!(w.seen_by(0).is_empty());
        // Nor does an enemy that is nowhere get seen.
        w.heroes[0].z = 0.0;
        w.enemies[0].pos = V2::new(f32::NAN, 40.0);
        assert!(w.seen_by(0).is_empty());
        w.enemies[0].pos = V2::new(f32::MAX, 40.0);
        assert!(w.seen_by(0).is_empty());
    }

    #[test]
    fn a_mirror_with_a_hero_that_is_not_a_number_sees_nothing() {
        let mut w = world(&[], 120.0, false);
        put(&mut w, 0, 12.0, 25.0);
        w.step();
        let mut s = w.snapshot();
        s.heroes[0].pos = V2::new(f32::NAN, f32::NAN);
        s.enemies.push(Enemy {
            id: 9,
            kind: 0,
            pos: V2::new(38.0, 25.0),
            z: 0.0,
            facing: V2::new(1.0, 0.0),
            hp: 900.0,
            max_hp: 900.0,
            speed: 0.0,
        });
        w.apply_snapshot(s);
        assert!(w.seen_by(0).is_empty());
    }
}
