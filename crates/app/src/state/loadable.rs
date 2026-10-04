//! Something fetched: not asked for, on its way, here, or failed.

use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Loadable<T> {
    #[default]
    NotLoaded,
    Loading,
    Loaded(T),
    /// A sentence fit to show.
    Failed(String),
}

impl<T> Loadable<T> {
    #[cfg(test)]
    pub fn get(&self) -> Option<&T> {
        match self {
            Loadable::Loaded(value) => Some(value),
            _ => None,
        }
    }

    /// Whether a request should go out: nothing has been asked for, or the
    /// last attempt failed and opening the page again is the retry.
    pub fn needs_fetch(&self) -> bool {
        matches!(self, Loadable::NotLoaded | Loadable::Failed(_))
    }

    pub fn from_result<E: ToString>(result: Result<T, E>) -> Self {
        match result {
            Ok(value) => Loadable::Loaded(value),
            Err(error) => Loadable::Failed(error.to_string()),
        }
    }
}

/// Fetched pages by id, holding at most `cap` and dropping the one opened
/// longest ago. Memory stays bounded however long the app runs.
#[derive(Debug)]
pub struct PageCache<K, V> {
    entries: HashMap<K, (u64, Loadable<V>)>,
    cap: usize,
    clock: u64,
}

impl<K: Eq + Hash + Clone, V> PageCache<K, V> {
    pub fn new(cap: usize) -> Self {
        Self {
            entries: HashMap::new(),
            cap,
            clock: 0,
        }
    }

    pub fn get(&self, key: &K) -> &Loadable<V> {
        self.entries
            .get(key)
            .map_or(&Loadable::NotLoaded, |(_, page)| page)
    }

    /// Stores `page` and marks it the most recently used.
    pub fn insert(&mut self, key: K, page: Loadable<V>) {
        self.clock += 1;
        self.entries.insert(key, (self.clock, page));
        while self.entries.len() > self.cap {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, (used, _))| *used)
                .map(|(key, _)| key.clone());
            match oldest {
                Some(key) => self.entries.remove(&key),
                None => break,
            };
        }
    }

    /// The loaded page, to change in place.
    pub fn loaded_mut(&mut self, key: &K) -> Option<&mut V> {
        match self.entries.get_mut(key) {
            Some((_, Loadable::Loaded(page))) => Some(page),
            _ => None,
        }
    }

    /// Marks a page as just opened, so it outlives those opened before it.
    pub fn touch(&mut self, key: &K) {
        self.clock += 1;
        if let Some((used, _)) = self.entries.get_mut(key) {
            *used = self.clock;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_load_is_fetched_again() {
        assert!(Loadable::<u8>::NotLoaded.needs_fetch());
        assert!(Loadable::<u8>::Failed("no".into()).needs_fetch());
        assert!(!Loadable::<u8>::Loading.needs_fetch());
        assert!(!Loadable::Loaded(1).needs_fetch());
    }

    #[test]
    fn the_page_opened_longest_ago_is_dropped() {
        let mut cache = PageCache::new(2);
        cache.insert("a", Loadable::Loaded(1));
        cache.insert("b", Loadable::Loaded(2));
        cache.touch(&"a");
        cache.insert("c", Loadable::Loaded(3));
        assert_eq!(cache.get(&"a").get(), Some(&1));
        assert_eq!(cache.get(&"b"), &Loadable::NotLoaded);
        assert_eq!(cache.get(&"c").get(), Some(&3));
    }
}
