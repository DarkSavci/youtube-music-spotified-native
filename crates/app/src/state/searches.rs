//! Recent searches: the account's history and this computer's, as one list.

use spotified_client::models::SearchHistoryEntry;

/// How many recent searches are shown.
pub const RECENT_SHOWN: usize = 10;
/// How many this computer keeps of its own.
pub const RECENT_KEPT: usize = 8;

/// One row of the recent searches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentSearch {
    pub query: String,
    /// Removes the search from the account's history; empty for one only
    /// this computer knows.
    pub token: String,
}

fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// The two lists as one.
///
/// The account's history leads: it is the one list every device shares,
/// and searches made here are recorded in it too. The exception is this
/// computer's newest search while the account's list does not have it yet:
/// that is the most recent search of all, so it goes first. The rest of
/// this computer's follow the account's. A search in both keeps the
/// account's token, so removing it removes it everywhere.
pub fn merged(account: &[SearchHistoryEntry], local: &[String]) -> Vec<RecentSearch> {
    let mut out: Vec<RecentSearch> = Vec::new();
    let mut add = |query: &str, token: &str| {
        let query = query.trim();
        if query.is_empty() {
            return;
        }
        match out.iter_mut().find(|row| same(&row.query, query)) {
            Some(row) if row.token.is_empty() => row.token = token.to_owned(),
            Some(_) => {}
            None => out.push(RecentSearch {
                query: query.to_owned(),
                token: token.to_owned(),
            }),
        }
    };
    if let Some(newest) = local.first()
        && !account.iter().any(|entry| same(&entry.query, newest))
    {
        add(newest, "");
    }
    for entry in account {
        add(&entry.query, &entry.token);
    }
    for query in local {
        add(query, "");
    }
    out.truncate(RECENT_SHOWN);
    out
}

/// Puts a search at the head of this computer's list. Searching for the
/// same thing twice, whatever its case, does not fill the list with it.
/// Returns whether the list changed.
pub fn remember(local: &mut Vec<String>, query: &str) -> bool {
    let query = query.trim();
    if query.chars().count() < 2 || local.first().is_some_and(|first| first == query) {
        return false;
    }
    local.retain(|kept| !same(kept, query));
    local.insert(0, query.to_owned());
    local.truncate(RECENT_KEPT);
    true
}

/// Puts a search at the end of a list that does not have it yet, whatever
/// its case: one brought from elsewhere, older than those already there.
pub fn remember_after(local: &mut Vec<String>, query: &str) {
    let query = query.trim();
    if query.chars().count() >= 2 && !local.iter().any(|kept| same(kept, query)) {
        local.push(query.to_owned());
    }
}

/// Takes a search out of this computer's list, whatever its case: a merged
/// row may show the account's spelling of it.
pub fn forget(local: &mut Vec<String>, query: &str) {
    local.retain(|kept| !same(kept, query));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(query: &str, token: &str) -> SearchHistoryEntry {
        SearchHistoryEntry {
            query: query.into(),
            token: token.into(),
        }
    }

    #[test]
    fn the_accounts_history_leads_but_for_a_search_just_made_here() {
        let account = [entry("Daft Punk", "t1"), entry("bonobo", "t2")];
        let local = ["air".to_owned(), "daft punk".to_owned(), "moby".to_owned()];
        let rows = merged(&account, &local);
        let queries: Vec<&str> = rows.iter().map(|row| row.query.as_str()).collect();
        assert_eq!(queries, ["air", "Daft Punk", "bonobo", "moby"]);
        // In both lists: the account's token, so it is removed everywhere.
        assert_eq!(rows[1].token, "t1");
        assert_eq!(rows[0].token, "");
    }

    #[test]
    fn searching_again_moves_a_search_to_the_front() {
        let mut local = vec!["air".to_owned(), "moby".to_owned()];
        assert!(remember(&mut local, " MOBY "));
        assert_eq!(local, ["MOBY", "air"]);
        assert!(!remember(&mut local, "MOBY"));
        assert!(!remember(&mut local, "m"));
    }

    #[test]
    fn only_so_many_are_kept() {
        let mut local = Vec::new();
        for number in 0..20 {
            remember(&mut local, &format!("search {number}"));
        }
        assert_eq!(local.len(), RECENT_KEPT);
        assert_eq!(local[0], "search 19");
    }
}
