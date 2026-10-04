//! Which songs are liked, shown the moment the heart is pressed.
//!
//! A like goes to YouTube and takes a moment. Until it is confirmed, the
//! choice just made is what shows, and a list fetched in the meantime
//! cannot undo it: nothing the person did may flicker away and come back.

use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct Likes {
    /// As last heard from the core.
    confirmed: HashSet<String>,
    /// Changes sent and not yet answered: track id, and what was asked for.
    pending: HashMap<String, bool>,
}

impl Likes {
    pub fn is_liked(&self, track_id: &str) -> bool {
        match self.pending.get(track_id) {
            Some(wanted) => *wanted,
            None => self.confirmed.contains(track_id),
        }
    }

    /// Flips a track and returns what it now is. The change is pending
    /// until [`Likes::settle`] hears how it went.
    pub fn toggle(&mut self, track_id: &str) -> bool {
        let liked = !self.is_liked(track_id);
        self.pending.insert(track_id.to_owned(), liked);
        liked
    }

    /// The core's answer to a change. On failure the heart goes back to
    /// what the core last confirmed.
    pub fn settle(&mut self, track_id: &str, liked: bool, succeeded: bool) {
        // A later press on the same track is still in flight; its own
        // answer settles it.
        if self.pending.get(track_id) == Some(&liked) {
            self.pending.remove(track_id);
        }
        if succeeded {
            if liked {
                self.confirmed.insert(track_id.to_owned());
            } else {
                self.confirmed.remove(track_id);
            }
        }
    }

    /// The full list, freshly fetched. Pending changes stay on top of it.
    pub fn replace(&mut self, track_ids: impl IntoIterator<Item = String>) {
        self.confirmed = track_ids.into_iter().collect();
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_like_shows_before_it_is_confirmed() {
        let mut likes = Likes::default();
        assert!(likes.toggle("a"));
        assert!(likes.is_liked("a"));
    }

    #[test]
    fn a_list_fetched_meanwhile_cannot_undo_a_pending_like() {
        let mut likes = Likes::default();
        likes.toggle("a");
        likes.replace(Vec::new());
        assert!(likes.is_liked("a"));
        likes.settle("a", true, true);
        assert!(likes.is_liked("a"));
    }

    #[test]
    fn a_refused_like_goes_back() {
        let mut likes = Likes::default();
        likes.toggle("a");
        likes.settle("a", true, false);
        assert!(!likes.is_liked("a"));
    }

    #[test]
    fn an_unlike_is_confirmed_too() {
        let mut likes = Likes::default();
        likes.replace(["a".to_owned()]);
        assert!(!likes.toggle("a"));
        likes.settle("a", false, true);
        assert!(!likes.is_liked("a"));
    }

    #[test]
    fn an_earlier_answer_does_not_settle_a_later_press() {
        let mut likes = Likes::default();
        likes.toggle("a"); // like, sent
        likes.toggle("a"); // unlike, sent
        likes.settle("a", true, true); // the like's answer
        assert!(!likes.is_liked("a"));
        likes.settle("a", false, true);
        assert!(!likes.is_liked("a"));
    }
}
