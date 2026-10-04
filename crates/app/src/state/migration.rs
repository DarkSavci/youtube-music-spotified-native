//! Moving in from the Electron app, as the views see it: what was found
//! there, what is ticked to be brought, and how the bringing is going.

use crate::migrate::{Found, Kinds, Line, Progress};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Migration {
    /// What the Electron app's profile holds; `None` when there is no such
    /// profile, or it has not been looked at yet.
    pub found: Option<Found>,
    /// The kinds ticked to be brought.
    pub choice: Kinds,
    /// The kinds brought by an earlier run.
    pub brought: Kinds,
    /// How far the run under way has got; `None` when none is.
    pub running: Option<Progress>,
    /// What the last run came to, until the next is chosen.
    pub outcome: Option<Vec<Line>>,
}

impl Migration {
    /// Ticks what is worth bringing now: what there is, less what is here.
    pub fn suggest(&mut self) {
        self.choice = self
            .found
            .as_ref()
            .map(|found| found.suggested(self.brought))
            .unwrap_or_default();
    }
}
