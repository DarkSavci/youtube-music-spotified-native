//! The room you are in, as an album's page is laid out: the song playing
//! and the room's name over the cover's colour, the room's buttons under
//! them, then what waits beside who is there. The leader's settings, the
//! way out and a shared start open as cards between.

use std::time::Duration;

use eframe::egui::{self, Align, Layout, Rect, Sense, Ui, Vec2, pos2, vec2};

use super::super::collection::hero::{self, Hero, Part};
use super::super::widgets::menu::{self, Entry};
use super::super::widgets::{self, ArtShape, TextField};
use super::parts::{self, Kind};
use super::{GAP, listeners, queue};
use crate::actions::Action;
use crate::state::State;
use crate::theme::{self, Icon};
use crate::together::protocol::{Mode, Policy};
use crate::together::{Ask, Phase, Room, Setting, Transport};

/// The listeners' column, where it stands beside the room.
const LISTENERS_WIDTH: f32 = 320.0;
const PLAY_BUTTON: f32 = 56.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let me = state.together.me.as_str();
    let leads = room.leads(me);
    head(state, ui, actions, room);
    buttons(state, ui, actions, room);
    if let Some(error) = &state.together.error {
        super::alert(state, ui, actions, error);
        ui.add_space(GAP);
    }
    if let Some(next) = &state.together.leaving {
        leave(state, ui, actions, room, next);
        ui.add_space(GAP);
    }
    if state.together.settings_open && leads {
        settings(state, ui, actions, room);
        ui.add_space(GAP);
    }
    if room.countdown.is_some() {
        ready_check(state, ui, actions, room);
        ui.add_space(GAP);
    }
    let width = ui.available_width();
    if width < super::TWO_COLUMNS_FROM {
        queue::show(state, ui, actions, room);
        ui.add_space(GAP);
        listeners::show(state, ui, actions, room);
        return;
    }
    let share = (width - GAP - LISTENERS_WIDTH) / (width - GAP);
    super::columns(
        ui,
        share,
        actions,
        |ui, actions| queue::show(state, ui, actions, room),
        |ui, actions| listeners::show(state, ui, actions, room),
    );
}

/// The PIN as it is read out: two groups of four.
fn spaced(pin: &str) -> String {
    match pin.len() {
        8 => format!("{} {}", &pin[..4], &pin[4..]),
        _ => pin.to_owned(),
    }
}

/// The song playing, large, over its cover's colour, under the room's
/// name: what an album's page says of the album, the room says of itself.
fn head(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let me = state.together.me.as_str();
    let current = room.current().map(|(_, entry)| entry);
    let art = current.map_or(&[][..], |entry| &entry.track.artwork[..]);
    let title = room.title();
    let mut byline = Vec::new();
    match current {
        Some(entry) => {
            byline.push(Part::plain("Playing"));
            byline.push(Part::strong(entry.track.title.clone(), None).joined());
            byline.push(Part::plain(entry.track.artist_names()));
        }
        None if !room.may_add(me) => byline.push(Part::plain("The leader picks the first song")),
        None => byline.push(Part::plain("Nothing playing yet")),
    }
    let listening = room
        .members
        .iter()
        .filter(|member| member.connected)
        .count();
    byline.push(Part::plain(match listening {
        1 => "1 listener".to_owned(),
        count => format!("{count} listeners"),
    }));
    byline.push(Part::plain(room.mode.title()));
    // Whether this player is with the room, or on its way back to it.
    byline.push(Part::plain(
        if state.together.phase == Phase::Reconnecting {
            "Reconnecting…"
        } else {
            state.together.standing.label()
        },
    ));
    let tint = widgets::artwork_tint(ui, state, art, hero::COVER);
    let spec = Hero {
        art,
        shape: ArtShape::Rounded(4),
        placeholder: Icon::Headphones,
        kind: "Listen Together",
        title: &title,
        avatar: &[],
        byline,
        tint,
    };
    hero::show(state, ui, actions, spec);
    // The name is painted, not laid out: it is told to screen readers.
    let (_, named) = ui.allocate_exact_size(Vec2::ZERO, Sense::hover());
    parts::read_out(&named, &title);
}

/// Under the room's head: play or pause, the songs either side and the
/// vote to skip, for those who steer it; at the right, the PIN that
/// invites, the way out, and for the leader the room's settings.
fn buttons(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let me = state.together.me.as_str();
    let steers = room.may_control(me);
    let ask = |actions: &mut Vec<Action>, transport| {
        actions.push(Action::Room(Ask::Transport(transport)));
    };
    let size = vec2(ui.available_width(), hero::ACTIONS_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    row.spacing_mut().item_spacing.x = 12.0;
    row.add_enabled_ui(steers && room.current.is_some(), |ui| {
        play_button(state, ui, room, || ask(actions, Transport::Toggle));
    });
    row.add_enabled_ui(steers, |ui| {
        let back = (Icon::SkipBackFilled, 36.0, 18.0);
        if hero::round_button(state, ui, back, ("Previous song", "Previous")).clicked() {
            ask(actions, Transport::Previous);
        }
        let next = (Icon::SkipForwardFilled, 36.0, 18.0);
        if hero::round_button(state, ui, next, ("Next song", "Next")).clicked() {
            ask(actions, Transport::Next);
        }
    });
    if room.vote_skip {
        let label = format!("Vote to skip · {}", room.votes.len());
        if parts::text_button(&mut row, &state.palette, None, &label).clicked() {
            actions.push(Action::Room(Ask::VoteSkip));
        }
    }
    if let Some(person) = &room.last_controlled_by {
        parts::small(
            state,
            &mut row,
            &format!("Last controlled by {}", person.name),
        );
    }
    row.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        if room.leads(me) {
            let gear = (Icon::Settings, 36.0, 20.0);
            let settings = hero::round_button(state, ui, gear, ("Room settings", "Room settings"));
            if settings.clicked() {
                actions.push(Action::Room(Ask::ToggleSettings));
            }
        }
        if widgets::outline_button(ui, &state.palette, "Leave room").clicked() {
            actions.push(Action::Room(Ask::OpenLeave));
        }
        pin_share(state, ui, actions, room);
    });
}

/// The big round button that plays or pauses the room for everyone.
fn play_button(state: &State, ui: &mut Ui, room: &Room, toggled: impl FnOnce()) {
    let palette = &state.palette;
    let (icon, label) = if room.playing {
        (Icon::PauseFilled, "Pause room")
    } else {
        (Icon::PlayFilled, "Play room")
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(PLAY_BUTTON), Sense::click());
    widgets::name(ui, &response, label);
    let lift = widgets::hover(ui, &response);
    let fill = if ui.is_enabled() {
        crate::tint::blend(palette.accent, palette.accent_hover, lift)
    } else {
        palette.surface_active
    };
    // It swells a little under the pointer and gives when pressed, as an
    // album's does.
    let pressed = if response.is_pointer_button_down_on() {
        0.96
    } else {
        1.0 + 0.06 * lift
    };
    let radius = PLAY_BUTTON / 2.0 * pressed;
    let centre = rect.center();
    ui.painter().circle_filled(
        centre + vec2(0.0, 4.0),
        radius,
        palette.shadow.gamma_multiply(0.5),
    );
    ui.painter().circle_filled(centre, radius, fill);
    widgets::paint_icon(ui, icon, rect, 22.0, palette.on_accent);
    if response.on_hover_text(label).clicked() {
        toggled();
    }
}

/// The PIN, as a pill that copies it.
fn pin_share(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let shown = spaced(&room.pin);
    let figures = widgets::tracked(
        ui,
        &shown,
        theme::semibold(15.0),
        palette.text,
        1.5,
        (f32::INFINITY, 1),
    );
    let caption =
        ui.painter()
            .layout_no_wrap("PIN".to_owned(), theme::semibold(11.0), palette.secondary);
    let width = 16.0 + caption.size().x + 8.0 + figures.size().x + 8.0 + 16.0 + 14.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 34.0), Sense::click());
    widgets::name(ui, &response, "Copy room PIN");
    if ui.is_rect_visible(rect) {
        let lift = widgets::hover(ui, &response);
        let round = egui::CornerRadius::same(u8::MAX);
        ui.painter()
            .rect_filled(rect, round, widgets::wash(ui, 0.4 + lift * 0.6));
        let middle = rect.center().y;
        let mut x = rect.left() + 16.0;
        let at = pos2(x, middle - caption.size().y / 2.0);
        x += caption.size().x + 8.0;
        ui.painter().galley(at, caption, palette.secondary);
        let at = pos2(x, middle - figures.size().y / 2.0);
        x += figures.size().x + 8.0;
        ui.painter().galley(at, figures, palette.text);
        let copy = Rect::from_center_size(pos2(x + 8.0, middle), Vec2::splat(16.0));
        let ink = crate::tint::blend(palette.secondary, palette.text, lift);
        widgets::paint_icon(ui, Icon::Copy, copy, 16.0, ink);
    }
    if response.on_hover_text("Copy room PIN").clicked() {
        actions.push(Action::CopyText {
            text: room.pin.clone(),
            said: "PIN copied. Friends need the same server.",
        });
    }
}

/// How to leave: for the leader, who leads next, or the end of the room.
fn leave(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room, next: &str) {
    let palette = &state.palette;
    let me = state.together.me.as_str();
    let leads = room.leads(me);
    parts::panel(state, ui, |ui| {
        let (title, about) = if leads {
            (
                "Keep the music going.",
                "Choose the next leader, or let us pick a connected listener. The room and \
                 queue stay together, and the music keeps playing here from the room’s \
                 queue.",
            )
        } else {
            (
                "Leave this room?",
                "The music keeps playing from the room’s queue.",
            )
        };
        parts::heading(ui, title, 18.0);
        parts::quiet(state, ui, about);
        if leads {
            ui.add_space(10.0);
            parts::caption(state, ui, "Next leader");
            const AT_RANDOM: &str = "Choose someone at random";
            let others = || {
                let connected = room.members.iter().filter(|member| member.connected);
                connected.filter(|member| member.id != me)
            };
            let chosen = others().find(|member| member.id == next);
            let shown = chosen.map_or(AT_RANDOM, |member| &member.name);
            let select = parts::select(ui, palette, "Next leader", shown, true);
            menu::popup(&select, palette, |menu| {
                if menu.entry(Entry::plain(AT_RANDOM).checked(chosen.is_none())) {
                    actions.push(Action::Room(Ask::NextLeader(String::new())));
                }
                for member in others() {
                    let entry = Entry::plain(&member.name).checked(member.id == next);
                    if menu.entry(entry) {
                        actions.push(Action::Room(Ask::NextLeader(member.id.clone())));
                    }
                }
            });
        }
        ui.add_space(12.0);
        parts::actions_row(ui, |ui| {
            // The button above says the same: this one is told from it.
            let leave = ("Leave room", "Leave the room now");
            if parts::button_named(ui, palette, Kind::Primary, None, leave).clicked() {
                actions.push(Action::Room(Ask::Leave));
            }
            if parts::button(ui, palette, Kind::Secondary, None, "Stay").clicked() {
                actions.push(Action::Room(Ask::Stay));
            }
            let end = "End room for everyone";
            if leads && parts::button(ui, palette, Kind::Danger, None, end).clicked() {
                actions.push(Action::Room(Ask::EndRoom));
            }
        });
    });
}

/// The leader's controls over who steers, who joins, and the queue.
fn settings(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let set = |actions: &mut Vec<Action>, setting| actions.push(Action::Room(Ask::Set(setting)));
    parts::panel(state, ui, |ui| {
        parts::heading(ui, "Room settings", 18.0);
        ui.add_space(8.0);
        ui.columns(3, |columns| {
            let ui = &mut columns[0];
            parts::caption(state, ui, "Who controls playback?");
            let label = "Who controls playback?";
            let select = parts::select(ui, palette, label, room.mode.title(), true);
            menu::popup(&select, palette, |menu| {
                for mode in Mode::EVERY {
                    let entry = Entry::plain(mode.title()).checked(mode == room.mode);
                    if menu.entry(entry) {
                        set(actions, Setting::Mode(mode));
                    }
                }
            });
            let ui = &mut columns[1];
            parts::caption(state, ui, "Queue order");
            let select = parts::select(ui, palette, "Queue order", room.policy.title(), true);
            menu::popup(&select, palette, |menu| {
                for policy in Policy::EVERY {
                    let entry = Entry::plain(policy.title()).checked(policy == room.policy);
                    if menu.entry(entry) {
                        set(actions, Setting::Policy(policy));
                    }
                }
            });
            let ui = &mut columns[2];
            parts::caption(state, ui, "Songs per guest");
            limit_field(state, ui, actions, room);
        });
        ui.add_space(12.0);
        parts::actions_row(ui, |ui| {
            let mut tick = |ui: &mut Ui, on: bool, label: &str, setting: fn(bool) -> Setting| {
                if parts::check(ui, palette, on, label) {
                    set(actions, setting(!on));
                }
            };
            tick(
                ui,
                room.join_approval,
                "Approve new listeners",
                Setting::JoinApproval,
            );
            tick(ui, room.locked, "Lock new joins", Setting::Locked);
            if room.mode == Mode::Contributions {
                let label = "Add requests without asking me";
                tick(ui, room.auto_accept, label, Setting::AutoAccept);
            }
            tick(ui, room.duplicates, "Allow duplicates", Setting::Duplicates);
            tick(ui, room.vote_skip, "Vote to skip", Setting::VoteSkip);
        });
        ui.add_space(8.0);
        parts::actions_row(ui, |ui| {
            if parts::button(ui, palette, Kind::Secondary, None, "Rotate PIN").clicked() {
                actions.push(Action::Room(Ask::RotatePin));
            }
            parts::small(state, ui, "Everyone stays. The old PIN stops working.");
            let label = "Start together / ready check";
            if parts::button(ui, palette, Kind::Secondary, None, label).clicked() {
                actions.push(Action::Room(Ask::ReadyCheck));
            }
        });
    });
}

/// "Songs per guest": typed, and sent when the caret leaves it.
fn limit_field(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let held = room.limit.to_string();
    let text = state.together.limit_text.as_deref().unwrap_or(&held);
    let field = TextField {
        text,
        hint: "50",
        label: "Songs per guest",
        icon: None,
        width: ui.available_width(),
        compact: false,
    };
    let id = ui.id().with(("text-field", field.label));
    let had_caret = ui.memory(|memory| memory.has_focus(id));
    if let Some(typed) = field.show(ui, &state.palette) {
        actions.push(Action::Room(Ask::LimitText(typed)));
    }
    let has_caret = ui.memory(|memory| memory.has_focus(id));
    if had_caret && !has_caret && state.together.limit_text.is_some() {
        actions.push(Action::Room(Ask::CommitLimit));
    }
}

/// A shared start: who is ready, and the count once everyone is.
fn ready_check(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let Some(countdown) = &room.countdown else {
        return;
    };
    let said = match countdown.start_at {
        Some(start_at) => {
            // The count is the relay's clock running down.
            ui.ctx().request_repaint_after(Duration::from_millis(250));
            let seconds = ((start_at - state.together.server_now()) / 1000.0).ceil();
            if seconds > 0.0 {
                format!("Starting in {seconds}…")
            } else {
                "Starting…".to_owned()
            }
        }
        None => "Ready for a shared start?".to_owned(),
    };
    let (ready, connected) = room.ready();
    parts::panel(state, ui, |ui| {
        parts::actions_row(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            ui.label(egui::RichText::new(said).font(theme::bold(14.0)));
            parts::quiet(state, ui, &format!("{ready} of {connected} ready"));
            if parts::button(ui, palette, Kind::Primary, None, "I’m ready").clicked() {
                actions.push(Action::Room(Ask::Ready));
            }
            let soon = "Start in 3 seconds";
            let leads = room.leads(&state.together.me);
            if leads && parts::button(ui, palette, Kind::Secondary, None, soon).clicked() {
                actions.push(Action::Room(Ask::StartSoon));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pin_is_shown_in_two_groups() {
        assert_eq!(spaced("01234567"), "0123 4567");
        assert_eq!(spaced("123"), "123");
    }
}
