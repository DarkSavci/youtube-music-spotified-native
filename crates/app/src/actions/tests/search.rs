//! Searching: what is sent, and which answers are kept.

use super::*;

#[test]
fn typing_opens_search_and_waits_for_a_pause() {
    let mut state = ready();
    let effects = apply(&mut state, Action::SetSearchQuery("bonobo".into()));
    assert_eq!(state.nav.page(), &Page::Search);
    assert_eq!(effects, [Effect::DebounceSearch]);
    assert_eq!(state.search.serial, 0);
}

#[test]
fn clearing_the_query_stays_on_the_page_and_drops_the_results() {
    let mut state = ready();
    apply(&mut state, Action::SetSearchQuery("bonobo".into()));
    apply(&mut state, Action::RunSearch);
    apply(&mut state, Action::SetSearchQuery(String::new()));
    assert_eq!(state.nav.page(), &Page::Search);
    assert_eq!(state.search.results, Loadable::NotLoaded);
}

#[test]
fn an_answer_to_an_older_query_is_dropped() {
    let mut state = ready();
    apply(&mut state, Action::SetSearchQuery("bon".into()));
    apply(&mut state, Action::RunSearch);
    apply(&mut state, Action::SetSearchQuery("bonobo".into()));
    apply(&mut state, Action::RunSearch);
    let late = Response::Search {
        serial: 1,
        result: Ok(results("bon")),
    };
    apply(&mut state, Action::Loaded(Box::new(late)));
    assert_eq!(state.search.results, Loadable::Loading);

    let current = Response::Search {
        serial: 2,
        result: Ok(results("bonobo")),
    };
    apply(&mut state, Action::Loaded(Box::new(current)));
    assert_eq!(state.search.results, Loadable::Loaded(results("bonobo")));
}

#[test]
fn a_query_typed_before_the_core_is_ready_is_sent_once_it_is() {
    let mut state = state();
    apply(&mut state, Action::SetSearchQuery("bonobo".into()));
    assert!(apply(&mut state, Action::RunSearch).is_empty());
    let effects = apply(
        &mut state,
        Action::CoreChanged(CoreStatus::Ready {
            origin: "http://127.0.0.1:1".into(),
        }),
    );
    let search = Effect::Fetch(Request::Search {
        serial: 1,
        query: "bonobo".into(),
        filter: SearchFilter::All,
    });
    assert!(effects.contains(&search));
}

#[test]
fn a_filter_chip_searches_again_narrowed() {
    let mut state = ready();
    apply(&mut state, Action::SetSearchQuery("bonobo".into()));
    apply(&mut state, Action::RunSearch);
    let effects = apply(&mut state, Action::SetSearchFilter(SearchFilter::Albums));
    assert_eq!(
        effects,
        [Effect::Fetch(Request::Search {
            serial: 2,
            query: "bonobo".into(),
            filter: SearchFilter::Albums,
        })]
    );
}

#[test]
fn the_empty_search_page_asks_for_the_moods_and_the_recent_searches() {
    let mut state = ready();
    let effects = apply(&mut state, Action::Open(Page::Search));
    let moods = Request::Browse("FEmusic_moods_and_genres".into(), String::new());
    assert_eq!(
        effects,
        [Effect::Fetch(moods), Effect::Fetch(Request::SearchHistory)]
    );
}

#[test]
fn a_search_also_asks_how_the_query_might_go_on() {
    let mut state = ready();
    apply(&mut state, Action::SetSearchQuery("bon".into()));
    let effects = apply(&mut state, Action::RunSearch);
    assert!(effects.contains(&Effect::Fetch(Request::Suggest("bon".into()))));

    // Narrowing the same query does not ask again.
    let effects = apply(&mut state, Action::SetSearchFilter(SearchFilter::Albums));
    assert_eq!(effects.len(), 1);
}

#[test]
fn suggestions_for_a_query_no_longer_on_screen_are_dropped() {
    let mut state = ready();
    apply(&mut state, Action::SetSearchQuery("bonobo".into()));
    let late = Response::Suggestions("bon".into(), Ok(vec!["bon iver".into()]));
    apply(&mut state, Action::Loaded(Box::new(late)));
    assert!(state.search.suggestions.is_empty());

    let current = Response::Suggestions("bonobo".into(), Ok(vec!["bonobo kerala".into()]));
    apply(&mut state, Action::Loaded(Box::new(current)));
    assert_eq!(state.search.suggestions, ["bonobo kerala"]);
}

#[test]
fn choosing_a_suggestion_searches_for_it_at_once() {
    let mut state = ready();
    let effects = apply(&mut state, Action::Search("bonobo kerala".into()));
    assert_eq!(state.search.query, "bonobo kerala");
    assert_eq!(state.nav.page(), &Page::Search);
    assert!(matches!(
        &effects[0],
        Effect::Fetch(Request::Search { query, .. }) if query == "bonobo kerala"
    ));
}

#[test]
fn browse_all_empties_the_search_and_shows_the_tiles() {
    let mut state = ready();
    apply(&mut state, Action::SetSearchQuery("bonobo".into()));
    let effects = apply(&mut state, Action::BrowseAll);
    assert!(state.search.query.is_empty());
    assert_eq!(state.nav.page(), &Page::Search);
    let moods = Request::Browse("FEmusic_moods_and_genres".into(), String::new());
    assert!(effects.contains(&Effect::Fetch(moods)));
}

#[test]
fn tile_pictures_are_fetched_two_at_a_time_and_each_only_once() {
    let mut state = ready();
    let want =
        |state: &mut State, id: &str| apply(state, Action::WantTileArt(id.into(), String::new()));
    assert_eq!(
        want(&mut state, "a"),
        [Effect::Fetch(Request::TileArt("a".into(), String::new()))]
    );
    assert!(want(&mut state, "a").is_empty());
    assert_eq!(want(&mut state, "b").len(), 1);
    // Two are on their way: the third waits its turn.
    assert!(want(&mut state, "c").is_empty());
    let answer = Response::TileArt("a".into(), String::new(), Vec::new());
    apply(&mut state, Action::Loaded(Box::new(answer)));
    assert_eq!(want(&mut state, "c").len(), 1);
    // One that came back with no picture is not asked for again.
    assert!(want(&mut state, "a").is_empty());
}

/// The recent searches as the page lists them.
fn recent(state: &State) -> Vec<&str> {
    let rows = state.search.recent.iter();
    rows.map(|row| row.query.as_str()).collect()
}

/// A search for `query`, sent.
fn searched(state: &mut State, query: &str) {
    apply(state, Action::SetSearchQuery(query.into()));
    apply(state, Action::RunSearch);
}

#[test]
fn what_one_account_searched_for_here_is_not_shown_to_the_next() {
    let mut state = ready();
    let ada = crate::accounts::new_account("Ada");
    let ada_id = ada.id.clone();
    state.accounts.add(ada);
    searched(&mut state, "bonobo");
    assert_eq!(recent(&state), ["bonobo"]);

    state.accounts.add(crate::accounts::new_account("Grace"));
    state.refresh_recent_searches();
    assert!(recent(&state).is_empty());
    searched(&mut state, "air");
    assert_eq!(recent(&state), ["air"]);

    // Back to the first, and to what the first looked for.
    state.accounts.activate(&ada_id);
    state.refresh_recent_searches();
    assert_eq!(recent(&state), ["bonobo"]);
}

#[test]
fn a_channel_of_an_account_has_searches_of_its_own() {
    let mut state = ready();
    state.accounts.add(crate::accounts::new_account("Ada"));
    searched(&mut state, "bonobo");
    state.accounts.select_channel("123");
    state.refresh_recent_searches();
    assert!(recent(&state).is_empty());
    assert_eq!(state.search_scope().rsplit(':').next(), Some("123"));
}

#[test]
fn the_searches_from_before_they_were_kept_apart_go_to_whoever_searches_first() {
    let mut state = ready();
    state.settings.recent_searches = vec!["moby".into()];
    state.accounts.add(crate::accounts::new_account("Ada"));
    state.refresh_recent_searches();
    // Shown to whoever is here, and theirs once they search.
    assert_eq!(recent(&state), ["moby"]);
    searched(&mut state, "air");
    assert_eq!(recent(&state), ["air", "moby"]);
    assert!(state.settings.recent_searches.is_empty());

    state.accounts.add(crate::accounts::new_account("Grace"));
    state.refresh_recent_searches();
    assert!(recent(&state).is_empty());
}

#[test]
fn clearing_the_recent_searches_clears_only_this_accounts() {
    let mut state = ready();
    let ada = crate::accounts::new_account("Ada");
    let ada_id = ada.id.clone();
    state.accounts.add(ada);
    searched(&mut state, "bonobo");
    state.accounts.add(crate::accounts::new_account("Grace"));
    searched(&mut state, "air");
    apply(&mut state, Action::ClearSearches);
    assert!(recent(&state).is_empty());
    state.accounts.activate(&ada_id);
    state.refresh_recent_searches();
    assert_eq!(recent(&state), ["bonobo"]);
}
