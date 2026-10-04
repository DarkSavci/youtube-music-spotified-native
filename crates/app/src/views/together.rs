//! The Listen Together page: choose a server, make or join a room, and
//! while in one, see who is there. The room's music shows where music
//! always does: in the player bar and the queue.

use eframe::egui::{self, Align2, Frame, Margin, Rect, Sense, Ui, pos2, vec2};

use super::widgets::{self, ArtShape, TextField};
use crate::actions::Action;
use crate::state::State;
use crate::theme::{self, Icon};
use crate::together::protocol::Member;
use crate::together::{Field, Mode, Phase, Room};

const MAX_WIDTH: f32 = 860.0;
/// From this width the two ways into a room sit side by side.
const TWO_COLUMNS_FROM: f32 = 620.0;
const GAP: f32 = 12.0;
const AVATAR: f32 = 34.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    // No wider than reads well, and never wider than there is room for.
    ui.set_max_width(MAX_WIDTH.min(ui.available_width()));
    hero(state, ui);
    if let Some(error) = &state.together.error {
        ui.add_space(4.0);
        notice(state, ui, error);
    }
    match (state.together.phase, &state.together.room) {
        (Phase::Joined | Phase::Reconnecting, Some(room)) => in_room(state, ui, actions, room),
        (Phase::Idle, _) => lobby(state, ui, actions),
        (phase, _) => waiting(state, ui, actions, phase),
    }
    ui.add_space(18.0);
    quiet(
        state,
        ui,
        "Music plays through each person's own account. Volume stays personal. \
         Only your chosen name and what the room plays are shared.",
    );
}

/// The page's name, with what it is for.
fn hero(state: &State, ui: &mut Ui) {
    let palette = &state.palette;
    ui.horizontal(|ui| {
        let (disc, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
        ui.painter()
            .circle_filled(disc.center(), 28.0, palette.accent.gamma_multiply(0.18));
        widgets::paint_icon(ui, Icon::Users, disc, 26.0, palette.accent);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.add_space(2.0);
            ui.label(egui::RichText::new("Listen Together").font(theme::bold(30.0)));
            quiet(state, ui, "A shared queue. Your own sound.");
        });
    });
    ui.add_space(6.0);
}

fn quiet(state: &State, ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::regular(13.0))
            .color(state.palette.secondary),
    );
}

fn heading(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).font(theme::bold(18.0)));
}

/// Something that went wrong, on a tint of the colour that says so.
fn notice(state: &State, ui: &mut Ui, text: &str) {
    let palette = &state.palette;
    Frame::new()
        .fill(palette.danger.gamma_multiply(0.14))
        .corner_radius(theme::RADIUS)
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            widgets::error(ui, palette, text);
        });
}

fn card(state: &State, ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(state.palette.surface.gamma_multiply(0.6))
        .stroke((1.0, state.palette.outline))
        .corner_radius(theme::RADIUS + 4)
        .inner_margin(Margin::same(20))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            contents(ui);
        });
}

/// A labelled text field. The view may not change state, so the field
/// hands back what was typed and the page asks for it to be kept.
fn field(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    (label, hint): (&str, &str),
    value: &str,
    field: Field,
) {
    ui.label(
        egui::RichText::new(label)
            .font(theme::medium(12.5))
            .color(state.palette.secondary),
    );
    ui.add_space(2.0);
    let input = TextField {
        text: value,
        hint,
        label,
        icon: None,
        width: ui.available_width(),
    };
    if let Some(typed) = input.show(ui, &state.palette) {
        actions.push(Action::TogetherField(field, typed));
    }
    ui.add_space(10.0);
}

fn lobby(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    ui.add_space(GAP);
    card(state, ui, |ui| {
        ui.columns(2, |columns| {
            let server = ("Server", "wss://listen.example.com");
            let address = &state.settings.together_server;
            field(
                state,
                &mut columns[0],
                actions,
                server,
                address,
                Field::Server,
            );
            let account = state.account.as_ref().map_or("Listener", |me| &me.name);
            let name = &state.settings.together_name;
            let joining = ("Joining as", account);
            field(state, &mut columns[1], actions, joining, name, Field::Name);
        });
        quiet(
            state,
            ui,
            "The server is the relay your friends use too: a room's PIN only works on \
             the server it was made on.",
        );
    });
    ui.add_space(GAP);
    if ui.available_width() >= TWO_COLUMNS_FROM {
        ui.columns(2, |columns| {
            create(state, &mut columns[0], actions);
            join(state, &mut columns[1], actions);
        });
    } else {
        create(state, ui, actions);
        ui.add_space(GAP);
        join(state, ui, actions);
    }
}

fn create(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let form = &state.together.form;
    card(state, ui, |ui| {
        heading(ui, "Start a room");
        quiet(state, ui, "Pick who steers, then invite your people.");
        ui.add_space(12.0);
        let room_name = ("Room name (optional)", "Friday night together");
        field(
            state,
            ui,
            actions,
            room_name,
            &form.room_name,
            Field::RoomName,
        );
        for mode in Mode::EVERY {
            if mode_choice(state, ui, mode, form.mode == mode) {
                actions.push(Action::TogetherMode(mode));
            }
            ui.add_space(6.0);
        }
        ui.add_space(8.0);
        if widgets::pill_button(ui, &state.palette, "Create room").clicked() {
            actions.push(Action::TogetherCreate);
        }
    });
}

/// One way a room can be run, as a row to choose: its name, what it
/// means, and a mark on the one chosen.
fn mode_choice(state: &State, ui: &mut Ui, mode: Mode, chosen: bool) -> bool {
    let palette = &state.palette;
    let size = vec2(ui.available_width(), 54.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    widgets::name(ui, &response, mode.title());
    let lift = widgets::hover(ui, &response);
    let outline = if chosen {
        palette.accent
    } else {
        crate::tint::blend(palette.outline, palette.dim, lift)
    };
    let fill = if chosen {
        palette.accent.gamma_multiply(0.12)
    } else {
        widgets::wash(ui, lift)
    };
    ui.painter().rect(
        rect,
        theme::RADIUS,
        fill,
        (1.0, outline),
        egui::StrokeKind::Inside,
    );
    let dot = pos2(rect.left() + 20.0, rect.center().y);
    ui.painter().circle_stroke(dot, 7.0, (1.5, outline));
    if chosen {
        ui.painter().circle_filled(dot, 4.0, palette.accent);
    }
    let left = rect.left() + 40.0;
    let width = rect.right() - left - 12.0;
    let title = widgets::elided(
        ui,
        mode.title(),
        theme::semibold(14.0),
        palette.text,
        width,
        1,
    );
    ui.painter()
        .galley(pos2(left, rect.center().y - 18.0), title, palette.text);
    let font = theme::regular(12.0);
    let about = widgets::elided(ui, mode.about(), font, palette.secondary, width, 1);
    ui.painter()
        .galley(pos2(left, rect.center().y + 2.0), about, palette.secondary);
    response.clicked() && !chosen
}

fn join(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let form = &state.together.form;
    card(state, ui, |ui| {
        heading(ui, "Join a room");
        quiet(state, ui, "Enter the 8-digit PIN from whoever started it.");
        ui.add_space(12.0);
        let pin = ("Room PIN", "0000 0000");
        field(state, ui, actions, pin, &form.pin, Field::Pin);
        if widgets::pill_button(ui, &state.palette, "Join room").clicked() {
            actions.push(Action::TogetherJoin);
        }
        ui.add_space(10.0);
        quiet(
            state,
            ui,
            "Joining puts your own queue by. When you leave, the music carries on \
             from the room's queue.",
        );
    });
}

/// On the way into a room: connecting, or waiting to be let in.
fn waiting(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, phase: Phase) {
    let palette = &state.palette;
    let (title, about) = if phase == Phase::Waiting {
        (
            "Waiting for the leader…",
            "Your name was sent for approval. You'll join when they accept.",
        )
    } else {
        ("Connecting…", "Reaching the server and finding the room.")
    };
    ui.add_space(GAP);
    card(state, ui, |ui| {
        ui.horizontal(|ui| {
            widgets::spinner(ui, palette, 20.0);
            ui.label(egui::RichText::new(title).font(theme::semibold(16.0)));
        });
        quiet(state, ui, about);
        ui.add_space(10.0);
        if widgets::outline_button(ui, palette, "Cancel").clicked() {
            actions.push(Action::TogetherLeave);
        }
    });
}

/// The PIN as it is read out: two groups of four.
fn spaced(pin: &str) -> String {
    match pin.len() {
        8 => format!("{} {}", &pin[..4], &pin[4..]),
        _ => pin.to_owned(),
    }
}

fn in_room(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let leader = room.leader().map_or("Someone", |leader| &leader.name);
    ui.add_space(GAP);
    card(state, ui, |ui| {
        ui.label(
            egui::RichText::new(room.mode.title().to_uppercase())
                .font(theme::semibold(11.5))
                .color(palette.accent),
        );
        let name = if room.name.is_empty() {
            format!("{leader}'s room")
        } else {
            room.name.clone()
        };
        ui.label(egui::RichText::new(name).font(theme::bold(26.0)));
        if state.together.phase == Phase::Reconnecting {
            ui.horizontal(|ui| {
                widgets::spinner(ui, palette, 14.0);
                quiet(state, ui, "Reconnecting…");
            });
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            // The PIN stands out: it is what gets read to a friend.
            Frame::new()
                .fill(palette.surface)
                .corner_radius(theme::RADIUS)
                .inner_margin(Margin::symmetric(14, 8))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        quiet(state, ui, "Invite with PIN");
                        ui.label(egui::RichText::new(spaced(&room.pin)).font(theme::bold(22.0)));
                    });
                });
            ui.add_space(4.0);
            if widgets::outline_button(ui, palette, "Copy PIN").clicked() {
                actions.push(Action::CopyText {
                    text: room.pin.clone(),
                    said: "PIN copied. Friends need the same server.",
                });
            }
            if widgets::outline_button(ui, palette, "Leave room").clicked() {
                actions.push(Action::TogetherLeave);
            }
        });
    });
    ui.add_space(GAP);
    if ui.available_width() >= TWO_COLUMNS_FROM {
        ui.columns(2, |columns| {
            now_playing(state, &mut columns[0], room);
            listeners(state, &mut columns[1], room);
        });
    } else {
        now_playing(state, ui, room);
        ui.add_space(GAP);
        listeners(state, ui, room);
    }
}

fn now_playing(state: &State, ui: &mut Ui, room: &Room) {
    let palette = &state.palette;
    card(state, ui, |ui| {
        let heading = if room.current.is_some() {
            "NOW PLAYING, TOGETHER"
        } else {
            "READY WHEN YOU ARE"
        };
        ui.label(
            egui::RichText::new(heading)
                .font(theme::semibold(11.5))
                .color(palette.secondary),
        );
        ui.add_space(8.0);
        match room.current() {
            Some((_, entry)) => {
                let track = entry.track.to_track();
                ui.horizontal(|ui| {
                    let (cover, _) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::hover());
                    let shape = ArtShape::Rounded(theme::RADIUS_ROW);
                    widgets::artwork(ui, state, &track.artwork, cover, shape, Icon::Music);
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.add_space(6.0);
                        let title = egui::RichText::new(&track.title).font(theme::bold(17.0));
                        ui.add(egui::Label::new(title).truncate());
                        quiet(state, ui, &track.artist_names());
                        quiet(state, ui, &format!("Added by {}", entry.added_by.name));
                    });
                });
            }
            None => {
                ui.label(egui::RichText::new("Make the first pick.").font(theme::bold(18.0)));
                quiet(state, ui, "Play any song and the room plays it with you.");
            }
        }
        if !state.together.may_control() {
            ui.add_space(8.0);
            quiet(state, ui, "The leader and DJs steer this room.");
        }
    });
}

fn listeners(state: &State, ui: &mut Ui, room: &Room) {
    card(state, ui, |ui| {
        let count = room.members.len();
        ui.label(egui::RichText::new(format!("In the room · {count}")).font(theme::bold(15.0)));
        ui.add_space(6.0);
        for member in &room.members {
            listener(state, ui, room, member);
        }
    });
}

/// One listener: an initial on a disc, their name, and what they are.
fn listener(state: &State, ui: &mut Ui, room: &Room, member: &Member) {
    let palette = &state.palette;
    let size = vec2(ui.available_width(), AVATAR + 10.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let disc = Rect::from_center_size(
        pos2(rect.left() + AVATAR / 2.0, rect.center().y),
        vec2(AVATAR, AVATAR),
    );
    let leads = member.id == room.owner;
    let fill = if leads {
        palette.accent
    } else {
        palette.surface_active
    };
    ui.painter()
        .circle_filled(disc.center(), AVATAR / 2.0, fill);
    let initial: String = member
        .name
        .chars()
        .take(1)
        .flat_map(char::to_uppercase)
        .collect();
    let ink = if leads {
        palette.on_accent
    } else {
        palette.text
    };
    widgets::text_at(
        ui,
        disc.center(),
        Align2::CENTER_CENTER,
        &initial,
        theme::bold(14.0),
        ink,
    );
    let left = disc.right() + 12.0;
    let you = if member.id == state.together.me {
        " (you)"
    } else {
        ""
    };
    let name = format!("{}{you}", member.name);
    let at = pos2(left, rect.center().y - 9.0);
    widgets::text_at(
        ui,
        at,
        Align2::LEFT_CENTER,
        &name,
        theme::medium(14.0),
        palette.text,
    );
    let role = if leads {
        "Leader"
    } else if member.role == "dj" {
        "DJ"
    } else {
        "Listener"
    };
    let doing = if member.connected {
        member.status.as_str()
    } else {
        "reconnecting"
    };
    let detail = format!("{role} · {doing}");
    let at = pos2(left, rect.center().y + 9.0);
    let color = if member.connected {
        palette.secondary
    } else {
        palette.warning
    };
    widgets::text_at(
        ui,
        at,
        Align2::LEFT_CENTER,
        &detail,
        theme::regular(12.0),
        color,
    );
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
