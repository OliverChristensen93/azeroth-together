//! Rivals as content (spec §7.4): `content/nightfall/rivals.ron` lists the kinds of rival (who
//! they look like, what they wear and carry, and their kit from the same ability system heroes
//! use), the parties they go about in (two or three of them, with an aggression), the points of
//! interest they know of, where they come into the map and the routes they walk between those
//! and the exits; `rules.ron`'s `rivals:` holds the numbers their bots go by. Each kind becomes a
//! class of its own, appended to [`Content::classes`] and marked [`ClassDef::rival`], so a rival
//! is a hero to every system that reads a class: its health, power, swing, bar and free kit.
//! Giving a kind a whole hero class's bar (`kit: Class("rogue")`) is a content change, for
//! playtesting full kits against simplified ones (spec §13).

use serde::Deserialize;

use crate::content::{non_negative, parse, positive};
use crate::{ClassDef, Content, ContentError, Effect, V2};

fn one() -> f32 {
    1.0
}

/// The numbers rivals go by (`rules.ron`'s `rivals:`); each one left out takes its default.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct RivalRules {
    /// How many parties come into a raid, fewest and most.
    pub parties_out: (u8, u8),
    /// Seconds into the raid the parties come, earliest and latest.
    pub arrive: (f32, f32),
    /// A rival's front cone, degrees (an enemy's `cone`).
    pub cone: f32,
    /// A party this aggressive or more fights a hero it is alerted to; below, it slips away.
    pub engage_at: f32,
    /// A party this aggressive or more hunts heroes once its looting is done; below, it leaves.
    pub hunt_at: f32,
    /// Seconds a hunt goes on before the party leaves.
    pub hunt_for: f32,
    /// A member below this share of health makes the party break off and flee.
    pub flee_health: f32,
    /// Seconds a flight lasts.
    pub flee_for: f32,
    /// Yards: a hero this near is fought whatever the party's aggression (it is cornered).
    pub avoid_within: f32,
    /// A member below this share of health uses a heal when it has one ready.
    pub heal_below: f32,
    /// A party whose weakest member is below this share of health does not go hunting.
    pub rest_below: f32,
    /// A member carrying this share of the run's `weight_full` or more counts as full.
    pub full_load: f32,
    /// How many points of interest a party visits, fewest and most.
    pub pois: (u8, u8),
    /// The night's stage (by key) from which every party heads out, or hunts.
    pub leave_by: Option<String>,
    /// Yards between members walking behind their leader.
    pub gap: f32,
    /// Path searches all rivals together may begin in one step.
    pub paths_per_step: u32,
    /// The chance a rival dodges a telegraph it stands in.
    pub dodge: f32,
    /// Seconds before a telegraph lands that a rival dodges it. At most the dodge's immunity
    /// (`combat.dodge.immune`), or the rival walks back into the blow when the immunity is over.
    pub dodge_at: f32,
    /// A finisher once this many combo points are on the target.
    pub finisher_at: u8,
    /// Rivals at 0 health go down as heroes do; when false they die at once.
    pub downed: bool,
    /// Yards from a container a rival searches it from (within the run's `reach`).
    pub loot_reach: f32,
    /// Seconds a party makes no headway toward a place before it gives that place up.
    pub give_up: f32,
    /// Seconds a party makes no headway toward its goal (or finds its exit shut with no other
    /// route) before it leaves unseen: it vanishes with its packs once no hero sees it, counted
    /// as out, so every party leaves or dies within the night.
    pub stuck_for: f32,
    /// Yards: the most a leg of a route may be (between two waypoints, an end included), so each
    /// leg is found by one search inside the errand budget.
    pub route_leg: f32,
}

impl Default for RivalRules {
    fn default() -> Self {
        Self {
            parties_out: (1, 2),
            arrive: (0.0, 300.0),
            cone: 120.0,
            engage_at: 0.5,
            hunt_at: 0.7,
            hunt_for: 180.0,
            flee_health: 0.3,
            flee_for: 8.0,
            avoid_within: 10.0,
            heal_below: 0.5,
            rest_below: 0.7,
            full_load: 0.8,
            pois: (2, 4),
            // Not set here: a fixture with night stages of its own has no "hunt" to name.
            leave_by: None,
            gap: 2.5,
            paths_per_step: 2,
            dodge: 0.5,
            dodge_at: 0.25,
            finisher_at: 3,
            downed: false,
            loot_reach: 2.0,
            give_up: 15.0,
            stuck_for: 60.0,
            route_leg: 80.0,
        }
    }
}

impl RivalRules {
    /// Why these numbers cannot work, if they cannot; `immune` is `combat.dodge.immune` and `reach` is
    /// `run.reach`, how near the host lets a hero search a container.
    fn check(&self, immune: f32, reach: f32) -> Result<(), String> {
        let shares = [
            self.engage_at,
            self.hunt_at,
            self.flee_health,
            self.heal_below,
            self.rest_below,
            self.full_load,
            self.dodge,
        ];
        if shares.iter().any(|s| !(0.0..=1.0).contains(s)) {
            return Err(
                "engage_at, hunt_at, flee_health, heal_below, rest_below, full_load and dodge are \
                 between 0 and 1"
                    .into(),
            );
        }
        let more = [
            self.hunt_for,
            self.flee_for,
            self.avoid_within,
            self.gap,
            self.dodge_at,
            self.loot_reach,
            self.give_up,
            self.stuck_for,
            self.route_leg,
        ];
        if !more.iter().all(|&n| positive(n)) {
            return Err(
                "hunt_for, flee_for, avoid_within, gap, dodge_at, loot_reach, give_up, stuck_for \
                 and route_leg are more than 0"
                    .into(),
            );
        }
        if self.dodge_at > immune {
            return Err(format!(
                "dodge_at is at most combat.dodge.immune ({immune}): a rival that dodges earlier \
                 walks back into the blow once its immunity is over"
            ));
        }
        if self.loot_reach > reach {
            return Err(format!(
                "loot_reach is at most run.reach ({reach}): a rival that stands farther would ask \
                 to search a container the host refuses"
            ));
        }
        if self.parties_out.0 > self.parties_out.1 {
            return Err("parties_out's fewest is more than its most".into());
        }
        if !(non_negative(self.arrive.0)
            && positive(self.arrive.1)
            && self.arrive.0 <= self.arrive.1)
        {
            return Err("arrive is two times from 0, the earliest first".into());
        }
        if self.pois.0 == 0 || self.pois.0 > self.pois.1 {
            return Err("pois is at least 1, the fewest first".into());
        }
        if !(self.cone > 0.0 && self.cone <= 360.0) {
            return Err("cone is more than 0 and at most 360 degrees".into());
        }
        if self.paths_per_step == 0 {
            return Err("paths_per_step is at least 1".into());
        }
        Ok(())
    }
}

/// Where a kind of rival's kit comes from.
#[derive(Clone, Debug, Deserialize)]
pub enum KitDef {
    /// A few abilities by key, put on the bar from action 1 in order: a simplified kit.
    Abilities(Vec<String>),
    /// The whole bar of a hero class, by key (spec §7.4: "a full Rogue or Mage kit").
    Class(String),
}

/// A kind of rival (spec §7.4).
#[derive(Clone, Debug, Deserialize)]
pub struct RivalKindDef {
    pub key: String,
    pub name: String,
    /// Names one of them goes by, picked by its party's dice; the kind's name when empty.
    #[serde(default)]
    pub names: Vec<String>,
    /// The hero class whose numbers it starts from: power, speed, swing, stealth.
    pub base: String,
    /// `ChrClasses.dbc` id for its look (the starting outfit it wears); the base's when absent.
    #[serde(default)]
    pub class_id: Option<u8>,
    pub race: u8,
    pub sex: u8,
    /// Its health; the base's when absent.
    #[serde(default)]
    pub hp: Option<f32>,
    pub kit: KitDef,
    /// Its telegraphed big move by key (spec §7.4), put on its bar after the kit when not on it.
    #[serde(default)]
    pub big: Option<String>,
    /// Yards it keeps from its target in a fight; 0 for melee.
    #[serde(default)]
    pub stand_off: f32,
    /// It goes in stealth when not fighting (its class must stealth).
    #[serde(default)]
    pub sneaks: bool,
    /// What it wears and carries by item key: worn where it goes, a bag in its first bag slot,
    /// the rest in its backpack.
    #[serde(default)]
    pub gear: Vec<(String, u16)>,
    /// A loot table by key, rolled into its backpack as it comes: what it found before.
    #[serde(default)]
    pub carry: Option<String>,
    /// Its class, an index into [`Content::classes`], made as content loads.
    #[serde(skip)]
    pub class: usize,
    /// `carry` as an index into the loot tables.
    #[serde(skip)]
    pub carry_table: Option<usize>,
    /// `big` as an index into the abilities.
    #[serde(skip)]
    pub big_ability: Option<usize>,
}

/// A kind of party (spec §7.4: grave robbers, Defias scavengers, Night Watch deserters).
#[derive(Clone, Debug, Deserialize)]
pub struct PartyDef {
    pub key: String,
    pub name: String,
    /// Its members by kind key: two or three, the first leading.
    pub members: Vec<String>,
    /// Its aggression from 0 to 1, rolled between these for each raid.
    pub aggression: (f32, f32),
    /// The chance it is out in a raid.
    pub odds: f32,
    /// `members` as indices into [`RivalContent::kinds`].
    #[serde(skip)]
    pub kinds: Vec<usize>,
}

/// A place rivals go to loot (spec §7.4, §9.2).
#[derive(Clone, Debug, Deserialize)]
pub struct PoiDef {
    pub key: String,
    pub name: String,
    pub at: (f32, f32),
    /// Yards around `at` a party searches.
    pub radius: f32,
    /// How much it draws a party; nearer draws more too.
    #[serde(default = "one")]
    pub worth: f32,
}

/// One end of a route as `rivals.ron` writes it: a rival entry by its place in `entries`, a point
/// of interest or one of the map's exits by key.
#[derive(Clone, Debug, Deserialize)]
pub enum RouteEnd {
    Entry(usize),
    Poi(String),
    Exit(String),
}

/// One end of a route, resolved to an index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutePlace {
    /// Into [`RivalContent::entries`].
    Entry(usize),
    /// Into [`RivalContent::pois`].
    Poi(usize),
    /// Into the map's `exits`.
    Exit(usize),
}

/// A route as written: the waypoints between its two ends.
#[derive(Clone, Debug, Deserialize)]
struct RouteFile {
    from: RouteEnd,
    to: RouteEnd,
    via: Vec<(f32, f32)>,
}

/// A route a party walks, leg by leg (spec §7.4: rivals travel by waypoints measured on the real
/// bake, each leg found by one search). It runs from an entry to a point of interest or from one
/// to an exit, and either way round: [`RivalContent::route`] gives it in the direction wanted.
#[derive(Clone, Debug)]
pub struct RouteDef {
    pub from: RoutePlace,
    pub to: RoutePlace,
    /// Where it goes, its two ends included: at least two points, no leg longer than
    /// [`RivalRules::route_leg`].
    pub points: Vec<V2>,
}

#[derive(Deserialize)]
struct RivalFile {
    kinds: Vec<RivalKindDef>,
    parties: Vec<PartyDef>,
    #[serde(default)]
    pois: Vec<PoiDef>,
    #[serde(default)]
    entries: Vec<(f32, f32)>,
    #[serde(default)]
    routes: Vec<RouteFile>,
}

/// The rivals' content, resolved.
#[derive(Clone, Debug, Default)]
pub struct RivalContent {
    pub kinds: Vec<RivalKindDef>,
    pub parties: Vec<PartyDef>,
    pub pois: Vec<PoiDef>,
    /// Where parties come into the map.
    pub entries: Vec<V2>,
    /// The routes between entries, points of interest and exits. Empty means the content gives
    /// none: a party then searches straight to its goal (the small maps of the tests).
    pub routes: Vec<RouteDef>,
    /// [`RivalRules::leave_by`] as a stage index.
    pub leave_by: Option<usize>,
}

impl RivalContent {
    /// `rivals.ron`'s text, read into `content`: every kind made a class appended to its
    /// classes. An empty text is no rivals.
    pub(crate) fn load(text: &str, content: &mut Content) -> Result<RivalContent, ContentError> {
        let invalid = |why: String| ContentError::Invalid(format!("rivals.ron: {why}"));
        content
            .rules
            .rivals
            .check(content.rules.combat.dodge.immune, content.rules.run.reach)
            .map_err(|why| ContentError::Invalid(format!("rules.ron: rivals: {why}")))?;
        let leave_by = match content.rules.rivals.leave_by.clone() {
            Some(k) => Some(content.stage(&k).ok_or_else(|| {
                ContentError::Invalid(format!(
                    "rules.ron: rivals: leave_by names no stage \"{k}\""
                ))
            })?),
            None => None,
        };
        if text.trim().is_empty() {
            return Ok(RivalContent {
                leave_by,
                ..RivalContent::default()
            });
        }
        let file: RivalFile = parse("rivals.ron", text)?;
        let mut kinds = file.kinds;
        for i in 0..kinds.len() {
            if kinds[..i].iter().any(|k| k.key == kinds[i].key) {
                return Err(invalid(format!(
                    "kind \"{}\" is listed twice",
                    kinds[i].key
                )));
            }
            let class = class_of(&mut kinds[i], content).map_err(invalid)?;
            kinds[i].class = content.classes.len();
            content.classes.push(class);
        }
        let mut parties = file.parties;
        for i in 0..parties.len() {
            let p = &parties[i];
            if parties[..i].iter().any(|q| q.key == p.key) {
                return Err(invalid(format!("party \"{}\" is listed twice", p.key)));
            }
            if !(2..=3).contains(&p.members.len()) {
                return Err(invalid(format!("party \"{}\" has 2 or 3 members", p.key)));
            }
            let (lo, hi) = p.aggression;
            if !((0.0..=1.0).contains(&lo) && (0.0..=1.0).contains(&hi) && lo <= hi) {
                return Err(invalid(format!(
                    "party \"{}\": aggression is two numbers from 0 to 1, the lower first",
                    p.key
                )));
            }
            if !(0.0..=1.0).contains(&p.odds) {
                return Err(invalid(format!("party \"{}\": odds is from 0 to 1", p.key)));
            }
            let found: Result<Vec<usize>, String> = p
                .members
                .iter()
                .map(|m| {
                    kinds
                        .iter()
                        .position(|k| &k.key == m)
                        .ok_or_else(|| format!("party \"{}\": no kind \"{m}\"", p.key))
                })
                .collect();
            parties[i].kinds = found.map_err(invalid)?;
        }
        let bounds = &content.map.bounds;
        for (i, poi) in file.pois.iter().enumerate() {
            if file.pois[..i].iter().any(|q| q.key == poi.key) {
                return Err(invalid(format!("point \"{}\" is listed twice", poi.key)));
            }
            if !(positive(poi.radius) && positive(poi.worth)) {
                return Err(invalid(format!(
                    "point \"{}\": radius and worth are more than 0",
                    poi.key
                )));
            }
            if !(poi.at.0.is_finite() && poi.at.1.is_finite())
                || !bounds.contains(V2::new(poi.at.0, poi.at.1))
            {
                return Err(invalid(format!("point \"{}\" is outside the map", poi.key)));
            }
        }
        let entries: Vec<V2> = file.entries.iter().map(|&(x, y)| V2::new(x, y)).collect();
        if let Some(e) = entries
            .iter()
            .find(|e| !(e.x.is_finite() && e.y.is_finite()) || !bounds.contains(**e))
        {
            return Err(invalid(format!(
                "entry ({}, {}) is outside the map",
                e.x, e.y
            )));
        }
        if !parties.is_empty() && entries.is_empty() {
            return Err(invalid("parties need at least one entry".into()));
        }
        if !parties.is_empty() && usize::from(content.rules.rivals.pois.0) > file.pois.len() {
            return Err(invalid(format!(
                "rules.ron's rivals pois wants at least {} points of interest, there are {}",
                content.rules.rivals.pois.0,
                file.pois.len()
            )));
        }
        let routes = load_routes(&file.routes, &file.pois, &entries, content).map_err(invalid)?;
        Ok(RivalContent {
            kinds,
            parties,
            pois: file.pois,
            entries,
            routes,
            leave_by,
        })
    }

    /// The route between two places as a party walks it from `from` to `to`: the waypoints in
    /// that order, both ends included. A route is kept in one direction and walks either way.
    /// `None` when the content has none between them.
    pub fn route(&self, from: RoutePlace, to: RoutePlace) -> Option<Vec<V2>> {
        self.routes.iter().find_map(|r| {
            if (r.from, r.to) == (from, to) {
                Some(r.points.clone())
            } else if (r.from, r.to) == (to, from) {
                Some(r.points.iter().rev().copied().collect())
            } else {
                None
            }
        })
    }
}

/// The routes read and checked against the places they join. A map with routes has all of them:
/// one from every entry to every point of interest, and one from every point of interest to
/// every exit, so a party can go anywhere it may want to and leave by any exit that is open.
fn load_routes(
    file: &[RouteFile],
    pois: &[PoiDef],
    entries: &[V2],
    content: &Content,
) -> Result<Vec<RouteDef>, String> {
    let exits = &content.map.exits;
    let leg = content.rules.rivals.route_leg;
    let bounds = &content.map.bounds;
    let resolve = |end: &RouteEnd| -> Result<(RoutePlace, V2), String> {
        match end {
            RouteEnd::Entry(i) => entries
                .get(*i)
                .map(|&at| (RoutePlace::Entry(*i), at))
                .ok_or_else(|| format!("route: no entry {i}")),
            RouteEnd::Poi(k) => pois
                .iter()
                .position(|p| &p.key == k)
                .map(|i| (RoutePlace::Poi(i), V2::new(pois[i].at.0, pois[i].at.1)))
                .ok_or_else(|| format!("route: no point \"{k}\"")),
            RouteEnd::Exit(k) => exits
                .iter()
                .position(|e| &e.key == k)
                .map(|i| (RoutePlace::Exit(i), exits[i].at))
                .ok_or_else(|| format!("route: no exit \"{k}\"")),
        }
    };
    let mut routes: Vec<RouteDef> = Vec::new();
    for r in file {
        let (mut from, mut a) = resolve(&r.from)?;
        let (mut to, mut b) = resolve(&r.to)?;
        let mut via = r.via.clone();
        // Kept from the entry to the point of interest, from that to the exit, however written.
        if matches!(
            (from, to),
            (RoutePlace::Exit(_), RoutePlace::Poi(_)) | (RoutePlace::Poi(_), RoutePlace::Entry(_))
        ) {
            (from, to, a, b) = (to, from, b, a);
            via.reverse();
        }
        let joins = matches!(
            (from, to),
            (RoutePlace::Entry(_), RoutePlace::Poi(_)) | (RoutePlace::Poi(_), RoutePlace::Exit(_))
        );
        if !joins {
            return Err(format!(
                "route {from:?} to {to:?}: a route runs from an entry to a point of interest or \
                 from one to an exit"
            ));
        }
        if routes
            .iter()
            .any(|q| (q.from, q.to) == (from, to) || (q.from, q.to) == (to, from))
        {
            return Err(format!("route {from:?} to {to:?} is listed twice"));
        }
        let mut points = vec![a];
        for &(x, y) in &via {
            let p = V2::new(x, y);
            if !(x.is_finite() && y.is_finite()) || !bounds.contains(p) {
                return Err(format!(
                    "route {from:?} to {to:?}: waypoint ({x}, {y}) is outside the map"
                ));
            }
            points.push(p);
        }
        points.push(b);
        if let Some(w) = points.windows(2).find(|w| {
            let d = w[0].distance(w[1]);
            !d.is_finite() || d > leg
        }) {
            return Err(format!(
                "route {from:?} to {to:?}: the leg from ({}, {}) to ({}, {}) is longer than \
                 route_leg {leg}",
                w[0].x, w[0].y, w[1].x, w[1].y
            ));
        }
        routes.push(RouteDef { from, to, points });
    }
    if !routes.is_empty() {
        let has = |a: RoutePlace, b: RoutePlace| {
            routes
                .iter()
                .any(|r| (r.from, r.to) == (a, b) || (r.from, r.to) == (b, a))
        };
        for e in 0..entries.len() {
            for (p, poi) in pois.iter().enumerate() {
                if !has(RoutePlace::Entry(e), RoutePlace::Poi(p)) {
                    return Err(format!("no route from entry {e} to point \"{}\"", poi.key));
                }
            }
        }
        for (p, poi) in pois.iter().enumerate() {
            for (x, exit) in exits.iter().enumerate() {
                if !has(RoutePlace::Poi(p), RoutePlace::Exit(x)) {
                    return Err(format!(
                        "no route from point \"{}\" to exit \"{}\"",
                        poi.key, exit.key
                    ));
                }
            }
        }
    }
    Ok(routes)
}

/// The class kind `k` plays as: its base's, with its own name, look, health, bar and gear; and
/// its resolved parts filled in.
fn class_of(k: &mut RivalKindDef, content: &Content) -> Result<ClassDef, String> {
    let hero_class = |key: &str| content.class(key).filter(|&c| !content.classes[c].rival);
    let base = hero_class(&k.base)
        .ok_or_else(|| format!("kind \"{}\": no hero class \"{}\"", k.key, k.base))?;
    let mut class = content.classes[base].clone();
    class.key = format!("rival:{}", k.key);
    class.name = k.name.clone();
    class.description = String::new();
    if let Some(id) = k.class_id {
        class.class_id = id;
    }
    class.race = k.race;
    class.sex = k.sex;
    if let Some(hp) = k.hp {
        if !positive(hp) {
            return Err(format!("kind \"{}\": hp is more than 0", k.key));
        }
        class.base_hp = hp;
    }
    class.bar = match &k.kit {
        KitDef::Abilities(keys) if keys.is_empty() => {
            return Err(format!(
                "kind \"{}\": a kit has at least one ability",
                k.key
            ));
        }
        KitDef::Abilities(keys) => keys
            .iter()
            .enumerate()
            .map(|(i, key)| ((i + 1) as u8, key.clone()))
            .collect(),
        KitDef::Class(key) => {
            let c = hero_class(key)
                .ok_or_else(|| format!("kind \"{}\": no hero class \"{key}\"", k.key))?;
            content.classes[c].bar.clone()
        }
    };
    k.big_ability = match &k.big {
        None => None,
        Some(key) => {
            let a = content
                .ability(key)
                .ok_or_else(|| format!("kind \"{}\": no ability \"{key}\"", k.key))?;
            if content.abilities[a].telegraph.is_none() {
                return Err(format!(
                    "kind \"{}\": its big move \"{key}\" needs a telegraph (spec §7.4)",
                    k.key
                ));
            }
            if !class.bar.iter().any(|(_, b)| b == key) {
                let last = class.bar.iter().map(|(s, _)| *s).filter(|s| *s <= 12).max();
                class.bar.push((last.unwrap_or(0) + 1, key.clone()));
            }
            Some(a)
        }
    };
    let kit: Result<Vec<(u8, usize)>, String> = class
        .bar
        .iter()
        .map(|(slot, key)| {
            content
                .ability(key)
                .map(|a| (*slot, a))
                .ok_or_else(|| format!("kind \"{}\": no ability \"{key}\"", k.key))
        })
        .collect();
    class.kit = kit?;
    if k.sneaks {
        let can = class.stealth
            && class
                .kit
                .iter()
                .any(|&(_, a)| content.abilities[a].has(|e| matches!(e, Effect::Stealth)));
        if !can {
            return Err(format!(
                "kind \"{}\" sneaks, so its class and kit stealth",
                k.key
            ));
        }
    }
    if !non_negative(k.stand_off) {
        return Err(format!("kind \"{}\": stand_off is 0 or more", k.key));
    }
    let items: Result<Vec<(usize, u16)>, String> = k
        .gear
        .iter()
        .map(|(key, n)| {
            content
                .item(key)
                .map(|i| (i, *n))
                .ok_or_else(|| format!("kind \"{}\": no item \"{key}\"", k.key))
        })
        .collect();
    class.free_kit = k.gear.clone();
    class.free_items = items?;
    k.carry_table = match &k.carry {
        None => None,
        Some(t) => Some(
            content
                .loot
                .tables
                .iter()
                .position(|x| &x.key == t)
                .ok_or_else(|| format!("kind \"{}\": no loot table \"{t}\"", k.key))?,
        ),
    };
    class.rival = true;
    Ok(class)
}

impl Content {
    /// The classes a player may pick: every class but the rivals', with its index.
    pub fn playable(&self) -> impl Iterator<Item = (usize, &ClassDef)> + '_ {
        self.classes.iter().enumerate().filter(|(_, c)| !c.rival)
    }

    /// The kind of rival with this key, by index.
    pub fn rival_kind(&self, key: &str) -> Option<usize> {
        self.rivals.kinds.iter().position(|k| k.key == key)
    }

    /// The kind of party with this key, by index.
    pub fn party(&self, key: &str) -> Option<usize> {
        self.rivals.parties.iter().position(|p| p.key == key)
    }
}

impl Content {
    /// This content with every kind of rival that has a simplified kit (`KitDef::Abilities`)
    /// given the whole bar of its base hero class instead, as the cutthroats have in
    /// `rivals.ron`: the playtest panel's "full kits" (spec §13). The big move is kept. Each
    /// kind's class is built again from its base, so nothing else about it changes.
    pub fn with_full_rival_kits(&self) -> Result<Content, String> {
        let mut out = self.clone();
        for i in 0..out.rivals.kinds.len() {
            let mut k = out.rivals.kinds[i].clone();
            if matches!(k.kit, KitDef::Class(_)) {
                continue;
            }
            k.kit = KitDef::Class(k.base.clone());
            let class = class_of(&mut k, &out)?;
            out.classes[k.class] = class;
            out.rivals.kinds[i] = k;
        }
        Ok(out)
    }
}
