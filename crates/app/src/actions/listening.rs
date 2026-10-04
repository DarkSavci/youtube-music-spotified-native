//! The listening page's lookup and details, the recent searches, and the
//! release notes shown over the page.

use super::{Action, Effect};
use crate::backend::{Request, Response};
use crate::state::{Dialog, Loadable, StatSelection, State, searches};

/// How many artists have their pictures fetched at the same time.
const PHOTOS_AT_ONCE: usize = 2;

pub(super) fn listening(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        Action::SetStatsLookup(text) => {
            let asked = text.trim().to_owned();
            state.stats_page.lookup_text = text;
            state.stats_page.lookup_open = true;
            if asked.is_empty() {
                state.stats_page.lookup = None;
                return Vec::new();
            }
            // Asked once the typing pauses, as the Electron app did: the
            // answers to half a word are not worth the asking.
            vec![Effect::DebounceStatsLookup]
        }
        Action::RunStatsLookup => {
            let asked = state.stats_page.lookup_text.trim().to_owned();
            let answered = matches!(&state.stats_page.lookup, Some((text, _)) if *text == asked);
            if asked.is_empty() || answered {
                return Vec::new();
            }
            vec![Effect::Fetch(Request::StatsLookup(asked))]
        }
        Action::CloseStatsLookup => {
            state.stats_page.lookup_open = false;
            Vec::new()
        }
        Action::OpenStat { kind, id } => {
            state.stats_page.lookup_open = false;
            state.stats_page.selected = Some(StatSelection {
                kind,
                id: id.clone(),
                detail: Loadable::Loading,
                tracks: Vec::new(),
            });
            vec![Effect::Fetch(Request::StatDetail(kind, id))]
        }
        Action::CloseStat => {
            state.stats_page.selected = None;
            Vec::new()
        }
        Action::WantArtistPhoto(id) => {
            // Each picture is a whole artist page from YouTube, so they
            // are fetched a couple at a time; a card not served now asks
            // again.
            let waiting = state
                .artist_photos
                .values()
                .filter(|photo| photo.is_none())
                .count();
            if waiting >= PHOTOS_AT_ONCE || state.artist_photos.contains_key(&id) {
                return Vec::new();
            }
            state.artist_photos.insert(id.clone(), None);
            vec![Effect::Fetch(Request::ArtistPhoto(id))]
        }
        Action::ForgetSearch(query) => forget_search(state, &query),
        Action::ClearSearches => {
            let tokens: Vec<String> = state
                .search
                .account
                .drain(..)
                .map(|entry| entry.token)
                .filter(|token| !token.is_empty())
                .collect();
            let scope = state.search_scope();
            state.settings.recent_searches_mut(&scope).clear();
            state.refresh_recent_searches();
            let mut effects = vec![Effect::SaveSettings];
            if !tokens.is_empty() {
                let all = true;
                effects.push(Effect::Fetch(Request::ForgetSearches { tokens, all }));
            }
            effects
        }
        Action::ToggleAbout => {
            state.about_expanded = !state.about_expanded;
            Vec::new()
        }
        Action::ShowWhatsNew => {
            state.dialog = Some(Dialog::WhatsNew);
            mark_notes_read(state)
        }
        // `apply` sends nothing else here.
        _ => Vec::new(),
    }
}

/// Opening the notes is reading them: the mark that says there are new
/// ones goes.
pub(super) fn mark_notes_read(state: &mut State) -> Vec<Effect> {
    let latest = crate::changelog::latest();
    if state.settings.release_notes_read == latest {
        return Vec::new();
    }
    state.settings.release_notes_read = latest.to_owned();
    vec![Effect::SaveSettings]
}

/// Removes a search from this computer's list and, when it came from the
/// account, from there too.
fn forget_search(state: &mut State, query: &str) -> Vec<Effect> {
    let token = state
        .search
        .recent
        .iter()
        .find(|row| row.query == query)
        .map(|row| row.token.clone())
        .unwrap_or_default();
    let scope = state.search_scope();
    searches::forget(state.settings.recent_searches_mut(&scope), query);
    state
        .search
        .account
        .retain(|entry| entry.token != token || token.is_empty());
    state.refresh_recent_searches();
    let mut effects = vec![Effect::SaveSettings];
    if !token.is_empty() {
        effects.push(Effect::Fetch(Request::ForgetSearches {
            tokens: vec![token],
            all: false,
        }));
    }
    effects
}

/// A search has been sent: it joins this computer's recent ones. A search
/// is remembered once it is sent, not as it is typed.
pub(super) fn remember_search(state: &mut State, query: &str) -> Vec<Effect> {
    // Not taken to change before there is something to put in it: taking
    // it is what gives an account the list from before they were apart.
    let scope = state.search_scope();
    let worth = query.trim().chars().count() >= 2;
    if !worth || !searches::remember(state.settings.recent_searches_mut(&scope), query) {
        return Vec::new();
    }
    state.refresh_recent_searches();
    vec![Effect::SaveSettings]
}

/// What comes of the answers this module asked for.
pub(super) fn answered(state: &mut State, response: Response) -> Vec<Effect> {
    match response {
        Response::StatsLookup(text, result) => {
            // For text that has since been replaced, or cleared.
            if text == state.stats_page.lookup_text.trim() {
                state.stats_page.lookup = result.ok().map(|found| (text, found));
            }
        }
        Response::StatDetail(kind, id, result) => {
            if let Some(selected) = &mut state.stats_page.selected
                && selected.kind == kind
                && selected.id == id
            {
                selected.arrived(Loadable::from_result(result));
            }
        }
        Response::ArtistPhoto(id, photo) => {
            state.artist_photos.insert(id, Some(photo));
        }
        // Signed out there are none, and none is what is shown.
        Response::SearchHistory(result) => {
            state.search.account = result.unwrap_or_default();
            state.refresh_recent_searches();
        }
        Response::SearchesForgotten { all, result } if result.is_err() => {
            state.toast_error(if all {
                "Couldn't clear your YouTube Music search history."
            } else {
                "Couldn't remove that search from your YouTube Music history."
            });
            // What the account still holds is shown again.
            return vec![Effect::Fetch(Request::SearchHistory)];
        }
        // Forgotten as asked; and `store` sends nothing else here.
        _ => {}
    }
    Vec::new()
}
