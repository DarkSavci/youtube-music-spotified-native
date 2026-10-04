//! The equalizer's panel: the switch, the graph of sliders, the curves to
//! start from, and the level kept under them.
//!
//! There is one panel. Settings shows it in a card, and the buttons in the
//! player bar and at the top of the queue open the same one over the page,
//! where it can be reached while the music is being listened to.

mod graph;

use eframe::egui::{
    self, Align, Align2, CornerRadius, Frame, Key, Layout, Margin, Pos2, Rect, RectAlign, Response,
    Sense, Stroke, Ui, vec2,
};

use super::widgets;
use crate::actions::Action;
use crate::equalizer::{Ask, NAME_LENGTH, Named, PRESETS, decibels, named};
use crate::state::State;
use crate::theme::{self, Icon, Palette};

/// How wide the panel is when it opens over the page.
const POPUP_WIDTH: f32 = 580.0;
/// The most it comes to in height, with a name being typed and a few
/// saved curves: a window with less room than this scrolls it.
const POPUP_HEIGHT: f32 = 520.0;
/// The window that is left around it.
const WINDOW_MARGIN: f32 = 16.0;
const NAME_FIELD: &str = "Preset name";

/// Where the panel is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Opened over the page from a button.
    Popup,
    /// In a card on the Settings page, which scrolls.
    Page,
}

impl Place {
    fn graph_height(self) -> f32 {
        match self {
            Place::Popup => 190.0,
            Place::Page => 210.0,
        }
    }
}

/// The button in the player bar, centred on `at`, with the panel it opens
/// above it.
pub fn button_at(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, at: Pos2) {
    let rect = Rect::from_center_size(at, vec2(30.0, 30.0));
    let response = ui.interact(rect, ui.id().with("equalizer"), Sense::click());
    opener(state, ui, actions, (response, "Equalizer"), RectAlign::TOP);
}

/// The button at the top of the queue, with the panel it opens under it.
pub fn button(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let (_, response) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    let align = RectAlign::BOTTOM_END;
    opener(state, ui, actions, (response, "Queue equalizer"), align);
}

/// A button that opens the panel: the sliders icon, in the accent while
/// the equalizer is switched on, so that a shaped sound is never mistaken
/// for the song's own.
fn opener(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    (response, name): (Response, &str),
    align: RectAlign,
) {
    let palette = &state.palette;
    widgets::name(ui, &response, name);
    let rect = response.rect;
    let lift = widgets::hover(ui, &response);
    ui.painter()
        .circle_filled(rect.center(), rect.width() / 2.0, widgets::wash(ui, lift));
    let colour = if state.settings.equalizer_on {
        palette.accent
    } else {
        crate::tint::blend(palette.secondary, palette.text, lift)
    };
    let size = if response.is_pointer_button_down_on() {
        16.5
    } else {
        18.0
    };
    widgets::paint_icon(ui, Icon::SlidersVertical, rect, size, colour);
    let response = response.on_hover_text("Equalizer");

    let window = ui.ctx().content_rect().size();
    let width = POPUP_WIDTH.min(window.x - WINDOW_MARGIN * 2.0);
    let frame = Frame::new()
        .fill(palette.overlay)
        .stroke((1.0, palette.outline))
        .corner_radius(12)
        .inner_margin(Margin::same(16))
        .shadow(egui::epaint::Shadow {
            offset: [0, 16],
            blur: 40,
            spread: 0,
            color: palette.shadow,
        });
    egui::Popup::from_toggle_button_response(&response)
        .align(align)
        .gap(8.0)
        .frame(frame)
        .width(width)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_width(width - 32.0);
            // A window too short for the whole panel scrolls it.
            let room = window.y - theme::PLAYER_BAR_HEIGHT - WINDOW_MARGIN * 5.0;
            if room >= POPUP_HEIGHT {
                panel(state, ui, actions, Place::Popup);
            } else {
                egui::ScrollArea::vertical()
                    .max_height(room)
                    .auto_shrink([false, true])
                    .show(ui, |ui| panel(state, ui, actions, Place::Popup));
            }
        });
}

/// The panel itself, across the width `ui` has.
pub fn panel(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, place: Place) {
    let on = state.settings.equalizer_on;
    ui.spacing_mut().item_spacing.y = 0.0;
    header(state, ui, actions);
    ui.add_space(14.0);
    // Switched off, the controls are drawn back. They still answer: moving
    // a slider or choosing a preset is a wish to hear it, and switches the
    // equalizer on.
    ui.scope(|ui| {
        if !on {
            ui.multiply_opacity(0.5);
        }
        graph::show(state, ui, actions, place);
        ui.add_space(12.0);
        presets(state, ui, actions);
        naming(state, ui, actions);
        ui.add_space(14.0);
        headroom(state, ui, actions);
    });
}

/// What it is and which curve is set, with the reset and the switch.
fn header(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let settings = &state.settings;
    let on = settings.equalizer_on;
    ui.horizontal(|ui| {
        // Painted, and named once for a screen reader: as labels of their
        // own the two lines would say "Equalizer" and the preset a second
        // time, beside the button and the chip that say them.
        let curve = named(settings).label().to_owned();
        let line = if on {
            curve.clone()
        } else {
            format!("Off · {curve}")
        };
        let (rect, response) = ui.allocate_exact_size(vec2(260.0, 36.0), Sense::hover());
        response.widget_info(|| {
            let said = format!("Equalizer preset: {curve}");
            egui::WidgetInfo::labeled(egui::WidgetType::Label, true, said)
        });
        let font = theme::semibold(15.0);
        let at = rect.left_top();
        widgets::text_at(ui, at, Align2::LEFT_TOP, "Equalizer", font, palette.text);
        let font = theme::regular(12.5);
        let at = rect.left_bottom();
        widgets::text_at(ui, at, Align2::LEFT_BOTTOM, &line, font, palette.secondary);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if widgets::switch(ui, palette, on, "Equalizer on").clicked() {
                actions.push(Action::Equalizer(Ask::On(!on)));
            }
            let flat = settings.equalizer == [0.0; 10] && settings.equalizer_preamp == 0.0;
            ui.add_enabled_ui(!flat, |ui| {
                let reset =
                    widgets::icon_button(ui, palette, Icon::RotateCcw, 16.0, "Reset equalizer");
                if reset.clicked() {
                    actions.push(Action::Equalizer(Ask::Reset));
                }
            });
        });
    });
}

/// The curves to start from, and after them the ones saved here.
fn presets(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let settings = &state.settings;
    let current = named(settings);
    let naming = state.equalizer_naming.is_some();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (name, gains) in PRESETS {
            let active = current == Named::BuiltIn(name);
            if widgets::chip(ui, palette, name, active).clicked() && !active {
                actions.push(Action::Equalizer(Ask::Curve(gains)));
            }
        }
        for saved in &settings.equalizer_presets {
            let active = current == Named::Saved(&saved.name);
            if widgets::chip(ui, palette, &saved.name, active).clicked() && !active {
                actions.push(Action::Equalizer(Ask::Curve(saved.gains)));
            }
        }
        // What can be done about the curve that is set, after the curves
        // themselves: a new one can be kept, a kept one renamed or dropped.
        if naming {
            return;
        }
        match current {
            Named::Custom => {
                let save = ghost_chip(ui, palette, "Save as preset", Some(Icon::Plus));
                if save.clicked() {
                    actions.push(Action::Equalizer(Ask::Name));
                }
            }
            Named::Saved(name) => {
                if ghost_chip(ui, palette, "Rename", None).clicked() {
                    actions.push(Action::Equalizer(Ask::Rename(name.to_owned())));
                }
                if ghost_chip(ui, palette, "Delete", None).clicked() {
                    actions.push(Action::Equalizer(Ask::Delete(name.to_owned())));
                }
            }
            Named::BuiltIn(_) => {}
        }
    });
}

/// A chip that does something, where the filled ones choose something: an
/// outline, so the two are not mistaken for each other.
fn ghost_chip(ui: &mut Ui, palette: &Palette, label: &str, icon: Option<Icon>) -> Response {
    const HEIGHT: f32 = 26.0;
    const PADDING: f32 = 12.0;
    const ICON: f32 = 13.0;
    let font = theme::medium(12.0);
    let words = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, palette.text);
    let lead = if icon.is_some() { ICON + 5.0 } else { 0.0 };
    let size = vec2(words.size().x + lead + PADDING * 2.0, HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    // "Rename" and "Delete" are said elsewhere on the Settings page.
    widgets::name(ui, &response, &format!("{label} equalizer preset"));
    if ui.is_rect_visible(rect) {
        let lift = widgets::hover(ui, &response);
        let colour = crate::tint::blend(palette.secondary, palette.text, lift);
        let round = CornerRadius::same(u8::MAX);
        ui.painter()
            .rect_filled(rect, round, widgets::wash(ui, lift * 0.6));
        let outline = Stroke::new(1.0, crate::tint::blend(palette.dim, palette.text, lift));
        ui.painter()
            .rect_stroke(rect, round, outline, egui::StrokeKind::Inside);
        let mut left = rect.left() + PADDING;
        if let Some(icon) = icon {
            let at = Rect::from_center_size(
                egui::pos2(left + ICON / 2.0, rect.center().y),
                vec2(ICON, ICON),
            );
            widgets::paint_icon(ui, icon, at, ICON, colour);
            left += lead;
        }
        let at = egui::pos2(left, rect.center().y);
        let font = theme::medium(12.0);
        widgets::text_at(ui, at, Align2::LEFT_CENTER, label, font, colour);
    }
    response
}

/// The field a name is typed into, while one is being typed.
fn naming(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let Some(naming) = &state.equalizer_naming else {
        return;
    };
    let palette = &state.palette;
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        // The caret goes to the field once, as it appears.
        let id = ui.id().with(("text-field", NAME_FIELD));
        let asked = ui.id().with("naming-asked");
        if !ui.data(|data| data.get_temp::<bool>(asked).unwrap_or(false)) {
            ui.data_mut(|data| data.insert_temp(asked, true));
            ui.memory_mut(|memory| memory.request_focus(id));
        }
        let field = widgets::TextField {
            text: &naming.name,
            hint: "Name this preset",
            label: NAME_FIELD,
            icon: None,
            width: 220.0,
            compact: true,
        };
        if let Some(typed) = field.show(ui, palette) {
            let typed = typed.chars().take(NAME_LENGTH).collect();
            actions.push(Action::Equalizer(Ask::Typed(typed)));
        }
        let (enter, escape) = ui.input(|input| {
            (
                input.key_pressed(Key::Enter),
                input.key_pressed(Key::Escape),
            )
        });
        let ready = !naming.name.trim().is_empty();
        let label = if naming.renaming.is_some() {
            "Rename"
        } else {
            "Save"
        };
        let save = ui
            .add_enabled_ui(ready, |ui| {
                widgets::outline_button_named(ui, palette, label, "Save equalizer preset")
            })
            .inner;
        let cancel = widgets::outline_button_named(ui, palette, "Cancel", "Cancel naming");
        let done = if (save.clicked() || enter) && ready {
            Some(Ask::Save)
        } else if cancel.clicked() || escape {
            Some(Ask::Cancel)
        } else {
            None
        };
        if let Some(done) = done {
            // The next name typed gets the caret afresh.
            ui.data_mut(|data| data.remove::<bool>(asked));
            actions.push(Action::Equalizer(done));
        }
    });
}

/// The switch that keeps the level under what the curve adds, and what
/// that comes to at the moment.
fn headroom(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let on = state.settings.equalizer_headroom;
    let label = "Prevent clipping";
    let about = "Turns the level down by as much as the curve raises it, so that loud \
                 passages are not squashed. Off, the sound is louder and a limiter \
                 catches the peaks.";
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let switch = widgets::switch(ui, palette, on, label).on_hover_text(about);
        if switch.clicked() {
            actions.push(Action::Equalizer(Ask::Headroom(!on)));
        }
        // The words are the switch's name already; painted, they are not
        // said twice, and a click on them is a click on the switch.
        let font = theme::medium(13.0);
        let words = ui
            .painter()
            .layout_no_wrap(label.to_owned(), font, palette.text);
        let (rect, said) = ui.allocate_exact_size(words.size(), Sense::click());
        ui.painter().galley(rect.min, words, palette.text);
        if said.on_hover_text(about).clicked() {
            actions.push(Action::Equalizer(Ask::Headroom(!on)));
        }
        // What the level comes to, with the preamp: said only when it is
        // not nought, and only while it is being applied.
        let gain = graph::gain_db(ui);
        if state.settings.equalizer_on && gain.abs() >= 0.05 {
            let note = format!("Level {}", decibels(gain));
            ui.label(
                egui::RichText::new(note)
                    .font(theme::regular(12.5))
                    .color(palette.secondary),
            );
        }
    });
}
