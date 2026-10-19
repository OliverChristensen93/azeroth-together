//! The inn (spec §8.4, §8.5) as content of its own: the hub's place (its map, where heroes
//! appear, the rooms' bounds, the door, the evening's light), the NPCs at their places, the
//! trader's stock, the companions, the contracts Duskwood's characters give, and the stations
//! and their recipes. Read from `content/nightfall/hub.ron`, `contracts.ron` and `stations.ron`
//! beside the raid's content and checked against it, so the hub can move (to Darkshire, say) by
//! editing `hub.ron` alone.

use serde::Deserialize;

use crate::content::positive;
use crate::{Bounds, Content, ContentError, V2, V3};

/// The hub: a place on a map the heroes walk between raids.
#[derive(Clone, Debug, PartialEq)]
pub struct HubPlace {
    pub name: String,
    /// `Map.dbc` id.
    pub map: u32,
    /// Where heroes appear, WoW `(x, y, z)`.
    pub spawn: V3,
    /// Degrees from north toward west.
    pub facing: f32,
    /// The rooms: heroes are kept inside, as a raid keeps them inside its map.
    pub bounds: Bounds,
    /// The door's ring: stepping in opens the raid board.
    pub door: V2,
    pub door_radius: f32,
    /// Minute of the day the light stands at.
    pub minute: u32,
    /// How far the world is drawn, yards.
    /// Task 7's inn scene sets the engine's far clip from this when it lights the inn.
    pub view_distance: f32,
}

/// What an NPC does when clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpcRole {
    /// Keeps the stash: the stash panel.
    Banker,
    /// Buys and sells.
    Trader,
    /// Gives their contracts.
    Contracts,
    /// Keeps a station, by index.
    Station(usize),
}

/// An NPC's body: a character of this race and sex in this class's starting outfit (always in
/// the data, as the items' `Outfit` looks are).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Body {
    pub race: u8,
    pub sex: u8,
    pub class_id: u8,
}

/// An NPC standing in the inn.
#[derive(Clone, Debug, PartialEq)]
pub struct NpcDef {
    pub key: String,
    pub name: String,
    /// A line under the name.
    pub title: String,
    pub role: NpcRole,
    pub body: Body,
    pub at: V3,
    pub facing: f32,
}

/// The trader: what they sell and for how much, and the share of an item's value they pay.
#[derive(Clone, Debug, PartialEq)]
pub struct TraderDef {
    /// (item, gold each).
    pub stock: Vec<(usize, u32)>,
    pub sells_for: f32,
}

/// A companion (spec §8.2): a small pet that follows its hero and carries one stack never lost.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CompanionDef {
    pub key: String,
    pub name: String,
    /// `CreatureDisplayInfo.dbc` id.
    pub display: u32,
    #[serde(default = "one")]
    pub scale: f32,
}

fn one() -> f32 {
    1.0
}

/// One thing a contract asks.
#[derive(Clone, Debug, PartialEq)]
pub enum Objective {
    /// `count` enemies of kind `enemy` killed (within `near`'s circle when it has one), over any
    /// number of raids.
    Kill {
        enemy: usize,
        count: u32,
        near: Option<(V2, f32)>,
    },
    /// `count` of `item` brought to the giver: taken from the stash and loadout as the contract
    /// is turned in.
    Bring { item: usize, count: u32 },
    /// Out alive by `exit` (any exit for `None`), while the contract is taken.
    Extract { exit: Option<usize> },
}

/// What finishing a contract pays.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Reward {
    pub gold: u32,
    /// Stations unlocked, by index.
    pub unlocks: Vec<usize>,
}

/// A contract (spec §8.5): a route and a reason from one of Duskwood's characters.
#[derive(Clone, Debug, PartialEq)]
pub struct ContractDef {
    pub key: String,
    /// The NPC who gives it, by index.
    pub giver: usize,
    pub name: String,
    pub text: String,
    pub objectives: Vec<Objective>,
    pub reward: Reward,
    /// Contracts finished before this one is offered, by index.
    pub needs: Vec<usize>,
    pub repeatable: bool,
}

/// What a station's level gives.
#[derive(Clone, Debug, PartialEq)]
pub enum Gift {
    /// More stash room: this many slots added to `rules.run.stash_slots`.
    StashSlots(u16),
    /// A recipe, by index.
    Recipe(usize),
    /// The map table's look at the next raid.
    Intel,
    /// A companion, by index.
    Companion(usize),
}

/// One level of a station: what it costs and what it gives.
#[derive(Clone, Debug, PartialEq)]
pub struct StationLevel {
    pub gold: u32,
    /// Materials, from the stash: (item, count).
    pub items: Vec<(usize, u32)>,
    pub gives: Vec<Gift>,
}

/// A station in the inn (spec §8.5): upgraded level by level, kept forever.
#[derive(Clone, Debug, PartialEq)]
pub struct StationDef {
    pub key: String,
    pub name: String,
    /// Its keeper, by NPC index.
    pub keeper: usize,
    /// Upgradable from the start, not only once a contract unlocks it.
    pub open: bool,
    pub levels: Vec<StationLevel>,
}

/// Something a station makes.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeDef {
    pub key: String,
    /// Where it is made, by station index.
    pub station: usize,
    /// (item, count).
    pub makes: (usize, u16),
    pub needs: Vec<(usize, u32)>,
    pub gold: u32,
}

/// The inn, resolved.
#[derive(Clone, Debug)]
pub struct InnContent {
    pub place: HubPlace,
    pub npcs: Vec<NpcDef>,
    /// How many contracts a player may have taken at once.
    pub contracts_at_once: u8,
    pub trader: TraderDef,
    pub companions: Vec<CompanionDef>,
    pub contracts: Vec<ContractDef>,
    pub stations: Vec<StationDef>,
    pub recipes: Vec<RecipeDef>,
}

/// The text of each of the inn's files.
pub struct InnSources<'a> {
    pub hub: &'a str,
    pub contracts: &'a str,
    pub stations: &'a str,
}

#[derive(Deserialize)]
struct DoorFile {
    at: (f32, f32),
    radius: f32,
}

#[derive(Deserialize)]
struct PlaceFile {
    name: String,
    map: u32,
    spawn: (f32, f32, f32),
    #[serde(default)]
    facing: f32,
    bounds: Vec<(f32, f32)>,
    door: DoorFile,
    minute: u32,
    view_distance: f32,
}

#[derive(Deserialize)]
enum RoleFile {
    Banker,
    Trader,
    Contracts,
    Station(String),
}

#[derive(Deserialize)]
struct NpcFile {
    key: String,
    name: String,
    #[serde(default)]
    title: String,
    role: RoleFile,
    body: Body,
    at: (f32, f32, f32),
    #[serde(default)]
    facing: f32,
}

#[derive(Deserialize)]
struct StockFile {
    item: String,
    price: u32,
}

#[derive(Deserialize)]
struct TraderFile {
    stock: Vec<StockFile>,
    sells_for: f32,
}

#[derive(Deserialize)]
struct HubFile {
    place: PlaceFile,
    npcs: Vec<NpcFile>,
    contracts_at_once: u8,
    trader: TraderFile,
    #[serde(default)]
    companions: Vec<CompanionDef>,
}

#[derive(Deserialize)]
enum ObjectiveFile {
    Kill {
        enemy: String,
        count: u32,
        #[serde(default)]
        near: Option<(f32, f32, f32)>,
    },
    Bring {
        item: String,
        count: u32,
    },
    Extract {
        #[serde(default)]
        exit: Option<String>,
    },
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RewardFile {
    gold: u32,
    unlocks: Vec<String>,
}

#[derive(Deserialize)]
struct ContractFile {
    key: String,
    giver: String,
    name: String,
    #[serde(default)]
    text: String,
    objectives: Vec<ObjectiveFile>,
    #[serde(default)]
    reward: RewardFile,
    #[serde(default)]
    needs: Vec<String>,
    #[serde(default)]
    repeatable: bool,
}

#[derive(Deserialize)]
enum GiftFile {
    StashSlots(u16),
    Recipe(String),
    Intel,
    Companion(String),
}

#[derive(Deserialize)]
struct LevelFile {
    gold: u32,
    #[serde(default)]
    items: Vec<(String, u32)>,
    gives: Vec<GiftFile>,
}

#[derive(Deserialize)]
struct StationFile {
    key: String,
    name: String,
    keeper: String,
    #[serde(default)]
    open: bool,
    levels: Vec<LevelFile>,
}

#[derive(Deserialize)]
struct RecipeFile {
    key: String,
    station: String,
    makes: (String, u16),
    needs: Vec<(String, u32)>,
    #[serde(default)]
    gold: u32,
}

#[derive(Deserialize)]
struct StationsFile {
    stations: Vec<StationFile>,
    #[serde(default)]
    recipes: Vec<RecipeFile>,
}

fn parse<T: for<'de> Deserialize<'de>>(file: &'static str, text: &str) -> Result<T, ContentError> {
    ron::from_str(text).map_err(|e| ContentError::Parse {
        file,
        error: e.to_string(),
    })
}

/// A count is 1 or more.
fn count_ok(file: &str, key: &str, n: u32) -> Result<(), ContentError> {
    if n == 0 {
        return Err(ContentError::Invalid(format!(
            "{file}: \"{key}\": a count is 1 or more"
        )));
    }
    Ok(())
}

/// The first key listed twice, if any.
fn twice<'a>(keys: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut seen: Vec<&str> = Vec::new();
    for k in keys {
        if seen.contains(&k) {
            return Some(k);
        }
        seen.push(k);
    }
    None
}

impl InnContent {
    /// Reads the inn's three files and checks them against the raid's content: every key they
    /// name exists, the hub's people stand inside its bounds, numbers are finite and in range,
    /// stations are kept by who the keeper's role says, and no contract waits on a cycle.
    pub fn from_ron(src: InnSources, content: &Content) -> Result<Self, ContentError> {
        let hub: HubFile = parse("hub.ron", src.hub)?;
        let contracts: Vec<ContractFile> = parse("contracts.ron", src.contracts)?;
        let stations: StationsFile = parse("stations.ron", src.stations)?;
        let bad = |why: String| ContentError::Invalid(why);
        let item = |file: &str, key: &str| {
            content
                .item(key)
                .ok_or_else(|| bad(format!("{file}: no item \"{key}\" in items.ron")))
        };

        let p = &hub.place;
        let bounds = Bounds::new(p.bounds.iter().map(|&(x, y)| V2::new(x, y)).collect())
            .map_err(|why| bad(format!("hub.ron: {why}")))?;
        let spawn = V3::new(p.spawn.0, p.spawn.1, p.spawn.2);
        let door = V2::new(p.door.at.0, p.door.at.1);
        if !bounds.contains(spawn.ground()) || !bounds.contains(door) {
            return Err(bad(
                "hub.ron: the spawn and the door are inside the bounds".into()
            ));
        }
        if p.minute >= 1440 {
            return Err(bad("hub.ron: minute is below 1440".into()));
        }
        if !positive(p.door.radius) {
            return Err(bad("hub.ron: door radius is a number above 0".into()));
        }
        if !positive(p.view_distance) {
            return Err(bad("hub.ron: view_distance is a number above 0".into()));
        }
        if !p.facing.is_finite() || !spawn.z.is_finite() {
            return Err(bad(
                "hub.ron: facing and the spawn are finite numbers".into()
            ));
        }
        let place = HubPlace {
            name: p.name.clone(),
            map: p.map,
            spawn,
            facing: p.facing,
            bounds,
            door,
            door_radius: p.door.radius,
            minute: p.minute,
            view_distance: p.view_distance,
        };

        let names = |v: &[&str]| v.join(", ");
        if let Some(k) = twice(hub.npcs.iter().map(|n| n.key.as_str())) {
            return Err(bad(format!("hub.ron: NPC \"{k}\" is listed twice")));
        }
        if let Some(k) = twice(stations.stations.iter().map(|s| s.key.as_str())) {
            return Err(bad(format!(
                "stations.ron: station \"{k}\" is listed twice"
            )));
        }
        if let Some(k) = twice(stations.recipes.iter().map(|r| r.key.as_str())) {
            return Err(bad(format!("stations.ron: recipe \"{k}\" is listed twice")));
        }
        if let Some(k) = twice(contracts.iter().map(|c| c.key.as_str())) {
            return Err(bad(format!(
                "contracts.ron: contract \"{k}\" is listed twice"
            )));
        }
        if let Some(k) = twice(hub.companions.iter().map(|c| c.key.as_str())) {
            return Err(bad(format!("hub.ron: companion \"{k}\" is listed twice")));
        }
        let station_of = |key: &str| stations.stations.iter().position(|s| s.key == key);
        let mut npcs = Vec::new();
        for n in &hub.npcs {
            let at = V3::new(n.at.0, n.at.1, n.at.2);
            if !n.facing.is_finite() || !at.z.is_finite() {
                return Err(bad(format!(
                    "hub.ron: \"{}\" has a facing and a height that are finite numbers",
                    n.key
                )));
            }
            if !place.bounds.contains(at.ground()) {
                return Err(bad(format!(
                    "hub.ron: \"{}\" stands outside the bounds",
                    n.key
                )));
            }
            let role = match &n.role {
                RoleFile::Banker => NpcRole::Banker,
                RoleFile::Trader => NpcRole::Trader,
                RoleFile::Contracts => NpcRole::Contracts,
                RoleFile::Station(k) => NpcRole::Station(station_of(k).ok_or_else(|| {
                    bad(format!("hub.ron: \"{}\" keeps no station \"{k}\"", n.key))
                })?),
            };
            npcs.push(NpcDef {
                key: n.key.clone(),
                name: n.name.clone(),
                title: n.title.clone(),
                role,
                body: n.body,
                at,
                facing: n.facing,
            });
        }
        if npcs.iter().filter(|n| n.role == NpcRole::Banker).count() != 1 {
            return Err(bad("hub.ron: one NPC is the banker".into()));
        }
        if npcs.iter().filter(|n| n.role == NpcRole::Trader).count() > 1 {
            return Err(bad("hub.ron: at most one NPC is the trader".into()));
        }
        let npc_of = |file: &str, key: &str| {
            npcs.iter()
                .position(|n| n.key == key)
                .ok_or_else(|| bad(format!("{file}: no NPC \"{key}\" in hub.ron")))
        };

        let mut stock = Vec::new();
        for s in &hub.trader.stock {
            if s.price == 0 {
                return Err(bad(format!("hub.ron: \"{}\" sells for at least 1", s.item)));
            }
            stock.push((item("hub.ron: the trader", &s.item)?, s.price));
        }
        if !(0.0..=1.0).contains(&hub.trader.sells_for) {
            return Err(bad(
                "hub.ron: the trader pays a share of value, 0 to 1".into()
            ));
        }
        for c in &hub.companions {
            if !positive(c.scale) {
                return Err(bad(format!(
                    "hub.ron: companion \"{}\" has a scale above 0",
                    c.key
                )));
            }
        }
        let companion_of = |key: &str| {
            hub.companions
                .iter()
                .position(|c| c.key == key)
                .ok_or_else(|| bad(format!("stations.ron: no companion \"{key}\" in hub.ron")))
        };
        let recipe_of = |key: &str| {
            stations
                .recipes
                .iter()
                .position(|r| r.key == key)
                .ok_or_else(|| bad(format!("stations.ron: no recipe \"{key}\"")))
        };

        let mut station_defs = Vec::new();
        for (si, s) in stations.stations.iter().enumerate() {
            let keeper = npc_of(&format!("stations.ron: \"{}\"", s.key), &s.keeper)?;
            let keeps = npcs[keeper].role == NpcRole::Station(si)
                || (npcs[keeper].role == NpcRole::Banker && s.open);
            if !keeps {
                return Err(bad(format!(
                    "stations.ron: \"{}\"'s keeper \"{}\" must keep it (or be the banker, for a station open from the start)",
                    s.key, s.keeper
                )));
            }
            if s.levels.is_empty() {
                return Err(bad(format!("stations.ron: \"{}\" has a level", s.key)));
            }
            let mut levels = Vec::new();
            for l in &s.levels {
                let mut items = Vec::new();
                for (k, n) in &l.items {
                    count_ok("stations.ron", &s.key, *n)?;
                    items.push((item(&format!("stations.ron: \"{}\"", s.key), k)?, *n));
                }
                let mut gives = Vec::new();
                for g in &l.gives {
                    gives.push(match g {
                        GiftFile::StashSlots(n) => Gift::StashSlots(*n),
                        GiftFile::Recipe(k) => Gift::Recipe(recipe_of(k)?),
                        GiftFile::Intel => Gift::Intel,
                        GiftFile::Companion(k) => Gift::Companion(companion_of(k)?),
                    });
                }
                levels.push(StationLevel {
                    gold: l.gold,
                    items,
                    gives,
                });
            }
            station_defs.push(StationDef {
                key: s.key.clone(),
                name: s.name.clone(),
                keeper,
                open: s.open,
                levels,
            });
        }

        let mut recipes = Vec::new();
        for r in &stations.recipes {
            let station = station_of(&r.station)
                .ok_or_else(|| bad(format!("stations.ron: no station \"{}\"", r.station)))?;
            if r.makes.1 == 0 {
                return Err(bad(format!(
                    "stations.ron: recipe \"{}\" makes 1 or more",
                    r.key
                )));
            }
            let mut needs = Vec::new();
            for (k, n) in &r.needs {
                count_ok("stations.ron: recipe", &r.key, *n)?;
                needs.push((item(&format!("stations.ron: recipe \"{}\"", r.key), k)?, *n));
            }
            recipes.push(RecipeDef {
                key: r.key.clone(),
                station,
                makes: (
                    item(&format!("stations.ron: recipe \"{}\"", r.key), &r.makes.0)?,
                    r.makes.1,
                ),
                needs,
                gold: r.gold,
            });
        }
        // Buying and selling straight back must never make gold, and gold must not buy a recipe's
        // materials: otherwise a loop of buy, craft and sell would need no raid (spec §8.4).
        for &(it, price) in &stock {
            let def = &content.items[it];
            if (price as f32) <= def.value as f32 * hub.trader.sells_for {
                return Err(bad(format!(
                    "hub.ron: the trader sells \"{}\" for {price} but pays back more (value {}, share {})",
                    def.key, def.value, hub.trader.sells_for
                )));
            }
            if let Some(r) = recipes
                .iter()
                .find(|r| r.needs.iter().any(|&(n, _)| n == it))
            {
                return Err(bad(format!(
                    "hub.ron: the trader sells \"{}\", which recipe \"{}\" needs",
                    def.key, r.key
                )));
            }
        }
        // A recipe is made where it is learnt.
        for (si, s) in station_defs.iter().enumerate() {
            for g in s.levels.iter().flat_map(|l| &l.gives) {
                if let Gift::Recipe(r) = g {
                    if recipes[*r].station != si {
                        return Err(bad(format!(
                            "stations.ron: \"{}\" teaches \"{}\", which is made at another station",
                            s.key, recipes[*r].key
                        )));
                    }
                }
            }
        }

        let mut contract_defs = Vec::new();
        for c in &contracts {
            let giver = npc_of(&format!("contracts.ron: \"{}\"", c.key), &c.giver)?;
            if npcs[giver].role != NpcRole::Contracts {
                return Err(bad(format!(
                    "contracts.ron: \"{}\"'s giver \"{}\" gives no contracts",
                    c.key, c.giver
                )));
            }
            if c.objectives.is_empty() {
                return Err(bad(format!(
                    "contracts.ron: \"{}\" asks for something",
                    c.key
                )));
            }
            let mut objectives = Vec::new();
            for o in &c.objectives {
                objectives.push(match o {
                    ObjectiveFile::Kill { enemy, count, near } => {
                        if let Some((x, y, r)) = near {
                            if !(x.is_finite() && y.is_finite() && positive(*r)) {
                                return Err(bad(format!(
                                    "contracts.ron: \"{}\": near is a finite spot and a radius above 0",
                                    c.key
                                )));
                            }
                        }
                        count_ok("contracts.ron", &c.key, *count)?;
                        Objective::Kill {
                            enemy: content.enemy(enemy).ok_or_else(|| {
                                bad(format!(
                                    "contracts.ron: \"{}\": no enemy \"{enemy}\" in enemies.ron",
                                    c.key
                                ))
                            })?,
                            count: *count,
                            near: near.map(|(x, y, r)| (V2::new(x, y), r)),
                        }
                    }
                    ObjectiveFile::Bring { item: k, count } => {
                        count_ok("contracts.ron", &c.key, *count)?;
                        Objective::Bring {
                            item: item(&format!("contracts.ron: \"{}\"", c.key), k)?,
                            count: *count,
                        }
                    }
                    ObjectiveFile::Extract { exit } => Objective::Extract {
                        exit: match exit {
                            None => None,
                            Some(k) => Some(
                                content.map.exits.iter().position(|e| e.key == *k).ok_or_else(
                                    || {
                                        bad(format!(
                                            "contracts.ron: \"{}\": no exit \"{k}\" in map.ron",
                                            c.key
                                        ))
                                    },
                                )?,
                            ),
                        },
                    },
                });
            }
            let mut unlocks = Vec::new();
            for k in &c.reward.unlocks {
                unlocks.push(station_of(k).ok_or_else(|| {
                    bad(format!(
                        "contracts.ron: \"{}\": no station \"{k}\" to unlock",
                        c.key
                    ))
                })?);
            }
            contract_defs.push(ContractDef {
                key: c.key.clone(),
                giver,
                name: c.name.clone(),
                text: c.text.clone(),
                objectives,
                reward: Reward {
                    gold: c.reward.gold,
                    unlocks,
                },
                needs: Vec::new(),
                repeatable: c.repeatable,
            });
        }
        for (i, c) in contracts.iter().enumerate() {
            for k in &c.needs {
                let n = contracts.iter().position(|o| o.key == *k).ok_or_else(|| {
                    bad(format!(
                        "contracts.ron: \"{}\" needs \"{k}\", which is not a contract ({})",
                        c.key,
                        names(&contracts.iter().map(|o| o.key.as_str()).collect::<Vec<_>>())
                    ))
                })?;
                contract_defs[i].needs.push(n);
            }
        }

        // Contracts finished before this one is offered must be finishable themselves: peel off
        // those whose needs are all peeled; what is left waits on a cycle and can never be offered.
        let mut offered = vec![false; contract_defs.len()];
        while let Some(i) = (0..contract_defs.len())
            .find(|&i| !offered[i] && contract_defs[i].needs.iter().all(|&n| offered[n]))
        {
            offered[i] = true;
        }
        if let Some(start) = offered.iter().position(|&o| !o) {
            // Every contract left over needs another left over: follow the needs until one repeats.
            let mut walk = vec![start];
            loop {
                let last = *walk.last().expect("a walk has a start");
                let next = *contract_defs[last]
                    .needs
                    .iter()
                    .find(|&&n| !offered[n])
                    .expect("a contract left over needs another");
                if let Some(at) = walk.iter().position(|&w| w == next) {
                    walk.drain(..at);
                    walk.push(next);
                    break;
                }
                walk.push(next);
            }
            let cycle: Vec<&str> = walk
                .iter()
                .map(|&i| contract_defs[i].key.as_str())
                .collect();
            return Err(bad(format!(
                "contracts.ron: contracts need each other in a cycle and are never offered: {}",
                cycle.join(" -> ")
            )));
        }

        if hub.contracts_at_once == 0 {
            return Err(bad("hub.ron: contracts_at_once is 1 or more".into()));
        }

        Ok(Self {
            place,
            npcs,
            contracts_at_once: hub.contracts_at_once,
            trader: TraderDef {
                stock,
                sells_for: hub.trader.sells_for,
            },
            companions: hub.companions,
            contracts: contract_defs,
            stations: station_defs,
            recipes,
        })
    }

    /// An NPC's index by key.
    pub fn npc(&self, key: &str) -> Option<usize> {
        self.npcs.iter().position(|n| n.key == key)
    }

    /// A contract's index by key.
    pub fn contract(&self, key: &str) -> Option<usize> {
        self.contracts.iter().position(|c| c.key == key)
    }

    /// A station's index by key.
    pub fn station(&self, key: &str) -> Option<usize> {
        self.stations.iter().position(|s| s.key == key)
    }

    /// A recipe's index by key.
    pub fn recipe(&self, key: &str) -> Option<usize> {
        self.recipes.iter().position(|r| r.key == key)
    }

    /// A companion's index by key.
    pub fn companion(&self, key: &str) -> Option<usize> {
        self.companions.iter().position(|c| c.key == key)
    }

    /// The banker, by NPC index (the content has exactly one).
    pub fn banker(&self) -> usize {
        self.npcs
            .iter()
            .position(|n| n.role == NpcRole::Banker)
            .expect("checked as it loaded")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContentSources;

    fn raid() -> Content {
        raid_worth(0)
    }

    /// [`raid`], with the bread worth `bread` gold.
    fn raid_worth(bread: u32) -> Content {
        let items = r#"[
                (key: "tome", name: "Tome", kind: Valuable, icon: "a", weight: 1.0),
                (key: "cloth", name: "Cloth", kind: Material, icon: "a", weight: 0.1, stack: 20),
                (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 4),
                (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.2, stack: 20, value: WORTH),
                (key: "conjured_water", name: "Water", kind: Drink, icon: "a", weight: 0.1, stack: 20),
            ]"#
        .replace("WORTH", &bread.to_string());
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "test", name: "Test", map: 0,
                bounds: [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
                entries: [(50.0, 50.0, 0.0)],
                exits: [(key: "road", name: "The Road", at: (90.0, 90.0))])"#,
            rules: r#"(hero_level: 30, run: (conjured_drink: "conjured_water"),
                atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: r#"[(key: "worgen", name: "Worgen", display: 657, level: 27, hp: 900.0,
                walk_speed: 2.5, run_speed: 8.0)]"#,
            spawns: "[]",
            abilities: "[]",
            items: &items,
            loot: "()",
        })
        .unwrap()
    }

    const HUB: &str = r#"(
        place: (name: "Inn", map: 0, spawn: (10.0, 10.0, 0.0), facing: 0.0,
                bounds: [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)],
                door: (at: (19.0, 10.0), radius: 2.0), minute: 1170, view_distance: 200.0),
        npcs: [
            (key: "banker", name: "Farley", role: Banker, body: (race: 1, sex: 0, class_id: 1),
             at: (5.0, 5.0, 0.0)),
            (key: "trader", name: "Brog", role: Trader, body: (race: 1, sex: 0, class_id: 1),
             at: (5.0, 15.0, 0.0)),
            (key: "watch", name: "Ebonlock", role: Contracts, body: (race: 1, sex: 1, class_id: 2),
             at: (15.0, 5.0, 0.0)),
            (key: "smith", name: "Smith", role: Station("bench"), body: (race: 1, sex: 0, class_id: 1),
             at: (15.0, 15.0, 0.0)),
        ],
        contracts_at_once: 2,
        trader: (stock: [(item: "bread", price: 2)], sells_for: 0.5),
        companions: [(key: "hound", name: "Hound", display: 1052, scale: 0.6)],
    )"#;
    const CONTRACTS: &str = r#"[
        (key: "pack", giver: "watch", name: "Thin the pack",
         objectives: [Kill(enemy: "worgen", count: 3, near: Some((50.0, 50.0, 20.0)))],
         reward: (gold: 60, unlocks: ["bench"]), repeatable: true),
        (key: "tome", giver: "watch", name: "The tome", needs: ["pack"],
         objectives: [Bring(item: "tome", count: 1), Extract(exit: Some("road"))],
         reward: (gold: 100)),
    ]"#;
    const STATIONS: &str = r#"(
        stations: [
            (key: "stash", name: "The Stash", keeper: "banker", open: true,
             levels: [(gold: 50, items: [("cloth", 10)], gives: [StashSlots(20)])]),
            (key: "bench", name: "Workbench", keeper: "smith",
             levels: [(gold: 40, gives: [Recipe("pouch")]),
                      (gold: 90, gives: [Intel, Companion("hound")])]),
        ],
        recipes: [(key: "pouch", station: "bench", makes: ("pouch", 1), needs: [("cloth", 6)],
                   gold: 5)],
    )"#;

    fn load(hub: &str, contracts: &str, stations: &str) -> Result<InnContent, ContentError> {
        InnContent::from_ron(
            InnSources {
                hub,
                contracts,
                stations,
            },
            &raid(),
        )
    }

    #[test]
    fn the_inn_loads() {
        let inn = load(HUB, CONTRACTS, STATIONS).unwrap();
        assert_eq!(inn.banker(), 0);
        assert_eq!(inn.npcs[3].role, NpcRole::Station(1));
        assert_eq!(inn.trader.stock, vec![(3, 2)]);
        assert_eq!(inn.contracts[1].needs, vec![0]);
        assert_eq!(
            inn.contracts[1].objectives,
            vec![
                Objective::Bring { item: 0, count: 1 },
                Objective::Extract { exit: Some(0) }
            ]
        );
        assert_eq!(inn.contracts[0].reward.unlocks, vec![1]);
        assert_eq!(
            inn.stations[1].levels[1].gives,
            vec![Gift::Intel, Gift::Companion(0)]
        );
        assert_eq!(inn.recipes[0].station, 1);
        assert_eq!(inn.recipe("pouch"), Some(0));
        assert_eq!(inn.place.door, V2::new(19.0, 10.0));
        assert_eq!(inn.place.minute, 1170);
    }

    #[test]
    fn the_hub_is_wherever_its_content_puts_it() {
        // Darkshire instead of Goldshire: only hub.ron changes.
        let moved = HUB
            .replace("(10.0, 10.0, 0.0), facing", "(60.0, 60.0, 5.0), facing")
            .replace(
                "[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]",
                "[(50.0, 50.0), (70.0, 50.0), (70.0, 70.0), (50.0, 70.0)]",
            )
            .replace("(19.0, 10.0)", "(69.0, 60.0)")
            .replace("(5.0, 5.0, 0.0)", "(55.0, 55.0, 5.0)")
            .replace("(5.0, 15.0, 0.0)", "(55.0, 65.0, 5.0)")
            .replace("(15.0, 5.0, 0.0)", "(65.0, 55.0, 5.0)")
            .replace("(15.0, 15.0, 0.0)", "(65.0, 65.0, 5.0)");
        let inn = load(&moved, CONTRACTS, STATIONS).unwrap();
        assert_eq!(inn.place.spawn, V3::new(60.0, 60.0, 5.0));
        assert!(inn.place.bounds.contains(V2::new(60.0, 60.0)));
    }

    #[test]
    fn a_hub_with_its_people_outside_it_is_refused() {
        for bad in [
            HUB.replace("(5.0, 5.0, 0.0)", "(50.0, 5.0, 0.0)"),
            HUB.replace("(19.0, 10.0)", "(29.0, 10.0)"),
            HUB.replace("(10.0, 10.0, 0.0), facing", "(30.0, 10.0, 0.0), facing"),
            HUB.replace("minute: 1170", "minute: 1440"),
            HUB.replace("role: Banker", "role: Trader"),
        ] {
            assert!(
                matches!(
                    load(&bad, CONTRACTS, STATIONS),
                    Err(ContentError::Invalid(_))
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn contracts_and_stations_naming_what_is_not_there_are_refused() {
        for (contracts, stations) in [
            (
                CONTRACTS.replace("enemy: \"worgen\"", "enemy: \"dragon\""),
                STATIONS.to_string(),
            ),
            (
                CONTRACTS.replace(
                    "giver: \"watch\", name: \"Thin",
                    "giver: \"smith\", name: \"Thin",
                ),
                STATIONS.to_string(),
            ),
            (
                CONTRACTS.replace("needs: [\"pack\"]", "needs: [\"nothing\"]"),
                STATIONS.to_string(),
            ),
            (
                CONTRACTS.replace("exit: Some(\"road\")", "exit: Some(\"sea\")"),
                STATIONS.to_string(),
            ),
            (
                CONTRACTS.replace("count: 3", "count: 0"),
                STATIONS.to_string(),
            ),
            (
                CONTRACTS.to_string(),
                STATIONS.replace("Recipe(\"pouch\")", "Recipe(\"cake\")"),
            ),
            (
                CONTRACTS.to_string(),
                STATIONS.replace("keeper: \"smith\"", "keeper: \"watch\""),
            ),
            (
                CONTRACTS.to_string(),
                STATIONS.replace("station: \"bench\"", "station: \"stash\""),
            ),
        ] {
            assert!(
                matches!(
                    load(HUB, &contracts, &stations),
                    Err(ContentError::Invalid(_))
                ),
                "{contracts}\n{stations}"
            );
        }
    }

    /// Each number that could be NaN, infinite, negative or zero is refused, naming the field.
    #[test]
    fn invalid_inn_numbers_are_refused_naming_the_field() {
        let hub = |from: &str, to: &str| {
            (
                HUB.replace(from, to),
                CONTRACTS.to_string(),
                STATIONS.to_string(),
            )
        };
        let contracts = |from: &str, to: &str| {
            (
                HUB.to_string(),
                CONTRACTS.replace(from, to),
                STATIONS.to_string(),
            )
        };
        let stations = |from: &str, to: &str| {
            (
                HUB.to_string(),
                CONTRACTS.to_string(),
                STATIONS.replace(from, to),
            )
        };
        let near = "near: Some((50.0, 50.0, 20.0))";
        for (field, (hub, contracts, stations)) in [
            ("door radius", hub("radius: 2.0", "radius: inf")),
            ("door radius", hub("radius: 2.0", "radius: NaN")),
            ("door radius", hub("radius: 2.0", "radius: 0.0")),
            (
                "view_distance",
                hub("view_distance: 200.0", "view_distance: inf"),
            ),
            (
                "view_distance",
                hub("view_distance: 200.0", "view_distance: NaN"),
            ),
            (
                "view_distance",
                hub("view_distance: 200.0", "view_distance: -1.0"),
            ),
            ("facing", hub("facing: 0.0,", "facing: NaN,")),
            ("facing", hub("facing: 0.0,", "facing: inf,")),
            (
                "facing",
                hub("at: (5.0, 5.0, 0.0))", "at: (5.0, 5.0, 0.0), facing: NaN)"),
            ),
            ("scale", hub("scale: 0.6", "scale: inf")),
            ("scale", hub("scale: 0.6", "scale: NaN")),
            ("scale", hub("scale: 0.6", "scale: -0.5")),
            ("share of value", hub("sells_for: 0.5", "sells_for: NaN")),
            ("share of value", hub("sells_for: 0.5", "sells_for: 1.5")),
            ("near", contracts(near, "near: Some((50.0, 50.0, NaN))")),
            ("near", contracts(near, "near: Some((50.0, 50.0, inf))")),
            ("near", contracts(near, "near: Some((50.0, 50.0, -3.0))")),
            ("near", contracts(near, "near: Some((50.0, 50.0, 0.0))")),
            ("near", contracts(near, "near: Some((inf, 50.0, 20.0))")),
            ("recipe", stations("[(\"cloth\", 6)]", "[(\"cloth\", 0)]")),
        ] {
            match load(&hub, &contracts, &stations) {
                Err(ContentError::Invalid(why)) => {
                    assert!(why.contains(field), "{field}: {why}");
                }
                other => panic!("{field}: expected a refusal, got {other:?}"),
            }
        }
    }

    /// The trader must not turn gold into more gold, nor sell what a recipe consumes.
    #[test]
    fn a_trader_that_could_mint_gold_is_refused() {
        // Bread worth 10, paid back at half: 5. A price of 2 makes 3 gold on every loaf.
        match InnContent::from_ron(
            InnSources {
                hub: HUB,
                contracts: CONTRACTS,
                stations: STATIONS,
            },
            &raid_worth(10),
        ) {
            Err(ContentError::Invalid(why)) => assert!(why.contains("pays back more"), "{why}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
        // Priced above what it pays back (6 > 5), the same bread is fine; so is an exact tie not.
        let dearer = HUB.replace("price: 2", "price: 6");
        assert!(load_with(&dearer, &raid_worth(10)).is_ok());
        let tie = HUB.replace("price: 2", "price: 5");
        assert!(
            load_with(&tie, &raid_worth(10)).is_err(),
            "a tie profits nothing but is not a gain"
        );
        // Cloth is what the pouch recipe needs.
        let cloth = HUB.replace("item: \"bread\", price: 2", "item: \"cloth\", price: 2");
        match load(&cloth, CONTRACTS, STATIONS) {
            Err(ContentError::Invalid(why)) => assert!(why.contains("recipe"), "{why}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    fn load_with(hub: &str, content: &Content) -> Result<InnContent, ContentError> {
        InnContent::from_ron(
            InnSources {
                hub,
                contracts: CONTRACTS,
                stations: STATIONS,
            },
            content,
        )
    }

    /// A contract that needs itself, or two that need each other, could never be offered.
    #[test]
    fn contracts_that_wait_on_a_cycle_are_refused() {
        for (contracts, named) in [
            (
                CONTRACTS.replace(
                    "name: \"Thin the pack\",",
                    "name: \"Thin the pack\", needs: [\"pack\"],",
                ),
                vec!["pack"],
            ),
            (
                CONTRACTS.replace(
                    "name: \"Thin the pack\",",
                    "name: \"Thin the pack\", needs: [\"tome\"],",
                ),
                vec!["pack", "tome"],
            ),
        ] {
            match load(HUB, &contracts, STATIONS) {
                Err(ContentError::Invalid(why)) => {
                    assert!(why.contains("cycle"), "{why}");
                    for key in named {
                        assert!(why.contains(key), "{key}: {why}");
                    }
                }
                other => panic!("expected a refusal, got {other:?}"),
            }
        }
        // A chain that is not a cycle loads.
        assert!(load(HUB, CONTRACTS, STATIONS).is_ok());
    }

    /// Each kind of key that names nothing is refused, with the key in the message.
    #[test]
    fn each_unknown_key_is_refused_naming_it() {
        let hub = |from: &str, to: &str| {
            (
                HUB.replace(from, to),
                CONTRACTS.to_string(),
                STATIONS.to_string(),
            )
        };
        let contracts = |from: &str, to: &str| {
            (
                HUB.to_string(),
                CONTRACTS.replace(from, to),
                STATIONS.to_string(),
            )
        };
        let stations = |from: &str, to: &str| {
            (
                HUB.to_string(),
                CONTRACTS.to_string(),
                STATIONS.replace(from, to),
            )
        };
        for (what, (hub, contracts, stations)) in [
            (
                "a contract's giver",
                contracts(
                    "giver: \"watch\", name: \"Thin",
                    "giver: \"ghost\", name: \"Thin",
                ),
            ),
            (
                "a station's keeper",
                stations("keeper: \"smith\"", "keeper: \"ghost\""),
            ),
            (
                "a recipe's station",
                stations("station: \"bench\"", "station: \"ghost\""),
            ),
            (
                "the trader's stock",
                hub("item: \"bread\"", "item: \"ghost\""),
            ),
            (
                "a station level's item",
                stations("(\"cloth\", 10)", "(\"ghost\", 10)"),
            ),
            (
                "a recipe's product",
                stations("makes: (\"pouch\", 1)", "makes: (\"ghost\", 1)"),
            ),
            (
                "a recipe's material",
                stations("needs: [(\"cloth\", 6)]", "needs: [(\"ghost\", 6)]"),
            ),
            (
                "a Bring objective",
                contracts("Bring(item: \"tome\"", "Bring(item: \"ghost\""),
            ),
            (
                "an unlocked station",
                contracts("unlocks: [\"bench\"]", "unlocks: [\"ghost\"]"),
            ),
            (
                "a Companion gift",
                stations("Companion(\"hound\")", "Companion(\"ghost\")"),
            ),
            (
                "an NPC's Station role",
                hub("Station(\"bench\")", "Station(\"ghost\")"),
            ),
        ] {
            match load(&hub, &contracts, &stations) {
                Err(ContentError::Invalid(why)) => {
                    assert!(why.contains("\"ghost\""), "{what}: {why}");
                }
                other => panic!("{what}: expected a refusal, got {other:?}"),
            }
        }
    }

    #[test]
    fn at_least_one_contract_at_once_and_errors_name_their_contract() {
        match load(
            &HUB.replace("contracts_at_once: 2", "contracts_at_once: 0"),
            CONTRACTS,
            STATIONS,
        ) {
            Err(ContentError::Invalid(why)) => assert!(why.contains("contracts_at_once"), "{why}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
        // A contract's own key is in what is wrong with it: its giver, enemy, exit, item and station.
        for (contracts, owner) in [
            (
                CONTRACTS.replace(
                    "giver: \"watch\", name: \"Thin",
                    "giver: \"ghost\", name: \"Thin",
                ),
                "pack",
            ),
            (
                CONTRACTS.replace("enemy: \"worgen\"", "enemy: \"ghost\""),
                "pack",
            ),
            (
                CONTRACTS.replace("exit: Some(\"road\")", "exit: Some(\"ghost\")"),
                "tome",
            ),
            (
                CONTRACTS.replace("Bring(item: \"tome\"", "Bring(item: \"ghost\""),
                "tome",
            ),
            (
                CONTRACTS.replace("unlocks: [\"bench\"]", "unlocks: [\"ghost\"]"),
                "pack",
            ),
        ] {
            match load(HUB, &contracts, STATIONS) {
                Err(ContentError::Invalid(why)) => {
                    assert!(why.contains("\"ghost\""), "{why}");
                    assert!(why.contains(&format!("\"{owner}\"")), "{owner}: {why}");
                }
                other => panic!("expected a refusal, got {other:?}"),
            }
        }
    }
}
