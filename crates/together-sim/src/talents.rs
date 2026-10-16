//! The talent web: progression between runs, one Path of Exile-style web shared by every class.
//!
//! The web is data ([`TalentWeb::from_ron`], `content/talents.ron`): nodes with an id, a place on
//! the web, links to their neighbours and the [`TalentEffect`]s they give. Each class has a start
//! node; a hero's allocation is the nodes it has taken, every one connected back to its class's
//! start through taken nodes, at most as many as its points. Other classes' starts are walls: no
//! path runs through them.
//!
//! What the web gives a hero is gathered into [`TalentBonuses`] when the run is built
//! (`World::apply_talents`). Plain stats go into the hero's [`crate::Stats`] like a tome's; the
//! rest (tag bonuses, notables' mechanics, keystones, steered card offers) the world reads where
//! they act: building a cast, landing a hit, dealing a hand.
//!
//! Points come from renown, earned by playing runs ([`renown_for_run`], [`PointRules`]), counted
//! per class or shared by all classes ([`TalentProfile::shared`]). A [`TalentProfile`] is what a
//! player keeps between sessions: renown, the setting, and each class's saved loadouts.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::augments::Augment;
use crate::spell::{Tag, Tags};
use crate::stats::Stat;

/// What a node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    /// A class's start: always taken by that class, a wall for every other.
    Start,
    /// A small bonus.
    Small,
    /// A bigger bonus or a mechanic of its own.
    Notable,
    /// A rule change that costs something.
    Keystone,
}

/// What a tag bonus raises on the spells with that tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpellBonus {
    /// More damage, 0.04 is +4%.
    Damage,
    /// More critical strike chance, 0.01 is +1%.
    Crit,
    /// Harder critical strikes, 0.1 makes them 210% instead of 200%.
    CritDamage,
    /// Shorter cooldowns, 0.04 takes 4% off.
    Cooldown,
    /// Wider areas.
    Area,
    /// Longer burns and ground areas.
    Duration,
    /// Longer reach.
    Range,
}

/// A rule a keystone changes, at a cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Keystone {
    /// A hit marks its target with its schools for a few seconds; your next hit on it deals more
    /// if it brings a school the mark lacks, less if it does not.
    ElementalEquilibrium,
    /// +40% damage, -50% max health.
    GlassCannon,
    /// Never a critical strike, but +20% damage.
    SteadyHand,
    /// More damage close up, less far away.
    PointBlank,
    /// Armor and health for speed.
    Juggernaut,
    /// Burns hit much harder, other hits softer.
    SlowBurn,
    /// Hits heal you, for less max health.
    SealOfLight,
}

/// Seconds an Elemental Equilibrium mark lasts, and what it adds or takes.
pub const EQUILIBRIUM_SECS: f32 = 4.0;
pub const EQUILIBRIUM: f32 = 0.25;
/// Point Blank: yards counted close, and far.
pub const POINT_BLANK_NEAR: f32 = 10.0;
pub const POINT_BLANK_FAR: f32 = 20.0;
/// Seal of Light: the share of each hit's damage that heals you.
pub const SEAL_HEAL: f32 = 0.01;
/// A healing notable's reach in yards.
pub const LEECH_REACH: f32 = 30.0;
/// Seconds between two casts of a crit streak notable.
pub const STREAK_ICD: f32 = 4.0;
/// Seconds a kill's move speed lasts, and the most it adds up to.
pub const KILL_HASTE_SECS: f32 = 3.0;
pub const KILL_HASTE_MAX: f32 = 0.2;
/// The most spells a pair notable counts.
pub const PAIR_MAX: u32 = 3;

impl Keystone {
    pub fn name(self) -> &'static str {
        match self {
            Keystone::ElementalEquilibrium => "Elemental Equilibrium",
            Keystone::GlassCannon => "Glass Cannon",
            Keystone::SteadyHand => "Steady Hand",
            Keystone::PointBlank => "Point Blank",
            Keystone::Juggernaut => "Juggernaut",
            Keystone::SlowBurn => "Slow Burn",
            Keystone::SealOfLight => "Seal of Light",
        }
    }

    /// What it does, a line each.
    pub fn lines(self) -> Vec<String> {
        let s = |t: &str| t.to_string();
        match self {
            Keystone::ElementalEquilibrium => vec![
                s("Your hits mark the target with their schools for 4 seconds."),
                s("Your hits on a marked target deal +25% damage if they bring a school the mark lacks, and -25% if they do not."),
            ],
            Keystone::GlassCannon => vec![s("+40% damage"), s("-50% max health")],
            Keystone::SteadyHand => vec![
                s("Your spells never roll critical strikes (augments that always crit still do)."),
                s("+20% damage"),
            ],
            Keystone::PointBlank => vec![
                s("+30% damage to enemies within 10 yards of you"),
                s("-20% damage to enemies farther than 20 yards"),
            ],
            Keystone::Juggernaut => vec![
                s("+25% armor"),
                s("+30% max health"),
                s("-15% move speed"),
            ],
            Keystone::SlowBurn => vec![
                s("The burns your hits leave deal double damage."),
                s("Your hits deal -25% damage."),
            ],
            Keystone::SealOfLight => vec![
                s("Your hits heal you for 1% of their damage."),
                s("-30% max health"),
            ],
        }
    }

    /// The stats it changes, as [`crate::Stats::add`] takes them.
    pub fn stats(self) -> &'static [(Stat, f32)] {
        match self {
            Keystone::GlassCannon => &[(Stat::Damage, 0.4), (Stat::Health, -0.5)],
            Keystone::Juggernaut => &[
                (Stat::Armor, 0.25),
                (Stat::Health, 0.3),
                (Stat::Speed, -0.15),
            ],
            Keystone::SealOfLight => &[(Stat::Health, -0.3)],
            _ => &[],
        }
    }
}

/// One thing a node gives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TalentEffect {
    /// A stat, as a tome adds it.
    Stat(Stat, f32),
    /// A bonus for every spell with the tag.
    Tagged(Tag, SpellBonus, f32),
    /// A bonus for spells with both tags (a Frostfire conversion is Fire and Frost).
    Hybrid(Tag, Tag, SpellBonus, f32),
    /// More damage for the first tag's spells for each spell of the second you know, and the
    /// other way round, up to [`PAIR_MAX`] each.
    Pair(Tag, Tag, f32),
    /// Critical strikes of spells with the tag deal this share of the hit again over 4 seconds.
    CritBurn(Tag, f32),
    /// Chills have this chance to Freeze.
    ChillFreeze(f32),
    /// More critical strike chance against Frozen enemies.
    FrozenCrit(f32),
    /// Fire hits on Chilled enemies, and Frost hits on Burning ones, deal this much more.
    Thermal(f32),
    /// More damage to enemies below 30% health.
    Execute(f32),
    /// Two critical strikes of the tag in a row cast your lowest-cooldown spell of the tag.
    CritStreak(Tag),
    /// Single-target hits heal the most wounded ally near you for this share of the damage.
    Leech(f32),
    /// Each kill gives this much move speed for a few seconds.
    KillHaste(f32),
    /// Spells with the tag are offered this much more often (0.3 is 30%).
    OfferWeight(Tag, f32),
    /// The augment is offered this much more often on augment screens (1.0 is twice).
    AugmentWeight(Augment, f32),
    /// The augment can also be offered on spells with any of these tags.
    Unlock(Augment, Vec<Tag>),
    /// More rerolls each run.
    Rerolls(u32),
    /// Gold at the start of each run.
    Gold(u32),
    Keystone(Keystone),
}

/// "Fire spells", "Damage over time spells".
fn tagged(tag: Tag) -> String {
    format!("{} spells", tag.name())
}

fn pct(v: f32) -> String {
    let p = v * 100.0;
    if (p - p.round()).abs() < 0.05 {
        format!("{:.0}%", p)
    } else {
        format!("{:.1}%", p)
    }
}

fn bonus_line(bonus: SpellBonus, amount: f32, whom: &str) -> String {
    let a = pct(amount);
    match bonus {
        SpellBonus::Damage => format!("+{a} damage with {whom}"),
        SpellBonus::Crit => format!("+{a} critical strike chance with {whom}"),
        SpellBonus::CritDamage => format!("+{a} critical strike damage with {whom}"),
        SpellBonus::Cooldown => format!("-{a} cooldown on {whom}"),
        SpellBonus::Area => format!("+{a} area with {whom}"),
        SpellBonus::Duration => format!("+{a} duration of {whom}' burns and ground areas"),
        SpellBonus::Range => format!("+{a} range with {whom}"),
    }
}

impl TalentEffect {
    /// What it does, as the web's tooltip says it: one line, or a keystone's several.
    pub fn lines(&self) -> Vec<String> {
        let one = |s: String| vec![s];
        match self {
            TalentEffect::Stat(stat, v) => one(stat.describe(*v)),
            TalentEffect::Tagged(tag, b, v) => one(bonus_line(*b, *v, &tagged(*tag))),
            TalentEffect::Hybrid(a, b, bonus, v) => one(bonus_line(
                *bonus,
                *v,
                &format!("spells that are both {} and {}", a.name(), b.name()),
            )),
            TalentEffect::Pair(a, b, v) => one(format!(
                "+{} {} damage for each {} spell you know, and +{} {} damage for each {} spell (up to {} each)",
                pct(*v),
                a.name(),
                b.name(),
                pct(*v),
                b.name(),
                a.name(),
                PAIR_MAX
            )),
            TalentEffect::CritBurn(tag, v) => one(format!(
                "{} critical strikes deal {} of the hit again over 4 seconds",
                tag.name(),
                pct(*v)
            )),
            TalentEffect::ChillFreeze(v) => one(format!("Your Chills have a {} chance to Freeze", pct(*v))),
            TalentEffect::FrozenCrit(v) => {
                one(format!("+{} critical strike chance against Frozen enemies", pct(*v)))
            }
            TalentEffect::Thermal(v) => one(format!(
                "Fire hits on Chilled enemies, and Frost hits on Burning enemies, deal +{} damage",
                pct(*v)
            )),
            TalentEffect::Execute(v) => one(format!("+{} damage to enemies below 30% health", pct(*v))),
            TalentEffect::CritStreak(tag) => one(format!(
                "Two {t} critical strikes in a row cast your lowest-cooldown {t} spell (at most every {} seconds)",
                STREAK_ICD,
                t = tag.name()
            )),
            TalentEffect::Leech(v) => one(format!(
                "Single-target hits heal the most wounded ally within {} yards (you too) for {} of the damage",
                LEECH_REACH,
                pct(*v)
            )),
            TalentEffect::KillHaste(v) => one(format!(
                "Each kill gives +{} move speed for {} seconds, up to +{}",
                pct(*v),
                KILL_HASTE_SECS,
                pct(KILL_HASTE_MAX)
            )),
            TalentEffect::OfferWeight(tag, v) => {
                one(format!("{} are offered {} more often", tagged(*tag), pct(*v)))
            }
            TalentEffect::AugmentWeight(a, v) => one(if (*v - 1.0).abs() < 1e-3 {
                format!("{} is offered twice as often on augment screens", a.name())
            } else {
                format!("{} is offered {} more often on augment screens", a.name(), pct(*v))
            }),
            TalentEffect::Unlock(a, tags) => {
                let names: Vec<&str> = tags.iter().map(|t| t.name()).collect();
                one(format!(
                    "{} can be offered on any {} spell",
                    a.name(),
                    names.join(" or ")
                ))
            }
            TalentEffect::Rerolls(n) => one(if *n == 1 {
                "+1 reroll each run".to_string()
            } else {
                format!("+{n} rerolls each run")
            }),
            TalentEffect::Gold(n) => one(format!("Start each run with {n} gold")),
            TalentEffect::Keystone(k) => k.lines(),
        }
    }

    /// A rough guess of how much stronger it makes a spell with `tags`, as a fraction of its
    /// damage: what the web's power budget is checked with (`docs/TALENT-WEB.md`). Survival and
    /// utility count for a little; widening offers counts for nothing.
    pub fn power(&self, tags: Tags) -> f32 {
        let bonus = |b: SpellBonus, v: f32| match b {
            SpellBonus::Damage | SpellBonus::Crit | SpellBonus::Cooldown => v,
            SpellBonus::CritDamage => v * 0.15,
            SpellBonus::Area => v * 0.5,
            SpellBonus::Duration => v * 0.3,
            SpellBonus::Range => v * 0.2,
        };
        match self {
            TalentEffect::Stat(stat, v) => match stat {
                Stat::Damage | Stat::Cooldown | Stat::Crit => *v,
                Stat::CritDamage => v * 0.15,
                Stat::Area => v * 0.5,
                Stat::Projectiles => v * 0.3,
                Stat::Speed | Stat::Armor => v * 0.5,
                Stat::Health => v * 0.25,
                Stat::Regen => v * 0.02,
                _ => v * 0.1,
            },
            TalentEffect::Tagged(t, b, v) if tags.has(*t) => bonus(*b, *v),
            TalentEffect::Hybrid(a, b, x, v) if tags.has(*a) && tags.has(*b) => bonus(*x, *v),
            TalentEffect::Pair(a, b, v) if tags.has(*a) || tags.has(*b) => v * 2.0,
            TalentEffect::CritBurn(t, v) if tags.has(*t) => v * 0.15,
            TalentEffect::ChillFreeze(v) if tags.has(Tag::Slow) => v * 0.5,
            TalentEffect::FrozenCrit(v) if tags.any(&[Tag::Frost, Tag::Freeze, Tag::Slow]) => {
                v * 0.3
            }
            TalentEffect::Thermal(v) if tags.has(Tag::Fire) || tags.has(Tag::Frost) => v * 0.2,
            TalentEffect::Execute(v) => v * 0.2,
            TalentEffect::CritStreak(t) if tags.has(*t) => 0.08,
            TalentEffect::Leech(_) => 0.05,
            TalentEffect::KillHaste(v) => v * 0.5,
            TalentEffect::Keystone(k) => match k {
                Keystone::GlassCannon => 0.4 - 0.25,
                Keystone::SteadyHand => 0.1,
                Keystone::PointBlank => 0.1,
                Keystone::ElementalEquilibrium => 0.1,
                Keystone::SlowBurn if tags.has(Tag::DamageOverTime) => 0.15,
                Keystone::SealOfLight => 0.05,
                _ => 0.0,
            },
            _ => 0.0,
        }
    }
}

/// One node of the web, as `talents.ron` writes it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TalentNode {
    /// Unique and stable: saved loadouts name nodes by it.
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    /// A start node's class key.
    #[serde(default)]
    pub class: Option<String>,
    /// Where it sits on the web: x right, y down, in web units (the screen draws them as pixels
    /// at zoom 1).
    pub pos: (f32, f32),
    /// Its neighbours by id; a link need only be written on one of its two ends.
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub effects: Vec<TalentEffect>,
    /// Its neighbours, by index, both ways: filled when the web is read.
    #[serde(skip)]
    pub adjacent: Vec<usize>,
}

impl TalentNode {
    /// What it gives, a line each.
    pub fn lines(&self) -> Vec<String> {
        self.effects.iter().flat_map(|e| e.lines()).collect()
    }

    /// The school it is about, for its colour: the first school one of its effects names.
    pub fn school(&self) -> Option<Tag> {
        self.effects.iter().find_map(|e| {
            let tags: &[Tag] = match e {
                TalentEffect::Tagged(t, ..)
                | TalentEffect::CritBurn(t, _)
                | TalentEffect::CritStreak(t)
                | TalentEffect::OfferWeight(t, _) => std::slice::from_ref(t),
                TalentEffect::Hybrid(a, ..) | TalentEffect::Pair(a, ..) => std::slice::from_ref(a),
                TalentEffect::ChillFreeze(_) | TalentEffect::FrozenCrit(_) => &[Tag::Frost],
                TalentEffect::Thermal(_) => &[Tag::Fire],
                _ => &[],
            };
            tags.iter().copied().find(|t| Tag::SCHOOLS.contains(t))
        })
    }
}

/// How renown turns into points: the renown from one level to the next is `first` at level 0,
/// `step` more each level after; a level is a point, up to `max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointRules {
    pub max: u32,
    pub first: u64,
    pub step: u64,
}

impl PointRules {
    /// Renown from `level` to the next.
    pub fn to_next(&self, level: u32) -> u64 {
        self.first + self.step * u64::from(level)
    }

    /// The level `renown` reaches, the renown past it, and what the next level needs; the level
    /// stops at `max`.
    pub fn level(&self, renown: u64) -> (u32, u64, u64) {
        let mut level = 0;
        let mut left = renown;
        while level < self.max && left >= self.to_next(level) {
            left -= self.to_next(level);
            level += 1;
        }
        (level, left, self.to_next(level))
    }
}

/// Renown a run earns its hero: 10 for each level the party reached past the first, 10 for each
/// minute survived, 1 for every 25 enemies the party slew.
pub fn renown_for_run(level: u32, elapsed: f32, kills: u32) -> u64 {
    let minutes = (elapsed / 60.0).floor().max(0.0) as u64;
    10 * u64::from(level.saturating_sub(1)) + 10 * minutes + u64::from(kills / 25)
}

#[derive(Deserialize)]
struct WebFile {
    points: PointRules,
    nodes: Vec<TalentNode>,
}

/// The whole web, read from `talents.ron`.
#[derive(Clone, Debug, PartialEq)]
pub struct TalentWeb {
    pub points: PointRules,
    pub nodes: Vec<TalentNode>,
}

impl TalentWeb {
    /// Read and check a web: ids unique, links to real nodes, one start per class, every node
    /// reachable from every start.
    pub fn from_ron(text: &str) -> Result<TalentWeb, String> {
        let file: WebFile = ron::from_str(text).map_err(|e| format!("talents.ron: {e}"))?;
        let mut web = TalentWeb {
            points: file.points,
            nodes: file.nodes,
        };
        let index = |web: &TalentWeb, id: &str| web.nodes.iter().position(|n| n.id == id);
        for (i, n) in web.nodes.iter().enumerate() {
            if index(&web, &n.id) != Some(i) {
                return Err(format!("talents.ron: two nodes are called \"{}\"", n.id));
            }
            if (n.kind == NodeKind::Start) != n.class.is_some() {
                return Err(format!(
                    "talents.ron: \"{}\": a start node, and only a start node, names a class",
                    n.id
                ));
            }
        }
        let mut edges = Vec::new();
        for (i, n) in web.nodes.iter().enumerate() {
            for link in &n.links {
                let j = index(&web, link).ok_or_else(|| {
                    format!("talents.ron: \"{}\" links to unknown \"{link}\"", n.id)
                })?;
                if i != j {
                    edges.push((i, j));
                }
            }
        }
        for (i, j) in edges {
            for (a, b) in [(i, j), (j, i)] {
                if !web.nodes[a].adjacent.contains(&b) {
                    web.nodes[a].adjacent.push(b);
                }
            }
        }
        let mut classes: Vec<&str> = Vec::new();
        for n in &web.nodes {
            if let Some(c) = &n.class {
                if classes.contains(&c.as_str()) {
                    return Err(format!("talents.ron: two starts for class \"{c}\""));
                }
                classes.push(c);
            }
        }
        for class in classes {
            let start = web.start_of(class).expect("just found");
            let reach = web.reach(start, &[]);
            if let Some(n) = web
                .nodes
                .iter()
                .enumerate()
                .find(|&(i, n)| n.kind != NodeKind::Start && !reach.contains(&i))
            {
                return Err(format!(
                    "talents.ron: \"{}\" cannot be reached from the {class} start",
                    n.1.id
                ));
            }
        }
        Ok(web)
    }

    /// The node called `id`.
    pub fn node(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    /// The start node of `class`.
    pub fn start_of(&self, class: &str) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.class.as_deref() == Some(class))
    }

    /// Whether a path may step onto node `i` from `start`'s side: never another class's start.
    fn walkable(&self, start: usize, i: usize) -> bool {
        i == start || self.nodes[i].kind != NodeKind::Start
    }

    /// Every node reachable from `start` through walkable nodes; with `only`, through those
    /// alone (and `start`).
    fn reach(&self, start: usize, only: &[usize]) -> Vec<usize> {
        self.flood(start, |i| {
            self.walkable(start, i) && (only.is_empty() || only.contains(&i))
        })
    }

    fn flood(&self, start: usize, ok: impl Fn(usize) -> bool) -> Vec<usize> {
        let mut seen = vec![start];
        let mut queue = VecDeque::from([start]);
        while let Some(i) = queue.pop_front() {
            for &j in &self.nodes[i].adjacent {
                if !seen.contains(&j) && ok(j) {
                    seen.push(j);
                    queue.push_back(j);
                }
            }
        }
        seen
    }

    /// Whether node `n` may be taken next by `class` holding `taken`: not taken, not a start, not
    /// a second keystone, and next to the start or a taken node.
    pub fn can_take(&self, class: &str, taken: &[usize], n: usize) -> bool {
        let Some(start) = self.start_of(class) else {
            return false;
        };
        n < self.nodes.len()
            && !taken.contains(&n)
            && self.nodes[n].kind != NodeKind::Start
            && !(self.nodes[n].kind == NodeKind::Keystone && self.holds_keystone(taken))
            && self.nodes[n]
                .adjacent
                .iter()
                .any(|&j| j == start || taken.contains(&j))
    }

    /// The fewest nodes `class` holding `taken` must take to reach `target`, nearest first,
    /// `target` last; `None` if it is taken, a start, or out of reach.
    pub fn path_to(&self, class: &str, taken: &[usize], target: usize) -> Option<Vec<usize>> {
        let start = self.start_of(class)?;
        if target >= self.nodes.len()
            || taken.contains(&target)
            || self.nodes[target].kind == NodeKind::Start
        {
            return None;
        }
        // Breadth first from everything held, through nodes not held.
        let mut from = vec![usize::MAX; self.nodes.len()];
        let mut queue = VecDeque::new();
        for &h in std::iter::once(&start).chain(taken) {
            from[h] = h;
            queue.push_back(h);
        }
        while let Some(i) = queue.pop_front() {
            if i == target {
                let mut path = vec![target];
                let mut at = target;
                while !taken.contains(&from[at]) && from[at] != start {
                    at = from[at];
                    path.push(at);
                }
                path.reverse();
                return Some(path);
            }
            for &j in &self.nodes[i].adjacent {
                // A keystone is only ever an end: never passed through, and one at a time.
                let keystone = self.nodes[j].kind == NodeKind::Keystone;
                let open = !keystone || (j == target && !self.holds_keystone(taken));
                if from[j] == usize::MAX && self.walkable(start, j) && open {
                    from[j] = i;
                    queue.push_back(j);
                }
            }
        }
        None
    }

    /// Whether `taken` holds a keystone: a loadout takes one at most.
    pub fn holds_keystone(&self, taken: &[usize]) -> bool {
        taken.iter().any(|&i| {
            self.nodes
                .get(i)
                .is_some_and(|n| n.kind == NodeKind::Keystone)
        })
    }

    /// Whether `class` may give back node `n` of `taken`: everything else it holds still
    /// connects to its start without it.
    pub fn can_refund(&self, class: &str, taken: &[usize], n: usize) -> bool {
        let Some(start) = self.start_of(class) else {
            return false;
        };
        if !taken.contains(&n) {
            return false;
        }
        let rest: Vec<usize> = taken.iter().copied().filter(|&i| i != n).collect();
        let reach = self.flood(start, |i| rest.contains(&i));
        rest.iter().all(|i| reach.contains(i))
    }

    /// `taken` made valid for `class` with `points`: in the order given, each node kept if it is
    /// real, new, not a start and connects to what was kept, until the points run out. Nodes
    /// listed before their way in are kept once it is.
    pub fn sanitize(&self, class: &str, taken: &[usize], points: u32) -> Vec<usize> {
        let mut kept: Vec<usize> = Vec::new();
        let mut left: Vec<usize> = taken.to_vec();
        loop {
            let before = kept.len();
            left.retain(|&n| {
                if kept.len() as u32 >= points {
                    return false;
                }
                if self.can_take(class, &kept, n) {
                    kept.push(n);
                    false
                } else {
                    n < self.nodes.len() && !kept.contains(&n)
                }
            });
            if kept.len() == before {
                return kept;
            }
        }
    }

    /// What the nodes `taken` give, gathered.
    pub fn bonuses(&self, taken: &[usize]) -> TalentBonuses {
        let mut b = TalentBonuses::default();
        for &n in taken {
            if let Some(node) = self.nodes.get(n) {
                for e in &node.effects {
                    b.add(e);
                }
            }
        }
        b
    }

    /// The ids of `taken`, for saving.
    pub fn ids(&self, taken: &[usize]) -> Vec<String> {
        taken
            .iter()
            .filter_map(|&n| self.nodes.get(n).map(|x| x.id.clone()))
            .collect()
    }

    /// The nodes called `ids`; unknown ids are dropped.
    pub fn indices(&self, ids: &[String]) -> Vec<usize> {
        ids.iter().filter_map(|id| self.node(id)).collect()
    }
}

/// Everything a hero's talents give, gathered from its nodes: what the world reads during a run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TalentBonuses {
    /// Plain stats, already added to the hero's [`crate::Stats`] when the run began.
    pub stats: Vec<(Stat, f32)>,
    pub tagged: Vec<(Tag, SpellBonus, f32)>,
    pub hybrid: Vec<(Tag, Tag, SpellBonus, f32)>,
    pub pairs: Vec<(Tag, Tag, f32)>,
    pub crit_burn: Vec<(Tag, f32)>,
    pub chill_freeze: f32,
    pub frozen_crit: f32,
    pub thermal: f32,
    pub execute: f32,
    pub crit_streak: Vec<Tag>,
    pub leech: f32,
    pub kill_haste: f32,
    pub offer_weight: Vec<(Tag, f32)>,
    pub augment_weight: Vec<(Augment, f32)>,
    pub unlocks: Vec<(Augment, Tags)>,
    pub rerolls: u32,
    pub gold: u32,
    pub keystones: Vec<Keystone>,
}

impl TalentBonuses {
    fn add(&mut self, e: &TalentEffect) {
        fn sum<K: PartialEq>(list: &mut Vec<(K, f32)>, k: K, v: f32) {
            match list.iter_mut().find(|(x, _)| *x == k) {
                Some((_, a)) => *a += v,
                None => list.push((k, v)),
            }
        }
        match e {
            TalentEffect::Stat(s, v) => sum(&mut self.stats, *s, *v),
            TalentEffect::Tagged(t, b, v) => {
                match self.tagged.iter_mut().find(|x| x.0 == *t && x.1 == *b) {
                    Some(x) => x.2 += v,
                    None => self.tagged.push((*t, *b, *v)),
                }
            }
            TalentEffect::Hybrid(a, b, x, v) => self.hybrid.push((*a, *b, *x, *v)),
            TalentEffect::Pair(a, b, v) => self.pairs.push((*a, *b, *v)),
            TalentEffect::CritBurn(t, v) => sum(&mut self.crit_burn, *t, *v),
            TalentEffect::ChillFreeze(v) => self.chill_freeze += v,
            TalentEffect::FrozenCrit(v) => self.frozen_crit += v,
            TalentEffect::Thermal(v) => self.thermal += v,
            TalentEffect::Execute(v) => self.execute += v,
            TalentEffect::CritStreak(t) => {
                if !self.crit_streak.contains(t) {
                    self.crit_streak.push(*t);
                }
            }
            TalentEffect::Leech(v) => self.leech += v,
            TalentEffect::KillHaste(v) => self.kill_haste += v,
            TalentEffect::OfferWeight(t, v) => sum(&mut self.offer_weight, *t, *v),
            TalentEffect::AugmentWeight(a, v) => sum(&mut self.augment_weight, *a, *v),
            TalentEffect::Unlock(a, tags) => {
                let t = Tags::of(tags);
                match self.unlocks.iter_mut().find(|x| x.0 == *a) {
                    Some(x) => x.1 = Tags(x.1 .0 | t.0),
                    None => self.unlocks.push((*a, t)),
                }
            }
            TalentEffect::Rerolls(n) => self.rerolls += n,
            TalentEffect::Gold(n) => self.gold += n,
            TalentEffect::Keystone(k) => {
                if !self.keystones.contains(k) {
                    self.keystones.push(*k);
                    for &(s, v) in k.stats() {
                        sum(&mut self.stats, s, v);
                    }
                }
            }
        }
    }

    /// Nothing taken.
    pub fn is_empty(&self) -> bool {
        *self == TalentBonuses::default()
    }

    pub fn has(&self, k: Keystone) -> bool {
        self.keystones.contains(&k)
    }

    /// The total of `bonus` a spell with `tags` gets from tag and hybrid nodes.
    pub fn spell_bonus(&self, tags: Tags, bonus: SpellBonus) -> f32 {
        let one: f32 = self
            .tagged
            .iter()
            .filter(|x| x.1 == bonus && tags.has(x.0))
            .map(|x| x.2)
            .sum();
        let both: f32 = self
            .hybrid
            .iter()
            .filter(|x| x.2 == bonus && tags.has(x.0) && tags.has(x.1))
            .map(|x| x.3)
            .sum();
        one + both
    }

    /// How much likelier a spell with `tags` is to be offered: 1 for no change.
    pub fn offer_weight(&self, tags: Tags) -> f32 {
        1.0 + self
            .offer_weight
            .iter()
            .filter(|(t, _)| tags.has(*t))
            .map(|(_, v)| v)
            .sum::<f32>()
    }

    /// How much likelier augment `a` is to be offered: 1 for no change.
    pub fn augment_weight(&self, a: Augment) -> f32 {
        1.0 + self
            .augment_weight
            .iter()
            .filter(|(x, _)| *x == a)
            .map(|(_, v)| v)
            .sum::<f32>()
    }

    /// Whether a talent lets augment `a` be offered on a spell with `tags`.
    pub fn unlocks(&self, a: Augment, tags: Tags) -> bool {
        self.unlocks
            .iter()
            .any(|&(x, t)| x == a && tags.shared(t) > 0)
    }
}

/// A hero's talents in a run: what they give, and the little state their mechanics keep. Only
/// the bonuses travel in a co-op snapshot; the state is the host's alone.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeroTalents {
    pub bonuses: TalentBonuses,
    /// How many nodes the hero took.
    pub nodes: u32,
    /// Critical strikes in a row, for a crit streak notable, and the run time it may cast again.
    #[serde(skip)]
    pub(crate) streak: u8,
    #[serde(skip)]
    pub(crate) streak_ready: f32,
    /// Elemental Equilibrium's marks: an enemy, the schools that marked it, and the run time
    /// the mark fades.
    #[serde(skip)]
    pub(crate) marks: Vec<(u32, Tags, f32)>,
}

/// How many loadouts each class keeps.
pub const LOADOUT_SLOTS: usize = 3;

/// One class's saved loadouts, by node id, and which is in use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassTalents {
    pub class: String,
    pub active: usize,
    pub loadouts: [Vec<String>; LOADOUT_SLOTS],
}

/// What a player keeps between sessions: renown, the per class or shared setting, any test
/// points, and each class's loadouts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TalentProfile {
    /// Points come from every class's renown together (true), or from each class's own (false,
    /// the default).
    pub shared: bool,
    /// Renown earned, by class key.
    pub renown: Vec<(String, u64)>,
    /// Points given for testing, on top of what renown gives (still at most the web's max).
    pub bonus: u32,
    pub classes: Vec<ClassTalents>,
}

impl TalentProfile {
    /// Renown `class` has earned.
    pub fn renown(&self, class: &str) -> u64 {
        self.renown
            .iter()
            .find(|(c, _)| c == class)
            .map_or(0, |&(_, r)| r)
    }

    /// Every class's renown together.
    pub fn total_renown(&self) -> u64 {
        self.renown.iter().map(|(_, r)| r).sum()
    }

    /// The renown that counts for `class` under the setting.
    pub fn counted_renown(&self, class: &str) -> u64 {
        if self.shared {
            self.total_renown()
        } else {
            self.renown(class)
        }
    }

    pub fn earn(&mut self, class: &str, amount: u64) {
        match self.renown.iter_mut().find(|(c, _)| c == class) {
            Some((_, r)) => *r += amount,
            None => self.renown.push((class.to_owned(), amount)),
        }
    }

    /// The points `class` has to spend.
    pub fn points(&self, class: &str, rules: &PointRules) -> u32 {
        let (level, _, _) = rules.level(self.counted_renown(class));
        (level + self.bonus).min(rules.max)
    }

    /// `class`'s loadouts, made if it has none yet.
    pub fn class_mut(&mut self, class: &str) -> &mut ClassTalents {
        if let Some(i) = self.classes.iter().position(|c| c.class == class) {
            return &mut self.classes[i];
        }
        self.classes.push(ClassTalents {
            class: class.to_owned(),
            ..Default::default()
        });
        self.classes.last_mut().expect("just pushed")
    }

    /// The loadout `class` uses, by node id.
    pub fn active(&self, class: &str) -> &[String] {
        self.classes
            .iter()
            .find(|c| c.class == class)
            .map_or(&[], |c| &c.loadouts[c.active.min(LOADOUT_SLOTS - 1)])
    }

    /// Which loadout `class` uses.
    pub fn active_slot(&self, class: &str) -> usize {
        self.classes
            .iter()
            .find(|c| c.class == class)
            .map_or(0, |c| c.active.min(LOADOUT_SLOTS - 1))
    }

    /// The profile as its file holds it: `key=value` lines.
    pub fn to_text(&self) -> String {
        let mut out = String::from("# Azeroth Together talents: renown, the setting, loadouts\n");
        out += &format!(
            "mode={}\n",
            if self.shared { "shared" } else { "per_class" }
        );
        out += &format!("bonus={}\n", self.bonus);
        for (class, r) in &self.renown {
            out += &format!("renown.{class}={r}\n");
        }
        for c in &self.classes {
            out += &format!("active.{}={}\n", c.class, c.active);
            for (i, l) in c.loadouts.iter().enumerate() {
                out += &format!("loadout.{}.{i}={}\n", c.class, l.join(","));
            }
        }
        out
    }

    /// A profile from its file; lines it does not understand are skipped.
    pub fn from_text(text: &str) -> TalentProfile {
        let mut p = TalentProfile::default();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            let parts: Vec<&str> = key.split('.').collect();
            match parts[..] {
                ["mode"] => p.shared = value == "shared",
                ["bonus"] => p.bonus = value.parse().unwrap_or(0),
                ["renown", class] => {
                    if let Ok(r) = value.parse::<u64>() {
                        p.earn(class, r);
                    }
                }
                ["active", class] => {
                    p.class_mut(class).active =
                        value.parse::<usize>().unwrap_or(0).min(LOADOUT_SLOTS - 1);
                }
                ["loadout", class, slot] => {
                    if let Ok(slot) = slot.parse::<usize>() {
                        if slot < LOADOUT_SLOTS {
                            p.class_mut(class).loadouts[slot] = value
                                .split(',')
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(str::to_owned)
                                .collect();
                        }
                    }
                }
                _ => {}
            }
        }
        p
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small web: two classes, a ring of four, and a branch.
    ///
    /// ```text
    ///   a_start - n1 - n2 - n3
    ///              |        |
    ///   b_start - n4 ------ n5 - k
    /// ```
    pub(crate) const SMALL: &str = r#"(
        points: (max: 3, first: 100, step: 50),
        nodes: [
            (id: "a_start", name: "A", kind: Start, class: Some("mage"), pos: (0, 0), links: ["n1"]),
            (id: "b_start", name: "B", kind: Start, class: Some("warrior"), pos: (0, 100), links: ["n4"]),
            (id: "n1", name: "Fire", kind: Small, pos: (50, 0), links: ["n2", "n4"],
             effects: [Tagged(Fire, Damage, 0.04)]),
            (id: "n2", name: "Frost", kind: Small, pos: (100, 0), links: ["n3"],
             effects: [Tagged(Frost, Damage, 0.04)]),
            (id: "n3", name: "Deep Winter", kind: Notable, pos: (150, 0), links: ["n5"],
             effects: [ChillFreeze(0.15)]),
            (id: "n4", name: "Might", kind: Small, pos: (50, 100), links: ["n5"],
             effects: [Stat(Damage, 0.02)]),
            (id: "n5", name: "Vigor", kind: Small, pos: (150, 100), links: ["k"],
             effects: [Stat(Health, 0.04)]),
            (id: "k", name: "Glass Cannon", kind: Keystone, pos: (200, 100),
             effects: [Keystone(GlassCannon)]),
        ],
    )"#;

    pub(crate) fn small() -> TalentWeb {
        TalentWeb::from_ron(SMALL).unwrap()
    }

    fn ix(w: &TalentWeb, ids: &[&str]) -> Vec<usize> {
        ids.iter().map(|id| w.node(id).unwrap()).collect()
    }

    #[test]
    fn links_go_both_ways() {
        let w = small();
        let n1 = w.node("n1").unwrap();
        let a = w.node("a_start").unwrap();
        assert!(w.nodes[a].adjacent.contains(&n1));
        assert!(w.nodes[n1].adjacent.contains(&a));
    }

    #[test]
    fn a_bad_web_is_refused() {
        let dup = SMALL.replace("id: \"n2\"", "id: \"n1\"");
        assert!(TalentWeb::from_ron(&dup).is_err());
        let unknown = SMALL.replace("links: [\"k\"]", "links: [\"nowhere\"]");
        assert!(TalentWeb::from_ron(&unknown).is_err());
        let cut = SMALL.replace("links: [\"k\"]", "links: []");
        assert!(
            TalentWeb::from_ron(&cut)
                .unwrap_err()
                .contains("cannot be reached"),
            "an island is refused"
        );
    }

    #[test]
    fn only_a_node_next_to_what_you_hold_can_be_taken() {
        let w = small();
        let [n1, n2, n4, a_start] = ix(&w, &["n1", "n2", "n4", "a_start"])[..] else {
            unreachable!()
        };
        assert!(w.can_take("mage", &[], n1));
        assert!(!w.can_take("mage", &[], n2), "not yet connected");
        assert!(w.can_take("mage", &[n1], n2));
        assert!(!w.can_take("mage", &[n1], n1), "already held");
        assert!(!w.can_take("mage", &[], a_start), "a start is never taken");
        assert!(w.can_take("warrior", &[], n4));
        assert!(!w.can_take("warrior", &[], n1));
    }

    #[test]
    fn a_path_runs_from_what_you_hold_and_never_through_another_start() {
        let w = small();
        let want = ix(&w, &["n1", "n2", "n3"]);
        assert_eq!(w.path_to("mage", &[], want[2]), Some(want.clone()));
        let k = w.node("k").unwrap();
        // The keystone is reached through n4 and n5, not a longer way.
        assert_eq!(
            w.path_to("mage", &[], k),
            Some(ix(&w, &["n1", "n4", "n5", "k"]))
        );
        assert_eq!(w.path_to("mage", &want, k), Some(ix(&w, &["n5", "k"])));
        assert_eq!(w.path_to("mage", &[], w.node("b_start").unwrap()), None);
    }

    #[test]
    fn a_loadout_takes_one_keystone_and_never_walks_through_one() {
        let text = SMALL.replace(
            "effects: [Keystone(GlassCannon)]),",
            "links: [\"k2\"], effects: [Keystone(GlassCannon)]),
            (id: \"k2\", name: \"Steady\", kind: Keystone, pos: (250, 100),
             effects: [Keystone(SteadyHand)]),",
        );
        let w = TalentWeb::from_ron(&text).unwrap();
        let held = ix(&w, &["n1", "n4", "n5", "k"]);
        let k2 = w.node("k2").unwrap();
        assert!(!w.can_take("mage", &held, k2), "a second keystone");
        assert_eq!(w.path_to("mage", &held, k2), None);
        assert_eq!(
            w.path_to("mage", &[], k2),
            None,
            "the way to it runs through another keystone"
        );
        assert!(w.holds_keystone(&held) && !w.holds_keystone(&held[..3]));
    }

    #[test]
    fn a_refund_must_leave_the_rest_connected() {
        let w = small();
        let held = ix(&w, &["n1", "n2", "n3"]);
        assert!(!w.can_refund("mage", &held, held[0]), "n1 holds the rest");
        assert!(w.can_refund("mage", &held, held[2]), "a leaf goes freely");
        // A ring: with n4 and n5 too, n2 can go, since n3 still connects through n5.
        let ring = ix(&w, &["n1", "n2", "n3", "n4", "n5"]);
        assert!(w.can_refund("mage", &ring, ring[1]));
    }

    #[test]
    fn sanitize_keeps_what_connects_up_to_the_points() {
        let w = small();
        let order = ix(&w, &["n3", "n2", "n1", "n4"]);
        let kept = w.sanitize("mage", &order, 10);
        assert_eq!(kept.len(), 4, "listed before its way in is still kept");
        assert_eq!(w.sanitize("mage", &order, 2), ix(&w, &["n1", "n4"]));
        let start = w.node("a_start").unwrap();
        assert!(w.sanitize("mage", &[start, 999], 10).is_empty());
        assert!(
            w.sanitize("warrior", &ix(&w, &["n2"]), 10).is_empty(),
            "not connected for that class"
        );
    }

    #[test]
    fn bonuses_gather_and_keystones_bring_their_stats() {
        let w = small();
        let b = w.bonuses(&ix(&w, &["n1", "n3", "n4", "k"]));
        assert_eq!(
            b.spell_bonus(Tags::of(&[Tag::Fire]), SpellBonus::Damage),
            0.04
        );
        assert_eq!(
            b.spell_bonus(Tags::of(&[Tag::Frost]), SpellBonus::Damage),
            0.0
        );
        assert_eq!(b.chill_freeze, 0.15);
        assert!(b.has(Keystone::GlassCannon));
        let damage: f32 = b
            .stats
            .iter()
            .filter(|(s, _)| *s == Stat::Damage)
            .map(|x| x.1)
            .sum();
        assert!((damage - 0.42).abs() < 1e-5);
        assert!(b.stats.contains(&(Stat::Health, -0.5)));
        assert!(TalentBonuses::default().is_empty() && !b.is_empty());
    }

    #[test]
    fn hybrid_bonuses_need_both_tags() {
        let mut b = TalentBonuses::default();
        b.add(&TalentEffect::Hybrid(
            Tag::Fire,
            Tag::Frost,
            SpellBonus::Damage,
            0.1,
        ));
        assert_eq!(
            b.spell_bonus(Tags::of(&[Tag::Fire]), SpellBonus::Damage),
            0.0
        );
        assert_eq!(
            b.spell_bonus(Tags::of(&[Tag::Fire, Tag::Frost]), SpellBonus::Damage),
            0.1
        );
    }

    #[test]
    fn weights_and_unlocks_read_by_tag() {
        let mut b = TalentBonuses::default();
        b.add(&TalentEffect::OfferWeight(Tag::Fire, 0.3));
        b.add(&TalentEffect::AugmentWeight(Augment::Frostfire, 1.0));
        b.add(&TalentEffect::Unlock(Augment::Ignite, vec![Tag::Fire]));
        assert!((b.offer_weight(Tags::of(&[Tag::Fire, Tag::Area])) - 1.3).abs() < 1e-6);
        assert_eq!(b.offer_weight(Tags::of(&[Tag::Frost])), 1.0);
        assert_eq!(b.augment_weight(Augment::Frostfire), 2.0);
        assert_eq!(b.augment_weight(Augment::Spellfire), 1.0);
        assert!(b.unlocks(Augment::Ignite, Tags::of(&[Tag::Fire, Tag::Ground])));
        assert!(!b.unlocks(Augment::Ignite, Tags::of(&[Tag::Frost])));
    }

    #[test]
    fn renown_levels_climb_and_stop_at_the_max() {
        let r = PointRules {
            max: 3,
            first: 100,
            step: 50,
        };
        assert_eq!(r.level(0), (0, 0, 100));
        assert_eq!(r.level(99), (0, 99, 100));
        assert_eq!(r.level(100), (1, 0, 150));
        assert_eq!(r.level(260), (2, 10, 200));
        assert_eq!(r.level(1_000_000).0, 3);
        assert_eq!(renown_for_run(1, 30.0, 10), 0);
        assert_eq!(renown_for_run(16, 8.5 * 60.0, 600), 150 + 80 + 24);
    }

    #[test]
    fn points_count_per_class_or_shared() {
        let rules = PointRules {
            max: 40,
            first: 100,
            step: 50,
        };
        let mut p = TalentProfile::default();
        p.earn("mage", 100);
        p.earn("warrior", 150);
        assert_eq!(p.points("mage", &rules), 1);
        assert_eq!(p.points("warrior", &rules), 1);
        assert_eq!(p.points("priest", &rules), 0);
        p.shared = true;
        assert_eq!(p.points("priest", &rules), 2, "250 renown together");
        p.bonus = 100;
        assert_eq!(p.points("priest", &rules), 40, "never past the max");
    }

    #[test]
    fn a_profile_survives_its_file() {
        let mut p = TalentProfile {
            shared: true,
            bonus: 5,
            ..Default::default()
        };
        p.earn("mage", 1234);
        let c = p.class_mut("mage");
        c.active = 2;
        c.loadouts[0] = vec!["n1".into(), "n2".into()];
        c.loadouts[2] = vec!["n4".into()];
        let back = TalentProfile::from_text(&p.to_text());
        assert_eq!(back, p);
        assert_eq!(back.active("mage"), &["n4".to_string()]);
        assert_eq!(back.active_slot("mage"), 2);
        assert!(back.active("rogue").is_empty());
        let junk = TalentProfile::from_text("nonsense\nactive.mage=99\nloadout.mage.7=a\n");
        assert_eq!(junk.active_slot("mage"), LOADOUT_SLOTS - 1);
    }

    #[test]
    fn every_effect_says_what_it_does() {
        let all = [
            TalentEffect::Stat(Stat::Speed, 0.01),
            TalentEffect::Tagged(Tag::Fire, SpellBonus::Damage, 0.04),
            TalentEffect::Tagged(Tag::DamageOverTime, SpellBonus::Duration, 0.06),
            TalentEffect::Hybrid(Tag::Fire, Tag::Frost, SpellBonus::Damage, 0.1),
            TalentEffect::Pair(Tag::Shadow, Tag::Fire, 0.04),
            TalentEffect::CritBurn(Tag::Fire, 0.4),
            TalentEffect::ChillFreeze(0.15),
            TalentEffect::FrozenCrit(0.25),
            TalentEffect::Thermal(0.2),
            TalentEffect::Execute(0.4),
            TalentEffect::CritStreak(Tag::Holy),
            TalentEffect::Leech(0.1),
            TalentEffect::KillHaste(0.04),
            TalentEffect::OfferWeight(Tag::Fire, 0.3),
            TalentEffect::AugmentWeight(Augment::Frostfire, 1.0),
            TalentEffect::Unlock(Augment::Shatter, vec![Tag::Slow]),
            TalentEffect::Rerolls(1),
            TalentEffect::Gold(25),
            TalentEffect::Keystone(Keystone::ElementalEquilibrium),
        ];
        for e in &all {
            let lines = e.lines();
            assert!(!lines.is_empty() && lines.iter().all(|l| !l.is_empty()));
            assert!(
                lines.iter().all(|l| !l.contains('\u{2212}')),
                "the WoW font has no minus sign"
            );
        }
        assert_eq!(all[1].lines()[0], "+4% damage with Fire spells");
        assert_eq!(all[6].lines()[0], "Your Chills have a 15% chance to Freeze");
        assert_eq!(
            all[14].lines()[0],
            "Frostfire is offered twice as often on augment screens"
        );
        assert_eq!(
            all[15].lines()[0],
            "Shatter can be offered on any Slow spell"
        );
    }
}
