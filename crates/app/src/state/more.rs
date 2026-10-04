//! What arrives a part at a time: the rest of Home, the rest of a long
//! playlist. Each remembers the token that fetches its next part, and the
//! tokens it has used, since one seen before would go round for ever.

use spotified_client::models::Shelf;

/// Where reading something page by page has got to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tail {
    /// The token for the next page; empty when there is no more.
    pub next: String,
    seen: Vec<String>,
    /// The next page has been asked for and is on its way.
    pub loading: bool,
    /// Why the last page did not come; it is asked for again only by hand.
    pub failed: Option<String>,
}

impl Tail {
    /// The tail of something whose first page ended with `next`.
    pub fn after(next: String) -> Self {
        Self {
            seen: vec![next.clone()],
            next,
            ..Self::default()
        }
    }

    /// The tail of something whose first page is on its way.
    pub fn starting() -> Self {
        Self {
            loading: true,
            ..Self::default()
        }
    }

    /// Whether there is more to read.
    pub fn more(&self) -> bool {
        !self.next.is_empty()
    }

    /// Whether the next page may be asked for now.
    pub fn idle(&self) -> bool {
        self.more() && !self.loading && self.failed.is_none()
    }

    /// A page has come, ending with `next`.
    pub fn arrived(&mut self, next: String) {
        self.loading = false;
        self.failed = None;
        if next.is_empty() || self.seen.contains(&next) {
            self.next.clear();
        } else {
            self.seen.push(next.clone());
            self.next = next;
        }
    }

    pub fn failed(&mut self, why: String) {
        self.loading = false;
        self.failed = Some(why);
    }
}

/// What is to be done with the whole of a playlist once every song of it
/// has been read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Whole {
    /// Play it from this place in it.
    Play(usize),
    /// Add it to the end of the queue, or right after what is playing.
    Queue { next: bool },
    /// Ask for a name for a new playlist that will hold its songs.
    NewPlaylist,
    /// Add its songs to another playlist.
    AddTo {
        playlist_id: String,
        playlist_title: String,
    },
}

/// The rest of Home, below what its first page brought.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HomeMore {
    pub shelves: Vec<Shelf>,
    pub tail: Tail,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_seen_before_ends_the_list() {
        let mut tail = Tail::after("a".into());
        assert!(tail.idle());
        tail.arrived("b".into());
        assert_eq!(tail.next, "b");
        tail.arrived("a".into());
        assert!(!tail.more());
    }

    #[test]
    fn a_failed_page_is_not_asked_for_again_by_itself() {
        let mut tail = Tail::after("a".into());
        tail.failed("no".into());
        assert!(tail.more());
        assert!(!tail.idle());
    }
}
