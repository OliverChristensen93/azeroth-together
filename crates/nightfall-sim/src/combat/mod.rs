//! The fight (spec §6). This first part is its arithmetic ([`rules`]); the rest of the module
//! comes in the next tasks.

mod rules;

pub use rules::Charges;
#[allow(unused_imports)]
pub(crate) use rules::{allowance, in_range, stands_right, within_arc, Dice};
