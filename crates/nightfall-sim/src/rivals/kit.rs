//! A rival's ability by tag (spec §7.4): its bot reads its kit by tag (opener, builder,
//! finisher, nuke, control, escape, heal) and at each moment of a fight presses the first that
//! fits, in a fixed order: a heal when hurt, an escape when worse off, an opener from stealth on
//! the unaware, a hold on a free target, the big move when it reaches, a finisher on enough
//! combo points, else a nuke or a builder. Pure: the bot gives it the moment and what is ready,
//! and the fight's own checks still decide whether the press goes off. A full hero kit reads the
//! same way, so a kind given the whole rogue bar plays it by its tags.

use crate::{AbilityDef, Aim, Content, Effect, Facing, HeroFight, RivalRules, Tag};

/// One ability on a rival's bar, as its bot reads it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KitSlot {
    /// The WoW action id it sits on, which a press names.
    pub(crate) slot: u8,
    /// Index into the content's abilities.
    pub(crate) ability: usize,
    pub(crate) tags: Vec<Tag>,
    /// Yards it reaches its target from: its range, or for one aimed at the rival itself its
    /// radius (its telegraph's reach when it has one).
    pub(crate) reach: f32,
    /// The kind's telegraphed big move.
    pub(crate) big: bool,
    /// Only from behind the target.
    pub(crate) behind: bool,
    /// Only from stealth, or only on a target not alerted to the rival.
    pub(crate) opener: bool,
    /// It needs a clear line to its target. The fight asks for one at every range, melee
    /// included (`in_sight_of`), so every slot of a kit does.
    pub(crate) sight: bool,
}

/// What a rival knows of its moment in a fight.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Moment {
    /// Yards to the target, centre to centre.
    pub(crate) distance: f32,
    /// The target's size, added to every reach (`CombatRules::reach`).
    pub(crate) size: f32,
    /// Its own share of health.
    pub(crate) health: f32,
    pub(crate) stealthed: bool,
    /// The target is not alerted to it, so an opener may land (spec §5.4).
    pub(crate) unaware: bool,
    /// The target is held already.
    pub(crate) held: bool,
    /// It stands behind the target.
    pub(crate) behind: bool,
    /// A clear line to the target.
    pub(crate) in_sight: bool,
    /// Combo points on the target.
    pub(crate) combo: u8,
}

/// What a rival presses: the bar slot, its ability, and the tag it was picked by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Choice {
    pub(crate) slot: u8,
    pub(crate) ability: usize,
    pub(crate) why: Tag,
}

/// The kit of `class`: every ability on its bar but Stealth's toggle (its bot presses Stealth by
/// count, as a player does), in bar order; `big` marks the big move.
pub(crate) fn kit_of(class: usize, big: Option<usize>, content: &Content) -> Vec<KitSlot> {
    content.classes[class]
        .kit
        .iter()
        .filter(|&&(_, a)| !content.abilities[a].has(|e| matches!(e, Effect::Stealth)))
        .map(|&(slot, a)| {
            let def = &content.abilities[a];
            KitSlot {
                slot,
                ability: a,
                tags: def.tags.clone(),
                reach: reach_of(def),
                big: Some(a) == big,
                behind: def.facing == Facing::Behind,
                opener: def.needs.stealth || def.needs.unaware,
                sight: true,
            }
        })
        .collect()
}

/// How far an ability reaches its target from.
fn reach_of(a: &AbilityDef) -> f32 {
    match a.aim {
        Aim::Me => a.telegraph.map_or(a.radius, |t| t.shape.reach()),
        Aim::Enemy | Aim::Ground => a.range,
    }
}

/// Whether `fight` may press `ability` now as far as its clocks and power go: up, no cast under
/// way, its own cooldown and the global one done, and power enough. The fight's checks (range,
/// facing, sight) come after, as for a player's press.
pub(crate) fn ready(fight: &HeroFight, ability: usize, a: &AbilityDef) -> bool {
    fight.up()
        && fight.casting.is_none()
        && !fight
            .cooldowns
            .iter()
            .any(|c| usize::from(c.ability) == ability)
        && !(a.gcd > 0.0 && fight.gcd > 0.0)
        && fight.power + 1e-3 >= a.cost
}

/// The ability to press now, if any fits (see the module's order).
pub(crate) fn choose(
    kit: &[KitSlot],
    m: &Moment,
    r: &RivalRules,
    ready: impl Fn(usize) -> bool,
) -> Option<Choice> {
    let reaches = |k: &KitSlot| {
        m.distance <= k.reach + m.size && (!k.sight || m.in_sight) && (!k.behind || m.behind)
    };
    let pick = |want: &dyn Fn(&KitSlot) -> bool, why: Tag| {
        kit.iter()
            .find(|k| want(k) && ready(k.ability))
            .map(|k| Choice {
                slot: k.slot,
                ability: k.ability,
                why,
            })
    };
    let has = |k: &KitSlot, t: Tag| k.tags.contains(&t);
    if m.health < r.heal_below {
        if let Some(c) = pick(&|k| has(k, Tag::Heal), Tag::Heal) {
            return Some(c);
        }
    }
    if m.health < r.flee_health {
        if let Some(c) = pick(&|k| has(k, Tag::Escape), Tag::Escape) {
            return Some(c);
        }
    }
    if m.stealthed && m.unaware {
        if let Some(c) = pick(&|k| has(k, Tag::Opener) && reaches(k), Tag::Opener) {
            return Some(c);
        }
    }
    if !m.held {
        if let Some(c) = pick(
            &|k| has(k, Tag::Control) && !k.opener && reaches(k),
            Tag::Control,
        ) {
            return Some(c);
        }
    }
    if let Some(c) = pick(&|k| k.big && reaches(k), Tag::Nuke) {
        return Some(c);
    }
    if m.combo >= r.finisher_at {
        if let Some(c) = pick(&|k| has(k, Tag::Finisher) && reaches(k), Tag::Finisher) {
            return Some(c);
        }
    }
    if let Some(c) = pick(
        &|k| has(k, Tag::Nuke) && !k.big && !k.opener && reaches(k),
        Tag::Nuke,
    ) {
        return Some(c);
    }
    pick(
        &|k| has(k, Tag::Builder) && !k.opener && reaches(k),
        Tag::Builder,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(slot: u8, ability: usize, tags: &[Tag], reach: f32) -> KitSlot {
        KitSlot {
            slot,
            ability,
            tags: tags.to_vec(),
            reach,
            big: false,
            behind: false,
            opener: false,
            sight: false,
        }
    }

    /// A simplified kit: a nuke, a control, a heal, the big move and an escape.
    fn kit() -> Vec<KitSlot> {
        vec![
            slot(1, 0, &[Tag::Nuke], 5.0),
            slot(2, 1, &[Tag::Control], 8.0),
            slot(3, 2, &[Tag::Heal], 0.0),
            KitSlot {
                big: true,
                ..slot(4, 3, &[Tag::Nuke], 6.0)
            },
            slot(5, 4, &[Tag::Escape], 0.0),
        ]
    }

    fn near() -> Moment {
        Moment {
            distance: 2.0,
            size: 1.5,
            health: 1.0,
            in_sight: true,
            ..Moment::default()
        }
    }

    fn r() -> RivalRules {
        RivalRules::default()
    }

    fn picked(kit: &[KitSlot], m: Moment, ready: &[usize]) -> Option<u8> {
        choose(kit, &m, &r(), |a| ready.contains(&a)).map(|c| c.slot)
    }

    #[test]
    fn a_hurt_rival_heals_and_one_worse_off_without_a_heal_runs() {
        let all = [0, 1, 2, 3, 4];
        let hurt = Moment {
            health: 0.4,
            ..near()
        };
        assert_eq!(picked(&kit(), hurt, &all), Some(3));
        let worse = Moment {
            health: 0.2,
            ..near()
        };
        assert_eq!(picked(&kit(), worse, &[0, 1, 3, 4]), Some(5));
        assert_eq!(
            choose(&kit(), &worse, &r(), |a| a == 4).map(|c| c.why),
            Some(Tag::Escape)
        );
    }

    #[test]
    fn a_free_target_is_held_and_a_held_one_takes_the_big_move() {
        let all = [0, 1, 2, 3, 4];
        assert_eq!(picked(&kit(), near(), &all), Some(2));
        let held = Moment {
            held: true,
            ..near()
        };
        assert_eq!(picked(&kit(), held, &all), Some(4));
        assert_eq!(picked(&kit(), held, &[0, 1, 2, 4]), Some(1));
    }

    #[test]
    fn nothing_out_of_reach_is_pressed() {
        // 10 yards: past the control's 8 + 1.5, the big move's 6 + 1.5 and the nuke's 5 + 1.5.
        let far = Moment {
            distance: 10.0,
            ..near()
        };
        assert_eq!(picked(&kit(), far, &[0, 1, 2, 3, 4]), None);
        let closer = Moment {
            distance: 9.0,
            ..near()
        };
        assert_eq!(picked(&kit(), closer, &[0, 1, 2, 3, 4]), Some(2));
    }

    #[test]
    fn nothing_ready_is_nothing_pressed() {
        assert_eq!(picked(&kit(), near(), &[]), None);
    }

    #[test]
    fn an_opener_only_from_stealth_on_the_unaware() {
        let mut k = kit();
        k.push(KitSlot {
            opener: true,
            ..slot(74, 5, &[Tag::Opener], 5.0)
        });
        let ready = [0, 1, 2, 3, 4, 5];
        let sneaking = Moment {
            stealthed: true,
            unaware: true,
            ..near()
        };
        assert_eq!(picked(&k, sneaking, &ready), Some(74));
        let seen = Moment {
            stealthed: true,
            ..near()
        };
        assert_ne!(picked(&k, seen, &ready), Some(74));
        let open = Moment {
            unaware: true,
            ..near()
        };
        assert_ne!(picked(&k, open, &ready), Some(74));
    }

    #[test]
    fn a_finisher_on_enough_combo_points_else_a_builder() {
        let k = vec![
            slot(2, 0, &[Tag::Builder], 5.0),
            slot(3, 1, &[Tag::Finisher], 5.0),
        ];
        assert_eq!(picked(&k, Moment { combo: 2, ..near() }, &[0, 1]), Some(2));
        assert_eq!(picked(&k, Moment { combo: 3, ..near() }, &[0, 1]), Some(3));
    }

    #[test]
    fn behind_only_waits_for_the_back_and_a_slot_that_needs_sight_needs_a_clear_line() {
        let k = vec![
            KitSlot {
                behind: true,
                ..slot(1, 0, &[Tag::Nuke], 5.0)
            },
            KitSlot {
                sight: true,
                ..slot(2, 1, &[Tag::Nuke], 30.0)
            },
        ];
        assert_eq!(picked(&k, near(), &[0, 1]), Some(2));
        assert_eq!(
            picked(
                &k,
                Moment {
                    behind: true,
                    ..near()
                },
                &[0, 1]
            ),
            Some(1)
        );
        let blind = Moment {
            in_sight: false,
            distance: 20.0,
            ..near()
        };
        assert_eq!(picked(&k, blind, &[0, 1]), None);
    }
}
