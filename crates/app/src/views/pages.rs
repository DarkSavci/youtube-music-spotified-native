//! What the page area shows for each [`Page`].

use eframe::egui::{self, Sense, Ui, vec2};

use super::{
    browse, cards, changelog, collection, search, settings, skeleton, stats, together, widgets,
};
use crate::actions::Action;
use crate::sidecar::CoreStatus;
use crate::state::{Loadable, Page, State};
use crate::theme::{self, Icon};

/// The room above a page that has no hero to start it.
const PAGE_TOP: f32 = 16.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let page = state.nav.page();
    // A hero reaches the top of the page; everything else starts below it.
    let hero = matches!(
        page,
        Page::Album(_) | Page::Playlist(_) | Page::Artist(_) | Page::Podcast(_) | Page::Mix(_)
    );
    let shelves = matches!(
        page,
        Page::Home
            | Page::Search
            | Page::Browse(_)
            | Page::History
            | Page::Stats
            | Page::ArtistSongs(_)
    );
    if !hero {
        ui.add_space(PAGE_TOP);
    }
    // The pages made of shelves and tables set every gap themselves.
    if hero || shelves {
        ui.spacing_mut().item_spacing.y = 0.0;
    }
    match page {
        Page::Home => home(state, ui, actions),
        Page::Search => search::show(state, ui, actions),
        Page::Settings => settings::show(state, ui, actions),
        Page::Stats => stats::show(state, ui, actions),
        Page::Browse(surface) => browse::surface(state, ui, actions, surface),
        Page::History => browse::history(state, ui, actions),
        Page::Changelog => changelog::show(state, ui),
        Page::ArtistSongs(id) => collection::songs::show(state, ui, actions, id),
        Page::Together => together::show(state, ui, actions),
        Page::Mix(id) => collection::mix(state, ui, actions, id),
        Page::Album(id) => collection::album(state, ui, actions, state.albums.get(id)),
        Page::Playlist(id) => collection::playlist(state, ui, actions, state.playlists.get(id)),
        Page::Artist(id) => collection::artist(state, ui, actions, state.artists.get(id)),
        Page::Podcast(id) => collection::podcast(state, ui, actions, state.podcasts.get(id)),
    }
}

pub fn title(ui: &mut Ui, text: &str, size: f32) {
    ui.label(egui::RichText::new(text).font(theme::bold(size)));
    ui.add_space(12.0);
}

/// What stands in for a page while it loads.
#[derive(Clone, Copy)]
pub enum Skeleton {
    /// This many shelves of cards.
    Shelves(usize),
    /// This many rows of a track list.
    Tracks(usize),
}

/// Draws a page's loaded data, or says that it is loading or why it failed.
pub fn loaded<T>(state: &State, ui: &mut Ui, page: &Loadable<T>, draw: impl FnOnce(&mut Ui, &T)) {
    match page {
        Loadable::NotLoaded | Loadable::Loading => {
            ui.add_space(PAGE_TOP);
            widgets::loading(ui, &state.palette, "Loading…");
        }
        Loadable::Failed(message) => {
            ui.add_space(PAGE_TOP);
            widgets::error(ui, &state.palette, message);
        }
        Loadable::Loaded(data) => draw(ui, data),
    }
}

/// As [`loaded`], with the shape of the page in grey while it loads, so
/// that what arrives moves nothing.
pub fn loaded_or<T>(
    state: &State,
    ui: &mut Ui,
    page: &Loadable<T>,
    skeleton: Skeleton,
    draw: impl FnOnce(&mut Ui, &T),
) {
    match page {
        Loadable::NotLoaded | Loadable::Loading => match skeleton {
            Skeleton::Shelves(count) => skeleton::shelves(state, ui, count),
            Skeleton::Tracks(rows) => skeleton::tracks(state, ui, rows),
        },
        Loadable::Failed(message) => {
            ui.add_space(PAGE_TOP);
            widgets::error(ui, &state.palette, message);
        }
        Loadable::Loaded(data) => draw(ui, data),
    }
}

fn home(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    if let CoreStatus::Failed(message) = &state.core {
        widgets::error(ui, &state.palette, message);
        return;
    }
    mood_chips(state, ui, actions);
    // What the app has made from the listening done here comes first, on
    // plain Home: a mood's page is YouTube's alone.
    if !state.mixes.is_empty() && state.home_mood.is_empty() {
        cards::noted_title(state, ui, "Made for you", "FROM YOUR LISTENING");
        cards::row(ui, state.mixes.len(), |ui, index| {
            cards::mix(state, ui, actions, &state.mixes[index]);
        });
    }
    loaded_or(state, ui, &state.home, Skeleton::Shelves(3), |ui, page| {
        let more = &state.home_more;
        let nothing = |shelves: &[spotified_client::models::Shelf]| {
            shelves.iter().all(|shelf| shelf.items.is_empty())
        };
        if nothing(&page.shelves)
            && page.moods.is_empty()
            && nothing(&more.shelves)
            && !more.tail.more()
        {
            let text = "Listen to a few things and recommendations will appear here.";
            let palette = &state.palette;
            widgets::empty_state(ui, palette, Icon::Music, "Nothing to show yet", text);
            return;
        }
        browse::shelves(state, ui, actions, &page.shelves, "home");
        browse::shelves(state, ui, actions, &more.shelves, "more");
        more_of_home(state, ui, actions);
    });
}

/// The end of Home: the next few shelves on their way, why they did not
/// come, or, out of sight until scrolled to, the place that asks for them.
/// Like YouTube's, the page arrives a few shelves at a time, and the next
/// are asked for only when the end is in view, never ahead of it: every
/// page is a request to YouTube.
fn more_of_home(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let tail = &state.home_more.tail;
    if let Some(why) = &tail.failed {
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            ui.label(
                egui::RichText::new(why)
                    .font(theme::regular(14.0))
                    .color(state.palette.secondary),
            );
            if widgets::chip(ui, &state.palette, "Try again", false).clicked() {
                actions.push(Action::MoreHome { retry: true });
            }
        });
    } else if tail.loading {
        skeleton::shelf(state, ui);
    } else if tail.more() {
        let size = vec2(ui.available_width(), 1.0);
        let (end, _) = ui.allocate_exact_size(size, Sense::hover());
        if ui.is_rect_visible(end) {
            actions.push(Action::MoreHome { retry: false });
        }
    }
}

/// Home's row of moods: YouTube Music's own. Each reads Home again through
/// it; the chosen one, clicked again, goes back to plain Home.
fn mood_chips(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let chips = &state.home_chips;
    if chips.is_empty() {
        return;
    }
    let mood = &state.home_mood;
    // The chip that carries the chosen params is the chosen one. A row
    // that does not carry them (YouTube writes them anew) says which
    // itself.
    let known = chips.iter().any(|chip| &chip.params == mood);
    egui::ScrollArea::horizontal()
        .id_salt("moods")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for chip in chips {
                    let pressed = !mood.is_empty()
                        && if known {
                            &chip.params == mood
                        } else {
                            chip.selected
                        };
                    if widgets::chip_large(ui, &state.palette, &chip.title, pressed).clicked() {
                        // Whatever the chosen chip's params, choosing it
                        // again is choosing none.
                        let params = if pressed { mood } else { &chip.params };
                        actions.push(Action::ChooseMood(params.clone()));
                    }
                }
            });
        });
    ui.add_space(8.0);
}
