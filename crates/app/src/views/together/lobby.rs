//! Before a room: who you join as, and the two ways in.

use eframe::egui::{self, Frame, Margin, Sense, Ui, Vec2, pos2, vec2};

use super::super::widgets::{self, TextField};
use super::parts::{self, Kind};
use crate::actions::Action;
use crate::state::State;
use crate::theme::{self, Icon};
use crate::together::protocol::picture_of;
use crate::together::{Ask, Field, Mode, Phase};

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    ui.add_space(22.0);
    // The two panels are as tall as the taller was on the frame before, as
    // the cells of a grid row are.
    let id = ui.id().with("lobby-height");
    // One above the other, each is as tall as it is.
    let side_by_side = ui.available_width() >= super::TWO_COLUMNS_FROM;
    let tall = if side_by_side {
        ui.data(|data| data.get_temp::<f32>(id)).unwrap_or(0.0)
    } else {
        0.0
    };
    let (mut made, mut joined) = (0.0, 0.0);
    super::columns(
        ui,
        0.555,
        actions,
        |ui, actions| made = create(state, ui, actions, tall),
        |ui, actions| joined = join(state, ui, actions, tall),
    );
    let tallest = f32::max(made, joined);
    if (tallest - tall).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(id, tallest));
        ui.ctx().request_repaint();
    }
}

/// The picture and name this listener is seen by, as rows of the
/// Connection card.
pub(super) fn identity(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let account = state.account.as_ref();
    let account_name = account.map_or("Listener", |account| &account.name);
    let typed = state.settings.together_name.trim();
    let name = if typed.is_empty() {
        account_name
    } else {
        typed
    };
    let has_picture = account.is_some_and(|account| !account.avatar_url.is_empty());
    let shares = has_picture && state.settings.together_share_picture;
    let about = "What the people in a room see you as.";
    super::servers::row(state, ui, ("Your name", about), |ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let field = TextField {
            text: &state.settings.together_name,
            hint: account_name,
            label: "Joining as",
            icon: None,
            width: 200.0,
            compact: true,
        };
        if let Some(typed) = field.show(ui, palette) {
            actions.push(Action::TogetherField(Field::Name, typed));
        }
        let (disc, _) =
            ui.allocate_exact_size(Vec2::splat(parts::AVATAR_SMALL + 6.0), Sense::hover());
        let picture = match account {
            Some(account) if shares => picture_of(&account.avatar_url),
            _ => Vec::new(),
        };
        parts::avatar(state, ui, disc, name, &picture);
    });
    if has_picture {
        ui.add_space(6.0);
        let about = "Shown beside your name. Otherwise your initial is.";
        super::servers::row(state, ui, ("Profile picture", about), |ui| {
            if parts::check(ui, palette, shares, "Share my profile picture") {
                actions.push(Action::Room(Ask::SharePicture(!shares)));
            }
        });
    }
}

/// A panel of the lobby, at least `tall`. Returns how tall it came out
/// without that floor.
fn lobby_panel(state: &State, ui: &mut Ui, tall: f32, contents: impl FnOnce(&mut Ui)) -> f32 {
    let mut natural = 0.0;
    let padding = f32::from(parts::PANEL_PADDING) * 2.0;
    parts::panel(state, ui, |ui| {
        let top = ui.cursor().top();
        // Asked for before anything is in it: a least height is counted
        // from where the next thing would go.
        ui.set_min_height((tall - padding).max(0.0));
        contents(ui);
        let gap = ui.spacing().item_spacing.y;
        natural = ui.cursor().top() - gap - top + padding;
    });
    natural
}

fn create(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, tall: f32) -> f32 {
    let palette = &state.palette;
    let settings = &state.settings;
    lobby_panel(state, ui, tall, |ui| {
        parts::heading(ui, "Start a room", 18.0);
        parts::quiet(
            state,
            ui,
            "Make a room and invite friends with its PIN. You lead it.",
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.label(egui::RichText::new("Room name").font(theme::regular(13.0)));
            parts::quiet(state, ui, "(optional)");
        });
        let field = TextField {
            text: &settings.together_room_name,
            hint: "Friday night together",
            label: "Room name",
            icon: None,
            width: ui.available_width(),
            compact: false,
        };
        if let Some(typed) = field.show(ui, palette) {
            actions.push(Action::TogetherField(Field::RoomName, typed));
        }
        parts::small(
            state,
            ui,
            "Remembered on this device for your next room. Visible to listeners.",
        );
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Who controls the music").font(theme::regular(13.0)));
        for mode in Mode::EVERY {
            if mode_choice(state, ui, mode, settings.together_mode == mode) {
                actions.push(Action::TogetherMode(mode));
            }
        }
        ui.add_space(12.0);
        let connecting = state.together.phase == Phase::Connecting;
        ui.add_enabled_ui(state.together.phase == Phase::Idle, |ui| {
            let label = if connecting {
                "Connecting…"
            } else {
                "Create room"
            };
            if parts::button(ui, palette, Kind::Primary, Some(Icon::Plus), label).clicked() {
                actions.push(Action::TogetherCreate);
            }
        });
    })
}

/// One way a room can be run, as a row to choose: its name, what it
/// means, and a mark on the one chosen.
fn mode_choice(state: &State, ui: &mut Ui, mode: Mode, chosen: bool) -> bool {
    let palette = &state.palette;
    let size = vec2(ui.available_width(), 52.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    widgets::hand(ui, &response);
    response.widget_info(|| {
        let enabled = ui.is_enabled();
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, chosen, mode.title())
    });
    let lift = widgets::hover(ui, &response);
    // A row as the app's lists have them: lit under the pointer, and the
    // chosen one held lit.
    let fill = if chosen {
        palette.surface_active
    } else {
        widgets::wash(ui, lift)
    };
    ui.painter().rect_filled(rect, theme::RADIUS_ROW, fill);
    let dot = pos2(rect.left() + 20.0, rect.center().y);
    if chosen {
        ui.painter().circle_filled(dot, 8.0, palette.accent);
        ui.painter().circle_filled(dot, 3.0, palette.on_accent);
    } else {
        let ring = crate::tint::blend(palette.dim, palette.secondary, lift);
        ui.painter().circle_stroke(dot, 7.5, (1.5, ring));
    }
    let left = rect.left() + 40.0;
    let width = rect.right() - left - 12.0;
    let font = theme::semibold(13.0);
    let title = widgets::elided(ui, mode.title(), font, palette.text, width, 1);
    ui.painter()
        .galley(pos2(left, rect.center().y - 18.0), title, palette.text);
    let font = theme::regular(11.5);
    let about = widgets::elided(ui, mode.about(), font, palette.secondary, width, 1);
    ui.painter()
        .galley(pos2(left, rect.center().y + 3.0), about, palette.secondary);
    response.clicked() && !chosen
}

fn join(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, tall: f32) -> f32 {
    let palette = &state.palette;
    let pin = &state.together.form.pin;
    lobby_panel(state, ui, tall, |ui| {
        parts::heading(ui, "Join a room", 18.0);
        parts::quiet(
            state,
            ui,
            "Enter the 8-digit PIN a friend gave you. You both need the same server.",
        );
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Room PIN").font(theme::regular(13.0)));
        let idle = state.together.phase == Phase::Idle;
        let whole = pin.len() == 8;
        if pin_field(state, ui, actions, pin) && idle && whole {
            actions.push(Action::TogetherJoin);
        }
        ui.add_space(12.0);
        ui.add_enabled_ui(idle && whole, |ui| {
            if parts::button(ui, palette, Kind::Primary, None, "Join room").clicked() {
                actions.push(Action::TogetherJoin);
            }
        });
        ui.add_space(14.0);
        parts::small(
            state,
            ui,
            "Your own queue waits while you are in a room. When you leave, the music \
             goes on from the room’s queue.",
        );
    })
}

/// The PIN, in figures large enough to read out. Returns whether Enter was
/// pressed in it.
fn pin_field(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, pin: &str) -> bool {
    let palette = &state.palette;
    let id = ui.id().with("room-pin");
    let focused = ui.memory(|memory| memory.has_focus(id));
    let outline = if focused {
        palette.secondary
    } else {
        palette.outline
    };
    let mut entered = false;
    Frame::new()
        .fill(palette.surface)
        .stroke((1.0, outline))
        .corner_radius(theme::RADIUS)
        .inner_margin(Margin::symmetric(16, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let font = theme::semibold(24.0);
            // The figures stand apart, as they are read: one at a time.
            let spaced = font.clone();
            let ink = palette.text;
            let mut layouter = move |ui: &Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
                let format = egui::TextFormat {
                    font_id: spaced.clone(),
                    color: ink,
                    extra_letter_spacing: 6.0,
                    ..egui::TextFormat::default()
                };
                let job = egui::text::LayoutJob::single_section(text.as_str().to_owned(), format);
                ui.painter().layout_job(job)
            };
            let mut edited = pin.to_owned();
            let hint = egui::RichText::new("0000 0000").font(font.clone());
            let edit = egui::TextEdit::singleline(&mut edited)
                .id(id)
                .hint_text(hint.color(palette.dim))
                .font(font)
                .layouter(&mut layouter)
                .frame(Frame::NONE)
                .desired_width(ui.available_width());
            let response = ui.add(edit);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Room PIN")
            });
            if response.changed() {
                actions.push(Action::TogetherField(Field::Pin, edited));
            }
            let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
            entered = response.lost_focus() && enter;
        });
    entered
}
