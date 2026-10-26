//! Human-readable rendering of capture results.
//!
//! [`Captures`] and [`CaptureOccurrence`] print as an indented tree. Indexed
//! locations come first, in order, then named locations alphabetically.
//! Untaken locations print as `not taken`. Matched text longer than
//! [`DISPLAY_TEXT_LIMIT`] characters is shortened with `{}` and printed in
//! full with `{:#}`.

use std::fmt;

use super::{CaptureOccurrence, Captures};

/// Matched text longer than this is shortened in `{}` output; `{:#}` prints it in full.
const DISPLAY_TEXT_LIMIT: usize = 40;

impl Captures {
    /// True if the scope declares no capture locations at all.
    fn is_empty(&self) -> bool {
        self.indexed.is_empty() && self.by_name.is_empty()
    }
}

/// Quotes and escapes matched text, shortening it unless `full` is set.
fn quoted(text: &str, full: bool) -> String {
    if full || text.chars().count() <= DISPLAY_TEXT_LIMIT {
        format!("{text:?}")
    } else {
        let short: String = text.chars().take(DISPLAY_TEXT_LIMIT - 1).collect();
        format!("{short:?}...")
    }
}

/// Appends the lines describing one scope: indexed locations in order, then
/// named locations alphabetically
fn push_captures(captures: &Captures, depth: usize, full: bool, out: &mut Vec<String>) {
    if captures.is_empty() {
        out.push(format!("{}(no captures)", "  ".repeat(depth)));
        return;
    }

    for (index, occurrences) in captures.indexed.iter().enumerate() {
        push_location(&format!("[{index}]"), occurrences, depth, full, out);
    }

    let mut names: Vec<&String> = captures.by_name.keys().collect();
    names.sort();

    for name in names {
        push_location(&format!("{name:?}"), &captures.by_name[name], depth, full, out);
    }
}

/// Appends the lines describing one capture location and its occurrences.
fn push_location(
    label: &str,
    occurrences: &[CaptureOccurrence],
    depth: usize,
    full: bool,
    out: &mut Vec<String>,
) {
    let pad = "  ".repeat(depth);

    if occurrences.is_empty() {
        out.push(format!("{pad}{label}: not taken"));
        return;
    }

    out.push(format!("{pad}{label}:"));

    for occurrence in occurrences {
        push_occurrence(occurrence, depth + 1, full, out);
    }
}

/// Appends the lines describing one occurrence. Nested scopes without any
/// capture locations are left out, since there is nothing to show.
fn push_occurrence(
    occurrence: &CaptureOccurrence,
    depth: usize,
    full: bool,
    out: &mut Vec<String>
) {
    let pad = "  ".repeat(depth);

    match occurrence {
        CaptureOccurrence::Literal(text) => {
            out.push(format!("{pad}- literal {}", quoted(text, full)));
        }

        CaptureOccurrence::Group { matched, nested } => {
            out.push(format!("{pad}- group {}", quoted(matched, full)));

            if !nested.is_empty() {
                push_captures(nested, depth + 1, full, out);
            }
        }

        CaptureOccurrence::Alternative {
            matched,
            branch_index,
            nested,
        } => {
            out.push(format!(
                "{pad}- branch {branch_index} {}",
                quoted(matched, full)
            ));

            if !nested.is_empty() {
                push_captures(nested, depth + 1, full, out);
            }
        }
    }
}

impl fmt::Display for Captures {
    /// Prints the capture tree. Use `{:#}` to print matched text in full
    /// instead of shortening long text.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut lines = Vec::new();
        push_captures(self, 0, f.alternate(), &mut lines);
        write!(f, "{}", lines.join("\n"))
    }
}

impl fmt::Display for CaptureOccurrence {
    /// Prints one occurrence and everything nested inside it. Use `{:#}` to
    /// print matched text in full instead of shortening long text.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut lines = Vec::new();
        push_occurrence(self, 0, f.alternate(), &mut lines);
        write!(f, "{}", lines.join("\n"))
    }
}
