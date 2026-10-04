//! The room you are in: its name and PIN, leaving it, the leader's
//! settings, the ready check, and what is playing.

use std::time::Duration;

use eframe::egui::{self, Align, Frame, Layout, Margin, Rect, Sense, Ui, Vec2, pos2, vec2};

use super::super::widgets::menu::{self, Entry};
use super::super::widgets::{self, ArtShape, TextField};
use super::parts::{self, Kind};
use super::{GAP, listeners, queue};
use crate::actions::Action;
use crate::state::State;
use crate::theme::{self, Icon};
use crate::together::protocol::{Mode, Policy};
use crate::together::{Ask, Room, Setting, Transport};

/// The listeners' column, where it stands beside the room.
const LISTENERS_WIDTH: f32 = 270.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let me = state.together.me.as_str();
    let leads = room.leads(me);
    ui.add_space(22.0);
    head(state, ui, actions, room);
    ui.add_space(22.0);
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
        now_playing(state, ui, actions, room);
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
        |ui, actions| {
            now_playing(state, ui, actions, room);
            queue::show(state, ui, actions, room);
        },
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

/// What the room is called, the PIN that invites to it, and the way out.
fn head(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        ui.vertical(|ui| {
            ui.set_max_width((ui.available_width() - 330.0).max(160.0));
            parts::eyebrow(state, ui, room.mode.title());
            parts::heading(ui, &room.title(), 22.0);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if room.leads(&state.together.me) {
                let gear = widgets::IconButton {
                    icon: Icon::Settings,
                    size: 20.0,
                    tooltip: "Room settings",
                    active: state.together.settings_open,
                };
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                if gear.show_at(ui, palette, rect.center()).clicked() {
                    actions.push(Action::Room(Ask::ToggleSettings));
                }
            }
            if parts::button(ui, palette, Kind::Secondary, None, "Leave room").clicked() {
                actions.push(Action::Room(Ask::OpenLeave));
            }
            pin_share(state, ui, actions, room);
        });
    });
}

/// The PIN, large, as a button that copies it.
fn pin_share(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let shown = spaced(&room.pin);
    let figures = widgets::tracked(
        ui,
        &shown,
        theme::semibold(22.0),
        palette.text,
        2.0,
        (f32::INFINITY, 1),
    );
    let size = vec2(figures.size().x + 28.0, 46.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    widgets::name(ui, &response, "Copy room PIN");
    let lift = widgets::hover(ui, &response);
    let caption = pos2(rect.left(), rect.top() + 6.0);
    let font = theme::regular(10.5);
    let align = egui::Align2::LEFT_CENTER;
    widgets::text_at(
        ui,
        caption,
        align,
        "Invite with PIN",
        font,
        palette.secondary,
    );
    let at = pos2(rect.left(), rect.bottom() - figures.size().y - 2.0);
    let copy = Rect::from_center_size(
        pos2(
            at.x + figures.size().x + 16.0,
            at.y + figures.size().y / 2.0,
        ),
        Vec2::splat(18.0),
    );
    ui.painter().galley(at, figures, palette.text);
    let ink = crate::tint::blend(palette.secondary, palette.text, lift);
    widgets::paint_icon(ui, Icon::Copy, copy, 18.0, ink);
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
        parts::heading(ui, title, 23.0);
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
        parts::heading(ui, "Room settings", 23.0);
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

/// What the room is playing, who added it, and the room's own transport.
fn now_playing(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let me = state.together.me.as_str();
    let steers = room.may_control(me);
    let current = room.current().map(|(_, entry)| entry);
    let narrow = ui.available_width() < 430.0;
    let art = if narrow { 80.0 } else { 140.0 };
    let fill = crate::tint::blend(palette.surface, palette.accent, 0.07);
    Frame::new()
        .fill(fill)
        .corner_radius(parts::PANEL_RADIUS)
        .inner_margin(Margin::same(22))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 22.0;
                let (cover, _) = ui.allocate_exact_size(Vec2::splat(art), Sense::hover());
                let pictures = current.map_or(&[][..], |entry| &entry.track.artwork);
                let shape = ArtShape::Rounded(9);
                widgets::artwork(ui, state, pictures, cover, shape, Icon::Music);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 5.0;
                    let eyebrow = if room.playing {
                        "NOW PLAYING, TOGETHER"
                    } else {
                        "READY WHEN YOU ARE"
                    };
                    parts::eyebrow(state, ui, eyebrow);
                    match current {
                        Some(entry) => {
                            parts::heading(ui, &entry.track.title, 23.0);
                            parts::quiet(state, ui, &entry.track.artist_names());
                            added_by(state, ui, &entry.added_by);
                        }
                        None => {
                            parts::heading(ui, "Make the first pick.", 23.0);
                            let hint = if !room.may_add(me) {
                                "The leader will pick the first song."
                            } else if room.requesting(me) {
                                "Request a song below. The leader picks what plays."
                            } else {
                                "Add a song below, or use search anywhere in the app."
                            };
                            parts::quiet(state, ui, hint);
                        }
                    }
                    ui.add_space(4.0);
                    transport(state, ui, actions, room, steers);
                    if let Some(person) = &room.last_controlled_by {
                        parts::small(state, ui, &format!("Last controlled by {}", person.name));
                    }
                });
            });
        });
}

/// A small picture and "Added by".
fn added_by(state: &State, ui: &mut Ui, person: &crate::together::protocol::Person) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 7.0;
        let (disc, _) = ui.allocate_exact_size(Vec2::splat(parts::AVATAR_SMALL), Sense::hover());
        parts::avatar(state, ui, disc, &person.name, &person.avatar);
        parts::small(state, ui, &format!("Added by {}", person.name));
    });
}

/// Previous, play or pause, next, and the vote to skip where the room has
/// one. The three are the room's, for those who steer it.
fn transport(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room, steers: bool) {
    let palette = &state.palette;
    let ask = |actions: &mut Vec<Action>, transport| {
        actions.push(Action::Room(Ask::Transport(transport)));
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.add_enabled_ui(steers, |ui| {
            let back = Icon::SkipBackFilled;
            if widgets::icon_button(ui, palette, back, 18.0, "Previous song").clicked() {
                ask(actions, Transport::Previous);
            }
        });
        ui.add_enabled_ui(steers && room.current.is_some(), |ui| {
            let (icon, label) = if room.playing {
                (Icon::PauseFilled, "Pause room")
            } else {
                (Icon::PlayFilled, "Play room")
            };
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::click());
            widgets::name(ui, &response, label);
            let lift = widgets::hover(ui, &response);
            let fill = crate::tint::blend(palette.accent, palette.accent_hover, lift);
            let fill = if ui.is_enabled() {
                fill
            } else {
                fill.gamma_multiply(0.4)
            };
            ui.painter().circle_filled(rect.center(), 22.0, fill);
            widgets::paint_icon(ui, icon, rect, 20.0, palette.on_accent);
            if response.on_hover_text(label).clicked() {
                ask(actions, Transport::Toggle);
            }
        });
        ui.add_enabled_ui(steers, |ui| {
            let on = Icon::SkipForwardFilled;
            if widgets::icon_button(ui, palette, on, 18.0, "Next song").clicked() {
                ask(actions, Transport::Next);
            }
        });
        if room.vote_skip {
            let label = format!("Vote to skip · {}", room.votes.len());
            if parts::text_button(ui, palette, None, &label).clicked() {
                actions.push(Action::Room(Ask::VoteSkip));
            }
        }
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
