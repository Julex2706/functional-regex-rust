//! [`SetItem`]: one item of a character class.

/// A single item inside a set expression.
#[derive(Debug, Clone)]
pub(crate) enum SetItem {
    /// A literal set of characters.
    Chars(Vec<char>),

    /// A character range.
    Range(char, char),
}

impl SetItem {
    /// Returns true if the given character belongs to this set item.
    pub(crate) fn accepts(&self, ch: char) -> bool {
        match self {
            SetItem::Chars(chars) => chars.contains(&ch),
            SetItem::Range(from, to) => *from <= ch && ch <= *to,
        }
    }
}
