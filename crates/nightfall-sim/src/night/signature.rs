//! The signature monsters' ways (spec §7.3) the fight's rules do not already give: Mor'Ladim, who
//! stands while any hero looks at him and otherwise comes for the nearest; and the banshee's
//! wail, an enemy ability like any other whose `Alarm` effect wakes everything near. Stitches is
//! content (a road walk, telegraphed blows); the Dark Riders' eyes along the roads are the
//! senses'.

use crate::combat::{within_arc, Hold};
use crate::{Event, Hero, PlayerId, World, CHEST_HEIGHT, EYE_HEIGHT, V2, V3};

impl World {
    /// Whether `hero` looks at a unit at `at` (its floor `z`): up, within the fog, facing it
    /// within half of `cone` either side, and nothing solid between their eyes and its chest.
    fn looks_at(&self, hero: &Hero, at: V2, z: f32, fog: f32, cone: f32) -> bool {
        hero.fight.up()
            && hero.pos.distance(at) <= fog
            && within_arc(hero.facing, at - hero.pos, cone * 0.5)
            && self.map().is_none_or(|m| {
                m.line_of_sight(
                    V3::at(hero.pos, hero.z + EYE_HEIGHT),
                    V3::at(at, z + CHEST_HEIGHT),
                )
            })
    }

    /// Mor'Ladim's step: looked at by any hero, he is out of the fight (no step, no turn, no
    /// blow); else, unless something else holds him, he is after the nearest hero up within his
    /// range, seen or not.
    pub(crate) fn watch_morladim(&mut self) {
        let content = self.content().clone();
        let fog = self.fog().end;
        let cone = content.rules.night.look_cone;
        let r = &content.rules.senses.awareness;
        for i in 0..self.enemies.len().min(self.combat.enemies.len()) {
            let (kind, hp, at, z) = {
                let e = &self.enemies[i];
                (e.kind, e.hp, e.pos, e.z)
            };
            let Some(range) = content.enemies[kind].watched else {
                continue;
            };
            if hp <= 0.0 {
                continue;
            }
            if self
                .heroes
                .iter()
                .any(|h| self.looks_at(h, at, z, fog, cone))
            {
                self.combat.enemies[i].engage.hold = Hold::Out;
                // Out of the fight, he stands: no run animation, and a blow begun before he was
                // looked at is taken down, as a stun would.
                self.enemies[i].speed = 0.0;
                self.stop_enemy_cast(i);
                continue;
            }
            if self.combat.enemies[i].engage.hold == Hold::Out {
                continue;
            }
            let nearest = self
                .heroes
                .iter()
                .filter(|h| h.fight.up() && h.pos.distance(at) <= range)
                .min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)))
                .map(|h| (h.player, V3::at(h.pos, h.z)));
            if let (Some((player, spot)), Some(mind)) = (nearest, self.senses.minds.get_mut(i)) {
                mind.alert(player, spot, r);
            }
        }
    }

    /// Enemy `ei`'s wail landed on `player`'s hero (the banshee, spec §7.3): every other enemy
    /// within `radius` yards of her that is in the fight is after that hero where they stand,
    /// walls or not. None of them shouts on.
    pub(crate) fn alarm(&mut self, ei: usize, player: PlayerId, radius: f32) {
        let Some(at) = self.hero(player).map(|h| V3::at(h.pos, h.z)) else {
            return;
        };
        let content = self.content().clone();
        let r = &content.rules.senses.awareness;
        let from = self.enemies[ei].pos;
        for j in self.enemies_within(from, radius) {
            let out = self
                .combat
                .enemies
                .get(j)
                .is_some_and(|c| c.engage.hold == Hold::Out);
            if j == ei || out {
                continue;
            }
            if let Some(m) = self.senses.minds.get_mut(j) {
                if !m.is_alerted() {
                    m.alert(player, at, r);
                }
            }
        }
        self.events.push(Event::Alarm {
            enemy: self.enemies[ei].id,
            player,
        });
    }
}
