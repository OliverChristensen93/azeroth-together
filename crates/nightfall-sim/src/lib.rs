//! Nightfall's game rules: heroes in Duskwood at night, as a plain-Rust simulation stepped at a
//! fixed [`TICK_HZ`].
//!
//! Nothing here knows about Bevy, rendering, WoW's file formats or the survivors games
//! (`together-sim`). The world is WoW's own `(x, y)` ground plane in yards; the client puts every
//! unit on the terrain. That split lets one machine (the co-op host) run the game while every
//! player's client only draws it. Sub-project 1 has heroes, their poses and the map's edge;
//! sub-project 2 adds the baked height map (walls, floors, sight and paths) and enemies that walk it; senses,
//! combat and loot come in later sub-projects (`docs/superpowers/specs/2026-10-16-nightfall-design.md`).

mod abilities;
mod bounds;
mod combat;
mod content;
mod contracts;
mod enemies;
mod heightmap;
mod inn;
mod inventory;
mod items;
mod math;
pub mod messages;
mod night;
mod playtest;
mod profile;
mod raid;
mod rivals;
mod senses;
mod showcase;
mod sight;
mod stations;
mod world;

pub use abilities::{
    AbilityDef, Aim, ControlKind, Creature, Effect, Facing, Moving, Needs, Shape, Sound, Tag,
    TelegraphDef,
};
pub use bounds::Bounds;
pub use combat::{
    CastPress, Casting, Charges, Cooldown, EnemyFight, Event, HeroFight, Life, Refusal, Stop,
    Telegraph, UnitRef,
};
pub use content::{
    Atmosphere, AwarenessRules, Behaviour, ClassDef, CombatRules, Content, ContentError,
    ContentSources, DeathLoss, DodgeRules, EnemyDef, ExitDef, HeroLight, LitArea, LoudnessTable,
    MapDef, MoveDef, Rules, RunRules, SenseRules, SpawnDef, StanceTable, StepNoise, Swing,
    WeaponDef, WorldRules,
};
pub use contracts::ContractError;
pub use enemies::{Enemy, ERRAND_BUDGET, PATROL_BUDGET, STEP_FLOORS, WANDER_BUDGET};
pub use heightmap::{
    Floors, Grid, HeightMap, HeightMapBuilder, Navigator, PathPlan, Span, SpanList, CELL, HEADROOM,
    MAX_FLOORS, MAX_STEP, MERGE_GAP, TERRAIN_DEPTH,
};
pub use inn::{
    Body, CompanionDef, ContractDef, Gift, HubPlace, InnContent, InnSources, NpcDef, NpcRole,
    Objective, RecipeDef, Reward, StationDef, StationLevel, TraderDef,
};
pub use inventory::{Death, InvError, Inventory, Place, Stack, MAX_BAGS};
pub use items::{
    Bonus, ContainerKindDef, EquipSlot, ItemDef, ItemKind, ItemUse, LightDef, Look, LootContent,
    PlacedContainer, Table, TableEntry,
};
pub use math::{V2, V3};
pub use night::{
    fog_of, CueSound, ExitRoll, Fog, HuntRules, MonsterDef, NightRules, NightView, RavenRules,
    RoadDef, ScentRules, StageDef, StepSound, WeatherDef, ZoneDef, FLOCK_FLOWN, FLOCK_GONE,
    FLOCK_SETTLED,
};
pub use playtest::{
    death_loss_key, next_of, MonstersOut, NightSpeed, Playtest, RivalKits, RivalsOut, DEATH_LOSSES,
};
pub use profile::{
    free_kit, inventory_of, saved_inventory, ContractState, InnProgress, Profile, ProfileError,
    SavedInventory, SavedRaid, SavedRecovery, SavedStack, StashError, PROFILE_VERSION,
};
pub use raid::{
    burden, Act, ActPress, Carry, ContainerMark, ExitMark, ExitPlan, Fate, HeroOutcome, Holder,
    Loadout, LootView, OpenView, RaidRoll, Recovery,
};
pub use rivals::{
    KitDef, PartyDef, PoiDef, RivalContent, RivalKindDef, RivalMark, RivalOut, RivalRules,
    RivalView, RouteDef, RouteEnd, RoutePlace, Side, RIVAL_BASE, RIVAL_BUDGET, RIVAL_POOL,
};
pub use senses::{Alert, Gait, Loudness, Mark, Stance};
pub use showcase::{plan as showcase_plan, ShowcasePlan};
pub use sight::{CHEST_HEIGHT, EYE_HEIGHT, KEEP_EXTRA};
pub use stations::StationError;
pub use world::{Hero, HeroPose, PlayerId, PlayerInput, Snapshot, World, WorldError, DT, TICK_HZ};
