//! Under what is playing: the room's queue with a search to add to it and
//! the requests that wait, what it has played, and what has happened.

use std::time::Duration;

use eframe::egui::{self, Align, Frame, Layout, Margin, Rect, Sense, Ui, Vec2, pos2, vec2};
use spotified_client::models::{Artwork, Track};

use super::super::{format, widgets};
use super::parts::{self, Kind};
use crate::actions::Action;
use crate::state::State;
use crate::theme::{self, Icon};
use crate::together::protocol::{Entry, Person, SongRequest};
use crate::together::{Ask, Room, Tab};

const ROW: f32 = 60.0;
const ART: f32 = 40.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let me = state.together.me.as_str();
    ui.add_space(18.0);
    tabs(state, ui, actions, room);
    ui.add_space(10.0);
    match state.together.tab {
        Tab::Queue => {
            if room.may_add(me) {
                search_field(state, ui, actions, room);
                ui.add_space(10.0);
            }
            if state.together.search.active() && room.may_add(me) {
                results(state, ui, actions, room);
            } else {
                requests(state, ui, actions, room);
                upcoming(state, ui, actions, room);
            }
        }
        Tab::History => history(state, ui, actions, room),
        Tab::Activity => activity(state, ui, room),
    }
}

/// Queue, History and Activity, with the offer to take back an edit.
fn tabs(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let me = state.together.me.as_str();
    let waiting = if room.answers(me) {
        room.requests.len()
    } else {
        0
    };
    let row = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 24.0;
            for tab in Tab::EVERY {
                let chosen = state.together.tab == tab;
                let badge = if tab == Tab::Queue { waiting } else { 0 };
                if tab_button(state, ui, &tab.label(room), chosen, badge) {
                    actions.push(Action::Room(Ask::ShowTab(tab)));
                }
            }
            let server_now = state.together.server_now();
            if room.may_undo(me, server_now) {
                // The offer goes when the relay would refuse it.
                let left = room
                    .undo
                    .as_ref()
                    .map_or(0.0, |undo| undo.expires - server_now);
                let left = Duration::from_millis(left.clamp(0.0, 60_000.0) as u64 + 50);
                ui.ctx().request_repaint_after(left);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if parts::text_button(ui, palette, None, "Undo edit").clicked() {
                        actions.push(Action::Room(Ask::Undo));
                    }
                });
            }
        })
        .response
        .rect;
    let across = ui.max_rect().x_range();
    ui.painter()
        .hline(across, row.bottom(), (1.0, palette.outline));
}

/// One tab: its words, a line under the one chosen, and how many requests
/// wait, on the queue's.
fn tab_button(state: &State, ui: &mut Ui, label: &str, chosen: bool, badge: usize) -> bool {
    let palette = &state.palette;
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::medium(13.0), palette.text);
    let badge_room = if badge > 0 { 26.0 } else { 0.0 };
    let size = vec2(galley.size().x + badge_room, 40.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    widgets::hand(ui, &response);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), chosen, label)
    });
    let lift = widgets::hover(ui, &response);
    let ink = if chosen {
        palette.text
    } else {
        crate::tint::blend(palette.secondary, palette.text, lift)
    };
    let at = pos2(rect.left(), rect.center().y - galley.size().y / 2.0 - 2.0);
    let end = at.x + galley.size().x;
    ui.painter().galley(at, galley, ink);
    if badge > 0 {
        let pill =
            Rect::from_center_size(pos2(end + 15.0, rect.center().y - 2.0), vec2(18.0, 18.0));
        ui.painter().rect_filled(pill, 9, palette.accent);
        let font = theme::bold(10.0);
        let align = egui::Align2::CENTER_CENTER;
        let count = badge.to_string();
        widgets::text_at(ui, pill.center(), align, &count, font, palette.on_accent);
    }
    if chosen {
        let line = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 2.0), rect.right_bottom());
        ui.painter().rect_filled(line, 0, palette.accent);
    }
    response.clicked()
}

/// The field that finds a song to add, or to ask for.
fn search_field(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let requesting = room.requesting(&state.together.me);
    let (label, hint) = if requesting {
        (
            "Find songs to request",
            "Find a song to request from the leader",
        )
    } else {
        ("Find songs to add", "Find a song to add to the room")
    };
    let query = &state.together.search.query;
    let id = ui.id().with("room-search");
    let focused = ui.memory(|memory| memory.has_focus(id));
    let outline = if focused {
        palette.accent
    } else {
        palette.outline
    };
    Frame::new()
        .fill(widgets::wash(ui, 0.15))
        .stroke((1.0, outline))
        .corner_radius(9)
        .inner_margin(Margin::symmetric(12, 0))
        .show(ui, |ui| {
            ui.set_height(40.0);
            ui.set_width(ui.available_width());
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.add(Icon::Search.image(palette.secondary, 18.0));
                let mut edited = query.clone();
                let clear_room = if query.is_empty() { 0.0 } else { 34.0 };
                let edit = egui::TextEdit::singleline(&mut edited)
                    .id(id)
                    .hint_text(egui::RichText::new(hint).font(theme::regular(12.5)))
                    .font(theme::regular(12.5))
                    .frame(Frame::NONE)
                    .desired_width(ui.available_width() - clear_room);
                let response = ui.add(edit);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, label)
                });
                if response.changed() {
                    actions.push(Action::Room(Ask::Search(edited)));
                }
                if !query.is_empty() {
                    let clear = widgets::icon_button(ui, palette, Icon::X, 16.0, "Clear search");
                    if clear.clicked() {
                        actions.push(Action::Room(Ask::Search(String::new())));
                    }
                }
            });
        });
}

/// What a row shows of a song.
struct Song<'a> {
    title: &'a str,
    /// The line under the title.
    detail: String,
    artwork: &'a [Artwork],
    /// The row is the song that is playing.
    current: bool,
    /// Its place in the list, where places are shown.
    number: Option<usize>,
    by: Option<&'a Person>,
    duration_ms: Option<u64>,
}

/// A row of a track list: a number, the cover, the title over a line of
/// detail, then whatever `controls` adds from the right edge inwards.
/// Nothing is drawn for a row that is out of sight.
fn song_row(state: &State, ui: &mut Ui, song: &Song<'_>, controls: impl FnOnce(&mut Ui)) {
    let palette = &state.palette;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    if song.current {
        let fill = palette.accent.gamma_multiply(0.07);
        ui.painter().rect_filled(rect, theme::RADIUS, fill);
    }
    widgets::row_hover(ui, &response, rect);
    let mut left = rect.left() + 8.0;
    if let Some(number) = song.number {
        let at = pos2(left + 9.0, rect.center().y);
        let align = egui::Align2::CENTER_CENTER;
        let font = theme::regular(11.0);
        widgets::text_at(ui, at, align, &number.to_string(), font, palette.secondary);
        left += 28.0;
    }
    let cover = Rect::from_min_size(pos2(left, rect.center().y - ART / 2.0), Vec2::splat(ART));
    let shape = widgets::ArtShape::Rounded(5);
    widgets::artwork(ui, state, song.artwork, cover, shape, Icon::Music);
    left = cover.right() + 11.0;
    // The controls take what they need from the right; the words have
    // the rest.
    let inner = Rect::from_min_max(pos2(left, rect.top()), rect.right_bottom() - vec2(8.0, 0.0));
    let mut right = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::right_to_left(Align::Center)),
    );
    right.spacing_mut().item_spacing.x = 6.0;
    controls(&mut right);
    if let Some(duration_ms) = song.duration_ms {
        let length = format::duration(duration_ms);
        right.label(
            egui::RichText::new(length)
                .font(theme::regular(11.0))
                .color(palette.secondary),
        );
    }
    if let Some(person) = song.by {
        let (disc, _) = right.allocate_exact_size(Vec2::splat(parts::AVATAR_SMALL), Sense::hover());
        parts::avatar(state, &right, disc, &person.name, &person.avatar);
    }
    let width = (right.min_rect().left() - left - 10.0).max(20.0);
    let font = theme::medium(12.5);
    let title = widgets::elided(ui, song.title, font, palette.text, width, 1);
    // A song known only by its name has the row's middle to itself.
    let rise = if song.detail.is_empty() {
        title.size().y / 2.0
    } else {
        17.0
    };
    ui.painter()
        .galley(pos2(left, rect.center().y - rise), title, palette.text);
    let font = theme::regular(11.5);
    let detail = widgets::elided(ui, &song.detail, font, palette.secondary, width, 1);
    ui.painter()
        .galley(pos2(left, rect.center().y + 2.0), detail, palette.secondary);
}

/// A line that stands where a list has nothing.
fn empty(state: &State, ui: &mut Ui, icon: Option<Icon>, text: &str) {
    ui.add_space(30.0);
    ui.vertical_centered(|ui| {
        if let Some(icon) = icon {
            ui.add(icon.image(state.palette.dim, 32.0));
            ui.add_space(8.0);
        }
        parts::quiet(state, ui, text);
    });
    ui.add_space(30.0);
}

/// What the search found, each with the ways to bring it into the room.
fn results(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let me = state.together.me.as_str();
    let search = &state.together.search;
    if search.searching {
        return empty(state, ui, None, "Searching…");
    }
    if search.results.is_empty() {
        return empty(state, ui, None, "No songs found. Try another search.");
    }
    let add = if room.requesting(me) {
        "Request"
    } else {
        "Add"
    };
    for track in &search.results {
        let song = Song {
            title: &track.title,
            detail: track.artist_names(),
            artwork: &track.artwork,
            current: false,
            number: None,
            by: None,
            duration_ms: None,
        };
        song_row(state, ui, &song, |ui| {
            let named = format!("{add} {}", track.title);
            let plus = Some(Icon::Plus);
            if parts::button_named(ui, palette, Kind::Secondary, plus, (add, &named)).clicked() {
                actions.push(Action::Room(Ask::Add(Box::new(track.clone()))));
            }
            if room.may_control(me) {
                let named = format!("Play {} radio", track.title);
                let radio = Some(Icon::Radio);
                let label = ("Radio", named.as_str());
                let button = parts::button_named(ui, palette, Kind::Secondary, radio, label);
                if button.on_hover_text("Play now, then its radio").clicked() {
                    actions.push(Action::Room(Ask::Radio(Box::new(track.clone()))));
                }
            }
        });
    }
}

/// Songs waiting for approval. The leader and DJs answer them; a guest
/// sees their own, and may withdraw them.
fn requests(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let me = state.together.me.as_str();
    let answers = room.answers(me);
    let count = room.requests_for(me).count();
    if count == 0 {
        return;
    }
    Frame::new()
        .fill(palette.accent.gamma_multiply(0.06))
        .stroke((1.0, palette.accent.gamma_multiply(0.25)))
        .corner_radius(12)
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let title = if answers { "Requests" } else { "Your requests" };
                ui.label(egui::RichText::new(title).font(theme::semibold(13.0)));
                parts::small(state, ui, &count.to_string());
                if answers && count > 1 {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let all = Some(Icon::CircleCheck);
                        if parts::text_button(ui, palette, all, "Accept all").clicked() {
                            let requests = room.requests.iter().map(|r| r.id.clone()).collect();
                            let next = false;
                            actions.push(Action::Room(Ask::Accept { requests, next }));
                        }
                    });
                }
            });
            for request in room.requests_for(me) {
                request_row(state, ui, actions, request, answers);
            }
        });
    ui.add_space(10.0);
}

fn request_row(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    request: &SongRequest,
    answers: bool,
) {
    let palette = &state.palette;
    let title = request.track.title.as_str();
    let from = if answers {
        format!("from {}", request.by.name)
    } else {
        String::new()
    };
    let song = Song {
        title,
        detail: format::middle_dotted([request.track.artist_names().as_str(), from.as_str()]),
        artwork: &request.track.artwork,
        current: false,
        number: None,
        by: answers.then_some(&request.by),
        duration_ms: None,
    };
    let one = || vec![request.id.clone()];
    song_row(state, ui, &song, |ui| {
        if !answers {
            let named = format!("Cancel your request for {title}");
            if parts::text_button_named(ui, palette, ("Cancel", &named)).clicked() {
                actions.push(Action::Room(Ask::Withdraw(request.id.clone())));
            }
            ui.label(
                egui::RichText::new("Pending")
                    .font(theme::regular(11.0))
                    .color(palette.accent),
            );
            return;
        }
        let named = format!("Decline {title} from {}", request.by.name);
        if widgets::icon_button(ui, palette, Icon::X, 16.0, &named).clicked() {
            actions.push(Action::Room(Ask::Decline(one())));
        }
        let named = format!("Add {title} to queue");
        if parts::text_button_named(ui, palette, ("Add to queue", &named)).clicked() {
            let requests = one();
            let next = false;
            actions.push(Action::Room(Ask::Accept { requests, next }));
        }
        let named = format!("Play {title} next");
        if parts::text_button_named(ui, palette, ("Play next", &named)).clicked() {
            let requests = one();
            let next = true;
            actions.push(Action::Room(Ask::Accept { requests, next }));
        }
    });
}

fn entry_song<'a>(room: &Room, entry: &'a Entry, index: usize, history: bool) -> Song<'a> {
    Song {
        title: &entry.track.title,
        detail: entry.track.artist_names(),
        artwork: &entry.track.artwork,
        current: !history && room.current.as_deref() == Some(entry.id.as_str()),
        number: Some(index + 1),
        by: Some(&entry.added_by),
        duration_ms: Some(entry.track.duration_ms.max(0.0) as u64),
    }
}

/// The queue from the song that is playing on.
fn upcoming(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let me = state.together.me.as_str();
    if room.queue.is_empty() {
        return empty(
            state,
            ui,
            Some(Icon::ListMusic),
            "Good music starts with one song.",
        );
    }
    let steers = room.may_control(me);
    for (index, entry) in room.upcoming() {
        let song = entry_song(room, entry, index, false);
        let title = entry.track.title.as_str();
        song_row(state, ui, &song, |ui| {
            if room.may_remove(me, entry) {
                let named = format!("Remove {title}");
                if widgets::icon_button(ui, palette, Icon::X, 16.0, &named).clicked() {
                    actions.push(Action::Room(Ask::Remove(entry.id.clone())));
                }
            }
            if steers {
                let named = format!("Play {title}");
                if widgets::icon_button(ui, palette, Icon::PlayFilled, 14.0, &named).clicked() {
                    actions.push(Action::Room(Ask::Jump(entry.id.clone())));
                }
            }
        });
    }
}

/// What the room has played, with a way to keep it as a playlist.
fn history(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let me = state.together.me.as_str();
    ui.add_enabled_ui(!room.history.is_empty(), |ui| {
        if parts::button(ui, palette, Kind::Secondary, None, "Save as playlist").clicked() {
            actions.push(Action::Room(Ask::SaveHistory));
        }
    });
    ui.add_space(6.0);
    if room.history.is_empty() {
        return empty(
            state,
            ui,
            None,
            "Songs played in this room will appear here.",
        );
    }
    let may_add = room.may_add(me);
    let requesting = room.requesting(me);
    for (index, entry) in room.history.iter().enumerate() {
        let song = entry_song(room, entry, index, true);
        song_row(state, ui, &song, |ui| {
            if !may_add {
                return;
            }
            let title = &entry.track.title;
            let named = if requesting {
                format!("Request {title}")
            } else {
                format!("Add {title} to queue")
            };
            if widgets::icon_button(ui, palette, Icon::Plus, 16.0, &named).clicked() {
                let track: Track = entry.track.to_track();
                actions.push(Action::Room(Ask::Add(Box::new(track))));
            }
        });
    }
}

/// What has happened in the room, newest first, each with its time.
fn activity(state: &State, ui: &mut Ui, room: &Room) {
    let palette = &state.palette;
    let zone = state.together.zone_minutes;
    for happening in room.activity.iter().rev() {
        let size = vec2(ui.available_width(), 44.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        if !ui.is_rect_visible(rect) {
            continue;
        }
        parts::read_out(&response, &happening.text);
        let time = format::clock(happening.at, zone);
        let align = egui::Align2::RIGHT_CENTER;
        let font = theme::regular(10.5);
        let timed = widgets::text_at(
            ui,
            rect.right_center(),
            align,
            &time,
            font,
            palette.secondary,
        );
        let width = timed.left() - rect.left() - 20.0;
        let font = theme::regular(12.5);
        let said = widgets::elided(ui, &happening.text, font, palette.secondary, width, 1);
        let at = pos2(rect.left(), rect.center().y - said.size().y / 2.0);
        ui.painter().galley(at, said, palette.secondary);
        ui.painter()
            .hline(rect.x_range(), rect.bottom(), (1.0, palette.outline));
    }
}
