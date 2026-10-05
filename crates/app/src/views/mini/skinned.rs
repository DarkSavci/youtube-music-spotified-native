//! The mini player in a Winamp skin.
//!
//! The window is the skin's own: 275 by 116 of its pixels, each drawn as a
//! whole number of screen pixels so they stay sharp, with the equalizer and
//! the playlist hanging under it when they are open. Every control asks
//! for what the app's own mini player asks for. Where Winamp had something
//! this player has not, the control says what it does instead: Stop pauses
//! and rewinds, and Eject and the logo bring the main window forward.
//!
//! The drawing follows Spotifast's `src/ui/winamp` (MIT, see NOTICE.md).

mod display;
mod equalizer;
mod main;
mod modern;
mod playlist;

use eframe::egui::{
    self, Color32, Id, Pos2, Rect, Response, Sense, Ui, Vec2, ViewportCommand, pos2, vec2,
};

use crate::actions::Action;
use crate::skin::layout::{self, Area};
use crate::skin::{Skin, Sprite, font};
use crate::skins::{self, Ask, Worn};
use crate::state::{Playback, State};

/// How many screen pixels each skin pixel is drawn as on a display of this
/// density: the size chosen, in whole pixels.
pub fn pixels(settings: &crate::settings::Settings, pixels_per_point: f32) -> f32 {
    let scale = f32::from(settings.skin_scale.clamp(1, 4));
    (scale * pixels_per_point).round().max(1.0)
}

/// Points per skin pixel.
pub fn unit(settings: &crate::settings::Settings, pixels_per_point: f32) -> f32 {
    pixels(settings, pixels_per_point) / pixels_per_point
}

/// The stack's height in skin pixels: the main window, and the equalizer
/// and the playlist under it, whichever are open.
pub fn stack_height(settings: &crate::settings::Settings) -> u32 {
    let mut height = if settings.skin_shaded {
        layout::SHADE_HEIGHT
    } else {
        layout::WINDOW_HEIGHT
    };
    if settings.skin_equalizer {
        height += if settings.skin_equalizer_shaded {
            layout::EQ_SHADE_HEIGHT
        } else {
            layout::EQ_HEIGHT
        };
    }
    if settings.skin_playlist {
        height += playlist_rows(settings);
    }
    height
}

/// How many skin pixels tall the playlist is, as it stands.
fn playlist_rows(settings: &crate::settings::Settings) -> u32 {
    if settings.skin_playlist_shaded {
        layout::PLAYLIST_SHADE_HEIGHT
    } else {
        layout::playlist_height(settings.skin_playlist_height)
    }
}

/// A slider position as a balance, -1 to 1, snapping to the middle the way
/// Winamp's did.
fn balance_of(fraction: f32) -> f32 {
    let balance = fraction * 2.0 - 1.0;
    if balance.abs() < 0.08 { 0.0 } else { balance }
}

/// The window's size in points.
pub fn size(settings: &crate::settings::Settings, pixels_per_point: f32) -> Vec2 {
    vec2(layout::WINDOW_WIDTH as f32, stack_height(settings) as f32)
        * unit(settings, pixels_per_point)
}

/// The window's shape, for a skin that is not a rectangle: the rows of its
/// windows that show, as boxes of left, top, right and bottom in skin
/// pixels. `None` when the whole rectangle shows, which is most skins.
pub fn shape(settings: &crate::settings::Settings, skin: &Skin) -> Option<Vec<[u32; 4]>> {
    let regions = &skin.regions;
    let mut parts = Vec::new();
    if settings.skin_shaded {
        parts.push((layout::SHADE_HEIGHT, regions.shade.as_ref()));
    } else {
        parts.push((layout::WINDOW_HEIGHT, regions.normal.as_ref()));
    }
    if settings.skin_equalizer && settings.skin_equalizer_shaded {
        parts.push((layout::EQ_SHADE_HEIGHT, regions.equalizer_shade.as_ref()));
    } else if settings.skin_equalizer {
        parts.push((layout::EQ_HEIGHT, regions.equalizer.as_ref()));
    }
    if settings.skin_playlist {
        parts.push((playlist_rows(settings), None));
    }
    if parts.iter().all(|(_, mask)| mask.is_none()) {
        return None;
    }
    let mut boxes = Vec::new();
    let mut top = 0;
    for (height, mask) in parts {
        match mask {
            None => boxes.push([0, top, layout::WINDOW_WIDTH, top + height]),
            Some(mask) => {
                for row in 0..height {
                    let spans = mask.spans(row).iter();
                    boxes.extend(spans.map(|(from, to)| [*from, top + row, *to, top + row + 1]));
                }
            }
        }
        top += height;
    }
    Some(boxes)
}

/// What a slider did this frame, as a fraction of its travel.
#[derive(Clone, Copy, PartialEq)]
enum Slid {
    No,
    /// Held, and here.
    Dragging(f32),
    /// Let go, or clicked, here.
    Released(f32),
}

/// Draws the skin's sprites into the window and reads the pointer against
/// the skin's layout.
struct View<'a> {
    ui: &'a mut Ui,
    origin: Pos2,
    unit: f32,
    worn: &'a Worn,
}

impl View<'_> {
    fn skin(&self) -> &Skin {
        &self.worn.skin
    }

    /// The same window, with its top `rows` skin pixels further down: for
    /// the windows that hang under the main one.
    fn below(&mut self, rows: u32) -> View<'_> {
        View {
            origin: self.origin + vec2(0.0, rows as f32 * self.unit),
            ui: &mut *self.ui,
            unit: self.unit,
            worn: self.worn,
        }
    }

    fn rect(&self, area: Area) -> Rect {
        Rect::from_min_size(
            self.origin + vec2(area.x as f32, area.y as f32) * self.unit,
            vec2(area.width as f32, area.height as f32) * self.unit,
        )
    }

    /// A pointer position in skin pixels.
    fn skin_pos(&self, pos: Pos2) -> Vec2 {
        (pos - self.origin) / self.unit
    }

    fn paint(&self, painter: &egui::Painter, sprite: Sprite, x: u32, y: u32) {
        let Some((bitmap, clipped)) = self.skin().sprite(sprite) else {
            return;
        };
        let Some(texture) = self.worn.texture(sprite.sheet) else {
            return;
        };
        let (width, height) = (bitmap.width as f32, bitmap.height as f32);
        let uv = Rect::from_min_max(
            pos2(clipped.x as f32 / width, clipped.y as f32 / height),
            pos2(
                (clipped.x + clipped.width) as f32 / width,
                (clipped.y + clipped.height) as f32 / height,
            ),
        );
        let dest = self.rect(Area::new(x, y, clipped.width, clipped.height));
        painter.image(texture, dest, uv, Color32::WHITE);
    }

    fn sprite_at(&self, sprite: Sprite, x: u32, y: u32) {
        self.paint(self.ui.painter(), sprite, x, y);
    }

    fn sprite(&self, sprite: Sprite, area: Area) {
        self.sprite_at(sprite, area.x, area.y);
    }

    /// A sprite cut to an area, for tiles that run past the edge.
    fn sprite_clipped(&self, sprite: Sprite, x: u32, y: u32, clip: Area) {
        let clip = self.rect(clip).intersect(self.ui.clip_rect());
        let painter = self.ui.painter().with_clip_rect(clip);
        self.paint(&painter, sprite, x, y);
    }

    /// A block of skin pixels in one colour.
    fn fill(&self, area: Area, color: Color32) {
        self.ui.painter().rect_filled(self.rect(area), 0.0, color);
    }

    /// A line of the skin's bitmap font, cut off at the area's edge.
    fn text(&self, text: &str, area: Area) {
        let clip = self.rect(area).intersect(self.ui.clip_rect());
        let painter = self.ui.painter().with_clip_rect(clip);
        for (index, character) in text.chars().enumerate() {
            let x = area.x + 5 * index as u32;
            if x >= area.x + area.width {
                break;
            }
            self.paint(&painter, font::glyph(character), x, area.y);
        }
    }

    /// A control with nothing of its own to draw. `name` is what it is
    /// called, by a screen reader as by the tests.
    fn interact(&mut self, area: Area, name: &str, sense: Sense) -> Response {
        let rect = self.rect(area);
        let response = self.ui.interact(rect, Id::new(("skin", name)), sense);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
        response
    }

    /// A button drawn pressed while the pointer holds it down.
    fn button(&mut self, area: Area, normal: Sprite, pressed: Sprite, name: &str) -> Response {
        let response = self.interact(area, name, Sense::click());
        let sprite = if response.is_pointer_button_down_on() {
            pressed
        } else {
            normal
        };
        self.sprite(sprite, area);
        response
    }

    /// A button whose only sprite is its lit state, drawn over the
    /// background while it is on or held.
    fn lamp_button(&mut self, area: Area, lit: Sprite, on: bool, name: &str) -> Response {
        let response = self.interact(area, name, Sense::click());
        if on || response.is_pointer_button_down_on() {
            self.sprite(lit, area);
        }
        response
    }

    /// A slider along an area, its thumb `thumb` pixels wide: where the
    /// pointer has it, as a fraction of the thumb's travel.
    fn slider(&mut self, area: Area, name: &str, thumb: u32) -> (Response, Slid) {
        let response = self.interact(area, name, Sense::click_and_drag());
        let travel = (area.width - thumb) as f32;
        let pointer = response.interact_pointer_pos().map(|pos| {
            let along = self.skin_pos(pos).x - area.x as f32 - thumb as f32 / 2.0;
            (along / travel).clamp(0.0, 1.0)
        });
        let memory = Id::new(("skin-slider", name));
        let held = self.ui.data(|data| data.get_temp::<f32>(memory));
        let mut slid = Slid::No;
        if (response.drag_started() || response.dragged())
            && let Some(value) = pointer
        {
            self.ui.data_mut(|data| data.insert_temp(memory, value));
            slid = Slid::Dragging(value);
        }
        if response.drag_stopped() {
            if let Some(value) = held.or(pointer) {
                slid = Slid::Released(value);
            }
            self.ui.data_mut(|data| data.remove::<f32>(memory));
        } else if response.clicked()
            && let Some(value) = pointer
        {
            slid = Slid::Released(value);
        }
        (response, slid)
    }

    /// Where a slider is being held, if it is.
    fn held(&self, name: &str) -> Option<f32> {
        let memory = Id::new(("skin-slider", name));
        self.ui.data(|data| data.get_temp::<f32>(memory))
    }

    /// Makes a title bar move the window, roll it up on a double click,
    /// and bring the options on a right click.
    fn title_bar(&mut self, area: Area, name: &str) -> Response {
        let title = self.interact(area, name, Sense::click_and_drag());
        if title.drag_started() {
            self.ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        title
    }
}

/// The size of the window a skin is worn in, in points.
pub fn window_size(
    worn: &Worn,
    settings: &crate::settings::Settings,
    pixels_per_point: f32,
) -> Vec2 {
    match &worn.modern {
        Some(modern) => modern::size(&modern.skin),
        None => size(settings, pixels_per_point),
    }
}

/// The shape of the window a skin is worn in, in the screen's own pixels:
/// the boxes of it that show, or `None` for its whole rectangle.
pub fn window_shape(
    worn: &Worn,
    settings: &crate::settings::Settings,
    pixels_per_point: f32,
) -> Option<Vec<[i32; 4]>> {
    // A classic skin's pixels are whole screen pixels; a modern one's are
    // points, which need not be.
    let (boxes, scale) = match &worn.modern {
        Some(modern) => (modern.skin.shape.clone()?, pixels_per_point),
        None => (
            shape(settings, &worn.skin)?,
            pixels(settings, pixels_per_point),
        ),
    };
    let scaled = |[left, top, right, bottom]: [u32; 4]| {
        let near = |side: u32| (side as f32 * scale).floor() as i32;
        let far = |side: u32| (side as f32 * scale).ceil() as i32;
        [near(left), near(top), far(right), far(bottom)]
    };
    Some(boxes.into_iter().map(scaled).collect())
}

/// The whole window: the main one, and whichever of the others are open.
pub fn show(state: &State, worn: &Worn, ui: &mut Ui, actions: &mut Vec<Action>) {
    if let Some(modern) = &worn.modern {
        return modern::show(state, worn, modern, ui, actions);
    }
    let ctx = ui.ctx().clone();
    let settings = &state.settings;
    let unit = unit(settings, ctx.pixels_per_point());
    let focused = ctx.input(|input| input.viewport().focused.unwrap_or(true));
    let playback = state
        .playback
        .as_ref()
        .filter(|playback| playback.current().is_some());
    let mut view = View {
        origin: ui.max_rect().min,
        ui,
        unit,
        worn,
    };
    let shaded = settings.skin_shaded;
    let moving = if shaded {
        main::shade_bar(state, &mut view, actions, focused, playback);
        false
    } else {
        main::full_window(state, &mut view, actions, focused, playback)
    };
    // The other windows hang under this one in Winamp's order.
    let mut rows = if shaded {
        layout::SHADE_HEIGHT
    } else {
        layout::WINDOW_HEIGHT
    };
    if settings.skin_equalizer {
        equalizer::show(state, &mut view.below(rows), actions, focused, playback);
        rows += if settings.skin_equalizer_shaded {
            layout::EQ_SHADE_HEIGHT
        } else {
            layout::EQ_HEIGHT
        };
    }
    if settings.skin_playlist {
        playlist::show(state, &mut view.below(rows), actions, focused, playback);
    }
    // The analyser moves with the sound; the time and the marquee by the
    // clock, which is slower.
    if moving {
        ctx.request_repaint_after(display::ANALYSER_FRAME);
    } else if playback.is_some() {
        ctx.request_repaint_after(display::MARQUEE_STEP);
    }
}

/// The time something has got to and its length, in milliseconds, with a
/// seek that is being dragged shown where the drag is.
fn times(view: &View<'_>, playback: &Playback) -> (u64, u64) {
    let duration = playback.current().map_or(0, |track| track.duration_ms);
    let position = playback.position_ms(std::time::Instant::now());
    let held = view.held("Seek").or_else(|| view.held("Seek, rolled up"));
    match held {
        Some(fraction) => ((fraction * duration as f32) as u64, duration),
        None => (position, duration),
    }
}

/// Whether the player is stopped, as Winamp meant it: something loaded and
/// paused at the very start. There is no stop here, so this is what Stop
/// leaves behind, and what the display treats as stopped.
fn stopped(playback: Option<&Playback>) -> bool {
    playback.is_some_and(|playback| !playback.wants_to_play() && playback.session.position_ms == 0)
}

/// Winamp's Stop: pause, and back to the start.
fn stop(actions: &mut Vec<Action>, playback: Option<&Playback>) {
    if playback.is_none() {
        return;
    }
    actions.push(Action::SetPlaying(false));
    actions.push(Action::Seek(0));
}

/// Winamp's Play: on a song that is already playing, from the top.
fn play(actions: &mut Vec<Action>, playback: Option<&Playback>) {
    if playback.is_some_and(Playback::wants_to_play) {
        actions.push(Action::Seek(0));
    } else {
        actions.push(Action::SetPlaying(true));
    }
}

/// The type size of a menu at this scale.
fn menu_font(unit: f32) -> f32 {
    (5.0 * unit).clamp(10.0, 14.0)
}

/// A menu for the skinned player: the skin's playlist colours on a square
/// frame, type that follows the scale, and never taller than the window,
/// since a menu cannot leave the window it belongs to. A long list scrolls
/// inside it. Winamp used the system's menus and a skin says nothing about
/// them, so the playlist's colours are the nearest thing it says about
/// text on a background.
fn menu(popup: egui::Popup<'_>, skin: &Skin, unit: f32, contents: impl FnOnce(&mut Ui)) {
    let rgb = |[r, g, b]: [u8; 3]| Color32::from_rgb(r, g, b);
    let text = rgb(skin.playlist.normal);
    let current = rgb(skin.playlist.current);
    let background = rgb(skin.playlist.normal_background);
    let selected = rgb(skin.playlist.selected_background);
    let font = menu_font(unit);
    let style = move |style: &mut egui::Style| {
        for text_style in [egui::TextStyle::Body, egui::TextStyle::Button] {
            let size = egui::FontId::proportional(font);
            style.text_styles.insert(text_style, size);
        }
        style.spacing.item_spacing = vec2(4.0, 1.0);
        style.spacing.button_padding = vec2(6.0, 1.0);
        // A row is its text and padding, not egui's 18 points.
        style.spacing.interact_size = vec2(font * 2.0, font + 2.0);
        style.spacing.menu_margin = egui::Margin::same(2);
        let visuals = &mut style.visuals;
        visuals.window_fill = background;
        visuals.panel_fill = background;
        visuals.window_stroke = egui::Stroke::new(1.0, text.gamma_multiply(0.5));
        visuals.window_corner_radius = egui::CornerRadius::ZERO;
        visuals.menu_corner_radius = egui::CornerRadius::ZERO;
        visuals.window_shadow = egui::Shadow::NONE;
        visuals.popup_shadow = egui::Shadow::NONE;
        visuals.override_text_color = None;
        visuals.selection.bg_fill = selected;
        visuals.selection.stroke = egui::Stroke::new(1.0, current);
        let widgets = &mut visuals.widgets;
        for state in [&mut widgets.noninteractive, &mut widgets.inactive] {
            state.fg_stroke.color = text;
            state.weak_bg_fill = background;
            state.bg_fill = background;
            state.bg_stroke = egui::Stroke::NONE;
        }
        for state in [&mut widgets.hovered, &mut widgets.active, &mut widgets.open] {
            state.fg_stroke.color = current;
            state.weak_bg_fill = selected;
            state.bg_fill = selected;
            state.bg_stroke = egui::Stroke::NONE;
            state.expansion = 0.0;
            state.corner_radius = egui::CornerRadius::ZERO;
        }
    };
    popup.style(style).show(|ui| {
        let frame = ui.spacing().menu_margin.sum().y + 6.0;
        let most = (ui.ctx().content_rect().height() - frame).max(font);
        egui::ScrollArea::vertical()
            .max_height(most)
            .show(ui, contents);
    });
}

/// The menu behind a right click on the title bar and the O of the clutter
/// bar: the skins, the size, and the ways out.
fn options_menu(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, unit: f32) {
    let settings = &state.settings;
    ui.set_min_width(menu_font(unit) * 11.0);
    ui.horizontal(|ui| {
        ui.label("Size");
        for scale in skins::SCALES {
            let chosen = settings.skin_scale == scale;
            if ui.selectable_label(chosen, format!("{scale}x")).clicked() {
                actions.push(Action::Skin(Ask::Scale(scale)));
            }
        }
    });
    let mut on_top = settings.mini_on_top;
    if ui.checkbox(&mut on_top, "Always on top").clicked() {
        actions.push(Action::SetMiniOnTop(on_top));
    }
    let mut milkdrop = state.milkdrop.open;
    if ui.checkbox(&mut milkdrop, "MilkDrop").clicked() {
        actions.push(Action::MilkDrop(crate::milkdrop::Ask::Toggle));
    }
    ui.menu_button("Skin", |ui| {
        let worn = settings.mini_skin.as_deref();
        egui::ScrollArea::vertical()
            .max_height((ui.ctx().content_rect().height() - 12.0).max(menu_font(unit)))
            .show(ui, |ui| {
                if ui.selectable_label(false, "None").clicked() {
                    actions.push(Action::Skin(Ask::Wear(None)));
                }
                let built_in = std::iter::once(skins::BUILT_IN);
                let installed = state.skins.iter().map(|skin| skin.file.as_str());
                for file in built_in.chain(installed) {
                    let chosen = worn == Some(file);
                    let label = skins::label(file);
                    if ui.selectable_label(chosen, label).clicked() && !chosen {
                        actions.push(Action::Skin(Ask::Wear(Some(file.to_owned()))));
                    }
                }
            });
    });
    if ui.button("Add a skin\u{2026}").clicked() {
        actions.push(Action::Skin(Ask::Pick));
    }
    if ui.button("Get more skins").clicked() {
        actions.push(Action::Skin(Ask::OpenMuseum));
    }
    if ui.button("Open app").clicked() {
        actions.push(Action::ShowMainWindow);
    }
    if ui.button("Close mini player").clicked() {
        actions.push(Action::ToggleMiniPlayer);
    }
}
