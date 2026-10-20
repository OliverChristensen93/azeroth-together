//! Rivals (spec §7.4): parties of two or three bot adventurers with goals of their own, in the
//! manner of Single Player Tarkov's questing bots (§1.1). This task holds their content; the
//! later ones make each a hero of another side, driven by its bot.

mod content;
// Their callers come with the rivals' minds (Task 8); until then only the tests do.
#[allow(dead_code)]
mod goals;
// The bot that calls it comes with the rivals' minds; until then only the tests do.
#[allow(dead_code)]
mod kit;
#[allow(dead_code)]
mod travel;
#[cfg(test)]
mod tests;

pub use content::{
    KitDef, PartyDef, PoiDef, RivalContent, RivalKindDef, RivalRules, RouteDef, RouteEnd,
    RoutePlace,
};
pub use travel::{RIVAL_BUDGET, RIVAL_POOL};
