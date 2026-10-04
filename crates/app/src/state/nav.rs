//! Where the person is, and where they have been.

/// How many pages back the history reaches before the oldest is dropped.
const HISTORY_CAP: usize = 60;

/// A surface of YouTube Music's made of shelves or tiles, addressed as
/// YouTube addresses it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Surface {
    pub id: String,
    /// Tells apart surfaces that share an id, as the moods' pages do.
    pub params: String,
    /// What to call it until, and unless, the page names itself.
    pub title: String,
}

impl Surface {
    /// What YouTube Music puts forward: new albums, top songs, trending.
    pub fn explore() -> Self {
        Self::named("FEmusic_explore", "Discover")
    }

    /// Every mood and genre, as tiles.
    pub fn moods() -> Self {
        Self::named("FEmusic_moods_and_genres", "Moods and genres")
    }

    fn named(id: &str, title: &str) -> Self {
        Self {
            id: id.to_owned(),
            params: String::new(),
            title: title.to_owned(),
        }
    }

    /// What the fetched page is kept under.
    pub fn key(&self) -> (String, String) {
        (self.id.clone(), self.params.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Page {
    Home,
    Search,
    Settings,
    Browse(Surface),
    /// Recently played.
    History,
    /// What's new: the release notes.
    Changelog,
    /// Listen Together.
    Together,
    /// Your listening.
    Stats,
    /// One of the mixes made from what has been played here.
    Mix(String),
    Album(String),
    Artist(String),
    /// Every song of an artist's, in an order of the listener's choosing.
    ArtistSongs(String),
    Playlist(String),
    Podcast(String),
}

/// A browser-style history: opening a page drops whatever was ahead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nav {
    history: Vec<Page>,
    index: usize,
}

impl Default for Nav {
    fn default() -> Self {
        Self {
            history: vec![Page::Home],
            index: 0,
        }
    }
}

impl Nav {
    pub fn page(&self) -> &Page {
        &self.history[self.index]
    }

    pub fn can_go_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.index + 1 < self.history.len()
    }

    /// Opening the page already shown is not a new history entry.
    pub fn open(&mut self, page: Page) {
        if &page == self.page() {
            return;
        }
        self.history.truncate(self.index + 1);
        self.history.push(page);
        if self.history.len() > HISTORY_CAP {
            self.history.remove(0);
        }
        self.index = self.history.len() - 1;
    }

    pub fn back(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    pub fn forward(&mut self) {
        if self.can_go_forward() {
            self.index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_and_forward_retrace_the_path() {
        let mut nav = Nav::default();
        nav.open(Page::Search);
        nav.open(Page::Settings);
        nav.back();
        assert_eq!(nav.page(), &Page::Search);
        nav.forward();
        assert_eq!(nav.page(), &Page::Settings);
        assert!(!nav.can_go_forward());
    }

    #[test]
    fn opening_a_page_drops_what_was_ahead() {
        let mut nav = Nav::default();
        nav.open(Page::Search);
        nav.back();
        nav.open(Page::Settings);
        assert!(!nav.can_go_forward());
        nav.back();
        assert_eq!(nav.page(), &Page::Home);
    }

    #[test]
    fn reopening_the_current_page_adds_nothing() {
        let mut nav = Nav::default();
        nav.open(Page::Home);
        assert!(!nav.can_go_back());
    }

    #[test]
    fn the_oldest_page_falls_off_a_full_history() {
        let mut nav = Nav::default();
        for step in 0..HISTORY_CAP * 2 {
            nav.open(if step % 2 == 0 {
                Page::Search
            } else {
                Page::Settings
            });
        }
        let mut steps_back = 0;
        while nav.can_go_back() {
            nav.back();
            steps_back += 1;
        }
        assert_eq!(steps_back, HISTORY_CAP - 1);
    }
}
