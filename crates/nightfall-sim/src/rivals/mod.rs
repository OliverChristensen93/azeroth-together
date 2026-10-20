//! Rivals (spec §7.4): parties of two or three bot adventurers with goals of their own, in the
//! manner of Single Player Tarkov's questing bots (§1.1). This task holds their content; the
//! later ones make each a hero of another side, driven by its bot.

mod content;
#[cfg(test)]
mod tests;

pub use content::{
    KitDef, PartyDef, PoiDef, RivalContent, RivalKindDef, RivalRules, RouteDef, RouteEnd,
    RoutePlace,
};
