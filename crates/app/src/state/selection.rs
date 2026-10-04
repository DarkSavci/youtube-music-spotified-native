//! Which rows of a track list are selected.
//!
//! One list at a time, as in a file manager: a click selects a row, Ctrl
//! adds or removes one, Shift takes everything from the last plain click.

use std::collections::BTreeSet;

/// How a click changes the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Select {
    /// A plain click: this row alone.
    Only,
    /// With Ctrl: add this row, or remove it.
    Toggle,
    /// With Shift: everything between the anchor and this row.
    Range,
}

#[derive(Debug, Default)]
pub struct Selection {
    /// Which list the rows belong to. Lists are told apart by a number the
    /// view derives from where they sit on the page.
    list: u64,
    rows: BTreeSet<usize>,
    /// Where a Shift-click or Shift-arrow extends from.
    anchor: usize,
    /// The row last moved to, which arrows step from.
    cursor: usize,
}

impl Selection {
    pub fn contains(&self, list: u64, row: usize) -> bool {
        self.list == list && self.rows.contains(&row)
    }

    /// The selected rows of `list`, in order. Empty for any other list.
    pub fn rows(&self, list: u64) -> impl Iterator<Item = usize> + '_ {
        let ours = self.list == list;
        self.rows.iter().copied().filter(move |_| ours)
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn clear(&mut self) {
        self.rows.clear();
    }

    pub fn click(&mut self, list: u64, row: usize, how: Select) {
        if self.list != list {
            *self = Self {
                list,
                anchor: row,
                ..Self::default()
            };
        }
        match how {
            // Clicking the one selected row again lets go of it.
            Select::Only if self.rows.len() == 1 && self.rows.contains(&row) => self.rows.clear(),
            Select::Only => {
                self.rows = BTreeSet::from([row]);
                self.anchor = row;
            }
            Select::Toggle => {
                if !self.rows.remove(&row) {
                    self.rows.insert(row);
                }
                self.anchor = row;
            }
            Select::Range => {
                let (from, to) = (self.anchor.min(row), self.anchor.max(row));
                self.rows = (from..=to).collect();
            }
        }
        self.cursor = row;
    }

    pub fn select_all(&mut self, list: u64, len: usize) {
        *self = Self {
            list,
            rows: (0..len).collect(),
            anchor: 0,
            cursor: len.saturating_sub(1),
        };
    }

    /// Moves by `step` rows within a list of `len`, as an arrow key does.
    /// With `extend`, the range from the anchor grows or shrinks instead.
    /// Returns the row moved to, for the view to bring into sight.
    pub fn step(&mut self, list: u64, step: isize, len: usize, extend: bool) -> Option<usize> {
        if self.list != list || self.rows.is_empty() || len == 0 {
            return None;
        }
        let row = self.cursor.saturating_add_signed(step).min(len - 1);
        self.click(list, row, if extend { Select::Range } else { Select::Only });
        // A plain step onto the one selected row must not let go of it.
        if self.rows.is_empty() {
            self.rows.insert(row);
        }
        Some(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(selection: &Selection) -> Vec<usize> {
        selection.rows(1).collect()
    }

    #[test]
    fn a_click_selects_one_row_and_a_second_click_lets_go() {
        let mut selection = Selection::default();
        selection.click(1, 3, Select::Only);
        assert_eq!(rows(&selection), [3]);
        selection.click(1, 5, Select::Only);
        assert_eq!(rows(&selection), [5]);
        selection.click(1, 5, Select::Only);
        assert!(selection.is_empty());
    }

    #[test]
    fn ctrl_adds_and_removes() {
        let mut selection = Selection::default();
        selection.click(1, 1, Select::Only);
        selection.click(1, 4, Select::Toggle);
        assert_eq!(rows(&selection), [1, 4]);
        selection.click(1, 1, Select::Toggle);
        assert_eq!(rows(&selection), [4]);
    }

    #[test]
    fn shift_takes_the_range_from_the_last_plain_click_either_way() {
        let mut selection = Selection::default();
        selection.click(1, 4, Select::Only);
        selection.click(1, 6, Select::Range);
        assert_eq!(rows(&selection), [4, 5, 6]);
        selection.click(1, 2, Select::Range);
        assert_eq!(rows(&selection), [2, 3, 4]);
    }

    #[test]
    fn another_list_starts_afresh() {
        let mut selection = Selection::default();
        selection.click(1, 4, Select::Only);
        selection.click(2, 0, Select::Toggle);
        assert!(!selection.contains(1, 4));
        assert!(selection.contains(2, 0));
    }

    #[test]
    fn arrows_move_within_the_list_and_shift_extends() {
        let mut selection = Selection::default();
        selection.click(1, 1, Select::Only);
        assert_eq!(selection.step(1, 1, 3, false), Some(2));
        assert_eq!(rows(&selection), [2]);
        // Already on the last row: it stays selected.
        assert_eq!(selection.step(1, 1, 3, false), Some(2));
        assert_eq!(rows(&selection), [2]);
        assert_eq!(selection.step(1, -2, 3, true), Some(0));
        assert_eq!(rows(&selection), [0, 1, 2]);
    }

    #[test]
    fn arrows_do_nothing_without_a_selection() {
        let mut selection = Selection::default();
        assert_eq!(selection.step(1, 1, 10, false), None);
    }
}
