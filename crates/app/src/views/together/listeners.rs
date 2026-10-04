//! Who is in the room, who waits to be let in, and this listener's own
//! preferences.

use eframe::egui::{self, Rect, Sense, Ui, Vec2, pos2, vec2};

use super::super::widgets::menu::{self, Entry};
use super::super::{format, widgets};
use super::parts;
use crate::actions::Action;
use crate::state::State;
use crate::theme::{self, Icon};
use crate::together::protocol::{Knock, Member};
use crate::together::{Ask, Room};

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room) {
    let palette = &state.palette;
    let leads = room.leads(&state.together.me);
    parts::panel(state, ui, |ui| {
        if leads && !room.pending.is_empty() {
            ui.label(egui::RichText::new("Waiting to join").font(theme::bold(14.0)));
            ui.add_space(6.0);
            for knock in &room.pending {
                waiting(state, ui, actions, knock);
            }
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
        }
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("In the room").font(theme::bold(14.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let count = egui::RichText::new(room.members.len().to_string());
                ui.label(count.font(theme::bold(14.0)).color(palette.secondary));
            });
        });
        ui.add_space(12.0);
        for member in &room.members {
            listener(state, ui, actions, room, member);
            ui.add_space(8.0);
        }
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(12.0);
        let closes = format::clock(room.expires, state.together.zone_minutes);
        parts::small(state, ui, &format!("Room expires at {closes}"));
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Your experience").font(theme::bold(12.0)));
        ui.add_space(6.0);
        let notified = state.settings.together_notifications;
        if parts::check(ui, palette, notified, "Show room activity notifications") {
            actions.push(Action::Room(Ask::Notifications(!notified)));
        }
        let follows = state.settings.together_follow_video;
        let label = "Follow others\u{2019} video display changes";
        if parts::check(ui, palette, follows, label) {
            actions.push(Action::Room(Ask::FollowVideo(!follows)));
        }
        let about = "Everyone hears the same version. You decide whether to show its video.";
        parts::small(state, ui, about);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            // The words line up with the tick boxes above them.
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(-8.0);
            if parts::text_button(ui, palette, None, "Resync me").clicked() {
                actions.push(Action::Room(Ask::Resync));
            }
        });
        parts::small(state, ui, "Only affects your playback.");
    });
}

/// Someone asking to be let in: accept them, or turn them away.
fn waiting(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, knock: &Knock) {
    let palette = &state.palette;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (disc, _) = ui.allocate_exact_size(Vec2::splat(parts::AVATAR_SMALL), Sense::hover());
        parts::avatar(state, ui, disc, &knock.name, &knock.avatar);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let named = format!("Decline {}", knock.name);
            if widgets::icon_button(ui, palette, Icon::X, 14.0, &named).clicked() {
                actions.push(Action::Room(Ask::TurnAway(knock.id.clone())));
            }
            let named = format!("Accept {}", knock.name);
            if parts::text_button_named(ui, palette, ("Accept", &named)).clicked() {
                actions.push(Action::Room(Ask::Admit(knock.id.clone())));
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let name = egui::RichText::new(&knock.name).font(theme::regular(12.0));
                ui.add(egui::Label::new(name).truncate());
            });
        });
    });
}

/// One listener: their picture, their name, what they are and are doing,
/// and for the leader a menu of what can be done about them.
fn listener(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, room: &Room, member: &Member) {
    let palette = &state.palette;
    let me = state.together.me.as_str();
    let size = vec2(ui.available_width(), parts::AVATAR);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let disc = Rect::from_min_size(rect.min, Vec2::splat(parts::AVATAR));
    parts::avatar(state, ui, disc, &member.name, &member.avatar);
    let manages = room.leads(me) && member.id != me;
    let left = disc.right() + 10.0;
    let width = rect.right() - left - if manages { 32.0 } else { 0.0 };
    let you = if member.id == me { " (you)" } else { "" };
    let called = format!("{}{you}", member.name);
    let name = widgets::elided(ui, &called, theme::semibold(12.5), palette.text, width, 1);
    ui.painter()
        .galley(pos2(left, rect.center().y - 16.0), name, palette.text);
    let role = if room.leads(&member.id) {
        "Leader"
    } else if member.role == "dj" {
        "DJ"
    } else {
        "Listener"
    };
    let (doing, color) = if member.connected {
        (member.status.as_str(), palette.secondary)
    } else {
        ("Reconnecting", palette.warning)
    };
    let detail = format!("{role} · {doing}");
    parts::read_out(&response, &format!("{called}, {detail}"));
    let detail = widgets::elided(ui, &detail, theme::regular(10.5), color, width, 1);
    ui.painter()
        .galley(pos2(left, rect.center().y + 3.0), detail, color);
    if !manages {
        return;
    }
    let gear = widgets::IconButton {
        icon: Icon::Settings,
        size: 15.0,
        tooltip: &format!("Manage {}", member.name),
        active: false,
    };
    let at = pos2(rect.right() - 14.0, rect.center().y);
    let opener = gear.show_at(ui, palette, at);
    menu::popup(&opener, palette, |menu| {
        let is_dj = member.role == "dj";
        let (role, icon) = if is_dj {
            ("Make listener", Icon::Headphones)
        } else {
            ("Make DJ", Icon::Disc)
        };
        if menu.entry(Entry::new(role).icon(icon)) {
            actions.push(Action::Room(Ask::Role {
                member: member.id.clone(),
                dj: !is_dj,
            }));
        }
        let lead = Entry::new("Make leader").icon(Icon::User);
        if menu.entry(lead.enabled(member.connected)) {
            actions.push(Action::Room(Ask::MakeLeader(member.id.clone())));
        }
        menu.separator();
        if menu.entry(Entry::new("Remove & rotate PIN").danger()) {
            actions.push(Action::Room(Ask::RemoveListener {
                member: member.id.clone(),
                name: member.name.clone(),
            }));
        }
    });
}
