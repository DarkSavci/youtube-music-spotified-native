//! The Listen Together page: choose a server, make or join a room, and
//! while in one, see what it plays, what waits, and who is there.

mod listeners;
mod lobby;
mod parts;
mod queue;
mod room;
mod servers;

use eframe::egui::{self, Align, Frame, Layout, Margin, Sense, Ui, vec2};

use super::widgets;
use crate::actions::Action;
use crate::state::State;
use crate::theme::{self, Icon};
use crate::together::{Ask, Phase};

const MAX_WIDTH: f32 = 1168.0;
/// From this width panels sit side by side: the two ways into a room, and
/// the room beside who is in it.
const TWO_COLUMNS_FROM: f32 = 650.0;
const GAP: f32 = 20.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    // No wider than reads well, and never wider than there is room for.
    ui.set_max_width(MAX_WIDTH.min(ui.available_width()));
    ui.spacing_mut().item_spacing.y = 6.0;
    ui.add_space(14.0);
    heading(state, ui);
    ui.add_space(18.0);
    servers::bar(state, ui, actions);
    if let Some(form) = &state.together.manage {
        ui.add_space(GAP);
        servers::form(state, ui, actions, form);
    }
    if let Some(error) = &state.together.error {
        ui.add_space(GAP - 2.0);
        alert(state, ui, actions, error);
    }
    match (state.together.phase, &state.together.room) {
        (Phase::Joined | Phase::Reconnecting, Some(found)) => room::show(state, ui, actions, found),
        (phase, _) => {
            lobby::show(state, ui, actions);
            waiting(state, ui, actions, phase);
        }
    }
    ui.add_space(26.0);
    ui.scope(|ui| {
        ui.set_max_width(650.0_f32.min(ui.available_width()));
        parts::small(
            state,
            ui,
            "Music plays through each person’s own account. Volume stays personal. \
             Only your chosen name, picture and room activity are shared.",
        );
    });
    ui.add_space(12.0);
}

/// The page's name with what it is for, and that it is a preview.
fn heading(state: &State, ui: &mut Ui) {
    let palette = &state.palette;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            parts::eyebrow(state, ui, "YOUR MUSIC, TOGETHER");
            let width = ui.available_width() - 110.0;
            let title = widgets::tracked(
                ui,
                "Listen Together",
                theme::bold(40.0),
                palette.text,
                -1.4,
                (width, 1),
            );
            let (rect, response) = ui.allocate_exact_size(title.size(), Sense::hover());
            parts::read_out(&response, "Listen Together");
            ui.painter().galley(rect.min, title, palette.text);
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("A shared queue. Your own sound.")
                    .font(theme::regular(14.0))
                    .color(palette.secondary),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let font = theme::regular(11.0);
            let said =
                ui.painter()
                    .layout_no_wrap("Preview · v2".to_owned(), font, palette.secondary);
            let size = said.size() + vec2(22.0, 13.0);
            let (badge, response) = ui.allocate_exact_size(size, Sense::hover());
            parts::read_out(&response, "Preview · v2");
            let stroke = egui::Stroke::new(1.0, palette.outline);
            ui.painter()
                .rect_stroke(badge, 20, stroke, egui::StrokeKind::Inside);
            let at = badge.center() - said.size() / 2.0;
            ui.painter().galley(at, said, palette.secondary);
        });
    });
}

/// Something that went wrong, on a tint of the colour that says so, until
/// it is dismissed.
fn alert(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, text: &str) {
    let palette = &state.palette;
    Frame::new()
        .fill(palette.danger.gamma_multiply(0.12))
        .stroke((1.0, palette.danger.gamma_multiply(0.3)))
        .corner_radius(10)
        .inner_margin(Margin::symmetric(16, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let room = ui.available_width() - 40.0;
                let said = egui::RichText::new(text).font(theme::regular(13.0));
                ui.scope(|ui| {
                    ui.set_max_width(room);
                    ui.add(egui::Label::new(said).wrap());
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let dismiss = widgets::icon_button(ui, palette, Icon::X, 16.0, "Dismiss error");
                    if dismiss.clicked() {
                        actions.push(Action::Room(Ask::DismissError));
                    }
                });
            });
        });
}

/// On the way into a room: waiting to be let in, or still connecting.
fn waiting(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, phase: Phase) {
    let palette = &state.palette;
    match phase {
        Phase::Waiting => {
            ui.add_space(GAP);
            parts::panel(state, ui, |ui| {
                parts::heading(ui, "Waiting for the leader…", 23.0);
                parts::quiet(
                    state,
                    ui,
                    "Your name and picture were sent for approval. You’ll join when they \
                     accept.",
                );
                ui.add_space(10.0);
                let cancel =
                    parts::button(ui, palette, parts::Kind::Secondary, None, "Cancel request");
                if cancel.clicked() {
                    actions.push(Action::TogetherLeave);
                }
            });
        }
        Phase::Connecting => {
            ui.add_space(8.0);
            if parts::text_button(ui, palette, None, "Cancel connection").clicked() {
                actions.push(Action::TogetherLeave);
            }
        }
        _ => {}
    }
}

/// Two panels side by side, the first `share` of the width, or one above
/// the other where there is no room for that. Both are handed `actions`,
/// which two closures could not both hold.
fn columns(
    ui: &mut Ui,
    share: f32,
    actions: &mut Vec<Action>,
    left: impl FnOnce(&mut Ui, &mut Vec<Action>),
    right: impl FnOnce(&mut Ui, &mut Vec<Action>),
) {
    let width = ui.available_width();
    if width < TWO_COLUMNS_FROM {
        left(ui, actions);
        ui.add_space(GAP);
        right(ui, actions);
        return;
    }
    let first = ((width - GAP) * share).round();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        let top_down = Layout::top_down(Align::Min);
        ui.allocate_ui_with_layout(vec2(first, 0.0), top_down, |ui| {
            ui.set_width(first);
            left(ui, actions);
        });
        let rest = width - GAP - first;
        ui.allocate_ui_with_layout(vec2(rest, 0.0), top_down, |ui| {
            ui.set_width(rest);
            right(ui, actions);
        });
    });
}
