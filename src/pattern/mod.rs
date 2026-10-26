//! The pattern tree: the crate-private representation that builders produce,
//! compilation numbers, and the matching engine walks.
//!
//! - `set_item`: items of a character class.
//! - `slot`: the identity of a capture location within a scope.
//! - `node`: the [`PatternNode`] enum itself.
//!
//! The behaviour on the tree lives with the phase that needs it:
//! `built::compile` (numbering), `matching::engine` (matching) and
//! `recursion::checks` (definedness).

mod node;
mod set_item;
mod slot;

pub(crate) use node::PatternNode;
pub(crate) use set_item::SetItem;
pub(crate) use slot::CaptureSlot;
