//! Every raid a little different (spec §8.1): from the raid's seed, which exits open and when,
//! which one may close as the hunt lands (§7.2), the weather, and which signature monsters are
//! out (§7.3). The map stays the same, so knowing it pays; the same seed rolls the same raid.

use crate::combat::Dice;
use crate::raid::seed_for;
use crate::{Content, ExitDef, ExitPlan, RaidRoll};

/// The stream of the raid's seed the roll's dice come from, so they never follow the loot's (which
/// come from streams 0, 1, 2 ...). The seed is hashed ([`seed_for`]), so seeds that differ in one
/// bit roll different raids.
const ROLL_STREAM: u64 = 0x6E16_47F0_11ED_0005;

impl RaidRoll {
    /// This raid as the dice fall for `seed`. The late exits (Darkshire's cart) open within
    /// `cart_jitter` of `cart_opens_at`; a pinned exit is open from the start; every other one is
    /// shut at `shut`, else late (within `late_window`) at `late`, else open from the start;
    /// whatever falls, one exit is open from the start and `open_least` at some time. At
    /// `hunt_close`, one exit open by the hunt that is not late or pinned closes as it lands,
    /// never the last. The weather by its weights; the monsters each at its odds, between
    /// `monsters_out` of them, and every `always` one. The rival parties out, by the
    /// rivals' rules (spec §7.4).
    pub fn roll(content: &Content, seed: u64) -> Self {
        let x = &content.rules.night.exits;
        let cart = content.rules.run.cart_opens_at;
        let mut dice = Dice::new(seed_for(seed, ROLL_STREAM));
        let mut roll = Self::standard(content, seed);
        for (plan, def) in roll.exits.iter_mut().zip(&content.map.exits) {
            *plan = if def.late {
                ExitPlan {
                    open: true,
                    opens_at: (cart + dice.between(-x.cart_jitter, x.cart_jitter)).max(0.0),
                }
            } else if def.pinned {
                ExitPlan {
                    open: true,
                    opens_at: 0.0,
                }
            } else if dice.chance(x.shut) {
                ExitPlan {
                    open: false,
                    opens_at: 0.0,
                }
            } else if dice.chance(x.late) {
                ExitPlan {
                    open: true,
                    opens_at: dice.between(x.late_window.0, x.late_window.1),
                }
            } else {
                ExitPlan {
                    open: true,
                    opens_at: 0.0,
                }
            };
        }
        keep_enough_open(
            &mut roll.exits,
            &content.map.exits,
            usize::from(x.open_least),
        );
        roll.closes = hunt_closing(&roll.exits, content, &mut dice);
        roll.weather = pick_weather(content, &mut dice);
        roll.monsters = pick_monsters(content, &mut dice);
        // The rivals' own dice, so the rest of the roll stays as it was.
        roll.rivals = crate::rivals::pick_rivals(content, seed);
        roll
    }
}

/// One exit open from the start (the first that is not late, when every such one fell shut or
/// late), and shut ones opened in order until `least` open at some time.
fn keep_enough_open(plans: &mut [ExitPlan], defs: &[ExitDef], least: usize) {
    let open_now = ExitPlan {
        open: true,
        opens_at: 0.0,
    };
    if !plans.iter().any(|p| p.open && p.opens_at <= 0.0) {
        if let Some(i) = defs.iter().position(|d| !d.late) {
            plans[i] = open_now;
        }
    }
    let least = least.min(plans.len());
    let mut open = plans.iter().filter(|p| p.open).count();
    for p in plans.iter_mut() {
        if open >= least {
            break;
        }
        if !p.open {
            *p = open_now;
            open += 1;
        }
    }
}

/// The exit the hunt closes as it lands, if the dice say one does: one open by then and not
/// late or pinned, while another is open at some time, so it is never the last way out.
fn hunt_closing(plans: &[ExitPlan], content: &Content, dice: &mut Dice) -> Option<(u8, f32)> {
    let n = &content.rules.night;
    let at = n.stages.iter().find(|s| s.hunt)?.at;
    if !dice.chance(n.exits.hunt_close) {
        return None;
    }
    let open = plans.iter().filter(|p| p.open).count();
    let can: Vec<usize> = plans
        .iter()
        .zip(&content.map.exits)
        .enumerate()
        .filter(|(_, (p, d))| p.open && p.opens_at <= at && !d.late && !d.pinned)
        .map(|(i, _)| i)
        .collect();
    if open < 2 || can.is_empty() {
        return None;
    }
    let i = can[((dice.unit() * can.len() as f32) as usize).min(can.len() - 1)];
    Some((i as u8, at))
}

/// A weather by weight; 0 when the night has none (the atmosphere's own fog).
fn pick_weather(content: &Content, dice: &mut Dice) -> u8 {
    let w = &content.rules.night.weather;
    let total: f32 = w.iter().map(|w| w.weight).sum();
    let mut x = dice.unit() * total;
    for (i, def) in w.iter().enumerate() {
        if def.weight > 0.0 && x < def.weight {
            return i as u8;
        }
        x -= def.weight;
    }
    w.iter().rposition(|d| d.weight > 0.0).unwrap_or(0) as u8
}

/// The signature monsters out: every `always` one, and of the rest each whose roll comes up at
/// its odds, topped up or cut down to `monsters_out` (the likeliest kept), in order.
fn pick_monsters(content: &Content, dice: &mut Dice) -> Vec<u8> {
    let (lo, hi) = content.rules.night.monsters_out;
    // A key under 1 comes up at its odds; the lowest keys are the likeliest, so a raid short of
    // `lo` takes the next lowest and one over `hi` drops the highest.
    let mut keyed: Vec<(f32, usize)> = content
        .monsters
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.always && m.odds > 0.0)
        .map(|(i, m)| (dice.unit() / m.odds, i))
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    let rolled = keyed.iter().filter(|(k, _)| *k < 1.0).count();
    let take = rolled.clamp(usize::from(lo).min(keyed.len()), usize::from(hi));
    let mut out: Vec<u8> = content
        .monsters
        .iter()
        .enumerate()
        .filter(|(_, m)| m.always)
        .map(|(i, _)| i as u8)
        .collect();
    out.extend(keyed.iter().take(take).map(|&(_, i)| i as u8));
    out.sort_unstable();
    out
}
