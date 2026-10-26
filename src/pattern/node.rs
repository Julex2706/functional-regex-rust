//! [`PatternNode`]: the node type of the pattern tree.
//!
//! Capture-bearing nodes (`Alternation`, `Capturing`, `Embedded`,
//! `Recursive`) each carry a [`CaptureSlot`] and open a scope of their own.
//! Slots and layouts are placeholders while building and are filled in by
//! compilation (`built::compile`).

use std::sync::Arc;

use super::set_item::SetItem;
use super::slot::CaptureSlot;
use crate::built::ScopeLayout;
use crate::recursion::RecursiveCell;

/// Pattern nodes used to build a regex.
#[derive(Debug, Clone)]
pub(crate) enum PatternNode {
    /// A literal string pattern.
    Literal(Vec<char>),

    /// A set or negated set matcher.
    CharClass { items: Vec<SetItem>, negated: bool },

    /// A border matcher such as word boundary style checks.
    Boundary { items: Vec<SetItem>, negated: bool },

    /// A concatenation of multiple subpatterns.
    Sequence(Vec<PatternNode>),

    /// A list of alternative branches. The alternative is a capture location
    /// ([`CaptureSlot::Index`] or [`CaptureSlot::Name`]) and every branch is a
    /// scope of its own.
    Alternation {
        slot: CaptureSlot,
        branches: Vec<PatternNode>,
        /// Describes the scope of each branch, one entry per branch. Empty
        /// placeholder until `build` compiles the pattern and fills in the
        /// real values.
        branch_layouts: Vec<Arc<ScopeLayout>>,
    },

    /// A group ([`CaptureSlot::Index`]) or capture ([`CaptureSlot::Name`]). Its
    /// inner pattern is a scope of its own.
    Capturing {
        slot: CaptureSlot,
        inner: Box<PatternNode>,
        /// Describes the inner scope. A default placeholder until `build`
        /// compiles the pattern and fills in the real value.
        layout: Arc<ScopeLayout>,
    },

    /// An embedded, already compiled, reusable regex (a scope of its own).
    Embedded {
        slot: CaptureSlot,
        inner: Box<PatternNode>,
        layout: Arc<ScopeLayout>,
    },

    /// A reference to a recursive regex (a scope of its own). The body and
    /// its layout are looked up in the cell at match time.
    Recursive {
        slot: CaptureSlot,
        cell: Arc<RecursiveCell>,
    },

    /// A quantified subpattern.
    Repeat {
        inner: Box<PatternNode>,
        min: usize,
        max: Option<usize>,
        lazy: bool,
    },
}
