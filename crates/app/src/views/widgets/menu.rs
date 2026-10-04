//! Menus: the one that a right click brings, and every other list of
//! things to choose from that opens over the page.
//!
//! They are the Electron app's (`ContextMenu.tsx` and its `.ctxmenu`
//! rules): a card with a hairline and a deep shadow, rows of an icon and a
//! label, one highlight shared by the pointer and the arrow keys, a rule
//! above an entry that begins a new group, and a chevron on an entry that
//! opens a further menu. Where a menu opens, that it stays inside the
//! window and that a click elsewhere shuts it are egui's; how it looks and
//! how the keyboard moves through it are here, so no menu differs.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Frame, Id, Key, Margin, Modifiers, Rect, Response, Sense,
    Ui, containers::menu::MenuState, containers::menu::SubMenu, pos2, vec2,
};

use super::{elided, hover_of, paint_icon, text_at};
use crate::theme::{self, Icon, Palette};

/// The narrowest and the widest a menu is, as the Electron app's.
const MIN_WIDTH: f32 = 220.0;
const MAX_WIDTH: f32 = 340.0;
/// The room between the card's edge and its rows.
const PADDING: i8 = 4;
/// A row: eight points over and under an eighteen point icon.
pub const ROW_HEIGHT: f32 = 34.0;
const ROW_PADDING: f32 = 12.0;
const ICON: f32 = 18.0;
const GAP: f32 = 12.0;
const CHEVRON: f32 = 16.0;
const FONT: f32 = 12.5;
/// What is left of the window above and below the tallest menu.
const WINDOW_MARGIN: f32 = 16.0;

/// The card a menu is drawn on.
pub fn frame(palette: &Palette) -> Frame {
    Frame::new()
        .fill(palette.overlay)
        .stroke((1.0, palette.outline))
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::same(PADDING))
        .shadow(egui::epaint::Shadow {
            offset: [0, 16],
            blur: 40,
            spread: 0,
            color: palette.shadow,
        })
}

/// One entry of a menu.
#[derive(Clone, Copy)]
pub struct Entry<'a> {
    label: &'a str,
    icon: Option<Icon>,
    named: Option<&'a str>,
    danger: bool,
    enabled: bool,
    checked: bool,
    marked: bool,
    plain: bool,
}

impl<'a> Entry<'a> {
    /// An entry with the icon its label calls for.
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            named: None,
            danger: false,
            enabled: true,
            checked: false,
            marked: false,
            plain: false,
        }
    }

    /// One of a list of plain choices, as a drop-down's: no icon.
    pub fn plain(label: &'a str) -> Self {
        Self {
            plain: true,
            ..Self::new(label)
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// What a screen reader calls it, where the label alone would be
    /// said twice on one screen.
    pub fn named(mut self, name: &'a str) -> Self {
        self.named = Some(name);
        self
    }

    /// Something that cannot be taken back: drawn in the danger colour.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The one of several that is in force: ticked at its right.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// A dot at its right: something new is behind it.
    pub fn marked(mut self, marked: bool) -> Self {
        self.marked = marked;
        self
    }
}

/// The icon an entry gets from what it says, as the Electron app chose
/// them: a menu's entries are told apart by their icons before they are
/// read.
pub fn icon_for(label: &str) -> Icon {
    let label = label.to_lowercase();
    let has = |word: &str| label.contains(word);
    if has("delete") || label.starts_with("remove") {
        Icon::Trash
    } else if has("radio") {
        Icon::Radio
    } else if has("album") {
        Icon::Disc
    } else if has("folder") || label.starts_with("move to") {
        Icon::Folder
    } else if has("queue") {
        Icon::ListMusic
    } else if label.starts_with("play") {
        Icon::PlayFilled
    } else if has("share") || has("copy") {
        Icon::Share
    } else if has("pin") {
        Icon::Pin
    } else if has("save") || has("liked") {
        Icon::Heart
    } else if label.starts_with("go to") {
        Icon::User
    } else if has("playlist") || label.starts_with("create") {
        Icon::Plus
    } else {
        Icon::SquareLibrary
    }
}

/// Which row the highlight is on, kept between frames.
#[derive(Clone, Copy, Default)]
struct Keys {
    active: Option<usize>,
    /// The pass it was last shown on: a menu not shown on the pass before
    /// has just been opened.
    shown: u64,
}

/// What a row turned out to be this frame.
pub struct Row {
    pub rect: Rect,
    pub response: Response,
    /// How far the highlight has come in, from 0 to 1.
    pub lit: f32,
    pub chosen: bool,
}

/// A menu being filled in.
pub struct Menu<'u> {
    pub ui: &'u mut Ui,
    palette: Palette,
    id: Id,
    /// The menu this one was opened from, which Left goes back to.
    parent: Option<Id>,
    keys: Keys,
    /// The keyboard is this menu's: no menu is open beyond it.
    owner: bool,
    /// Each row so far: whether it can be chosen, and the menu it opens.
    rows: Vec<(bool, Option<Id>)>,
    separated: bool,
}

/// Hangs a menu on the control that opens it with a click.
pub fn popup(opener: &Response, palette: &Palette, body: impl FnOnce(&mut Menu<'_>)) {
    show(egui::Popup::menu(opener).gap(4.0), palette, body);
}

/// The menu a right click on `target` brings, at the pointer.
pub fn context(target: &Response, palette: &Palette, body: impl FnOnce(&mut Menu<'_>)) {
    show(egui::Popup::context_menu(target), palette, body);
}

/// A menu in a popup made by the caller, for one opened in some way of
/// its own.
pub fn show(popup: egui::Popup<'_>, palette: &Palette, body: impl FnOnce(&mut Menu<'_>)) {
    let id = popup.get_id();
    let palette = *palette;
    popup
        // A choice shuts the menu itself; a click on a rule or a note
        // does not.
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(frame(&palette))
        .style(move |style: &mut egui::Style| style_for(style, &palette))
        .show(|ui| contents(ui, &palette, id, None, body));
}

/// A menu's rows, in a list that scrolls when the window is too short
/// for it.
fn contents(
    ui: &mut Ui,
    palette: &Palette,
    id: Id,
    parent: Option<Id>,
    body: impl FnOnce(&mut Menu<'_>),
) {
    let most = (ui.ctx().content_rect().height() - WINDOW_MARGIN * 2.0).max(ROW_HEIGHT * 3.0);
    egui::ScrollArea::vertical()
        .max_height(most)
        // As tall as its rows until the window runs out, whatever room
        // the popup thought it had.
        .min_scrolled_height(most)
        .show(ui, |ui| {
            let inner = f32::from(PADDING) * 2.0 + 2.0;
            ui.set_min_width(MIN_WIDTH - inner);
            ui.set_max_width(MAX_WIDTH - inner);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let mut menu = Menu::new(ui, palette, id, parent);
            body(&mut menu);
            menu.finish();
        });
}

/// A further menu takes its card from the style, so the style carries it.
fn style_for(style: &mut egui::Style, palette: &Palette) {
    egui::containers::menu::menu_style(style);
    let card = frame(palette);
    style.spacing.menu_margin = card.inner_margin;
    style.visuals.menu_corner_radius = card.corner_radius;
    style.visuals.popup_shadow = card.shadow;
    style.visuals.window_fill = card.fill;
    style.visuals.window_stroke = card.stroke;
}

/// Whether a menu is open: the keyboard is the menu's while one is, and
/// no shortcut of the app's answers.
pub fn is_open(ctx: &egui::Context) -> bool {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|data| data.get_temp::<u64>(open_id()))
        .is_some_and(|shown| shown + 1 >= pass)
}

/// Opens the further menu `sub` of the menu `id`, as the keyboard does.
fn open_beyond(ctx: &egui::Context, id: Id, sub: Id) {
    // A menu egui has not seen shown is taken to have gone, and is shut
    // again before it is drawn: it is told this one is on its way.
    MenuState::mark_shown(ctx, sub);
    MenuState::from_id(ctx, id, |state| state.open_item = Some(sub));
}

fn open_id() -> Id {
    Id::new("menu-open-pass")
}

impl<'u> Menu<'u> {
    fn new(ui: &'u mut Ui, palette: &Palette, id: Id, parent: Option<Id>) -> Self {
        let pass = ui.ctx().cumulative_pass_nr();
        let mut keys = ui
            .data(|data| data.get_temp::<Keys>(id.with("keys")))
            .unwrap_or_default();
        let fresh = keys.shown + 1 < pass;
        if fresh {
            // The first row is lit as it opens, as the Electron app's menu
            // gave its first entry the focus.
            keys.active = Some(0);
            if parent.is_none() {
                // The arrows are the menu's, not a field's that had the caret.
                ui.memory_mut(|memory| memory.stop_text_input());
            }
        }
        keys.shown = pass;
        let owner = MenuState::from_id(ui.ctx(), id, |state| state.open_item).is_none();
        Self {
            ui,
            palette: *palette,
            id,
            parent,
            keys,
            owner,
            rows: Vec::new(),
            separated: false,
        }
    }

    /// An entry with the icon its label calls for. Returns whether it was
    /// chosen.
    pub fn item(&mut self, label: &str) -> bool {
        self.entry(Entry::new(label))
    }

    /// Returns whether the entry was chosen. The menu shuts on a choice.
    pub fn entry(&mut self, entry: Entry<'_>) -> bool {
        let row = self.labelled(entry, None);
        if row.chosen {
            self.ui.close();
        }
        row.chosen
    }

    /// An entry that opens a further menu beside it.
    pub fn submenu(&mut self, entry: Entry<'_>, body: impl FnOnce(&mut Menu<'_>)) {
        // The row's id is the next one the menu hands out, which is how the
        // further menu is known before the row is drawn.
        self.rule();
        let sub = SubMenu::id_from_widget_id(self.ui.next_auto_id());
        let row = self.labelled(entry, Some(sub));
        if !entry.enabled {
            return;
        }
        if row.chosen && !row.response.clicked() {
            // Opened with the keyboard, which then goes on into it.
            open_beyond(self.ui.ctx(), self.id, sub);
        }
        let (palette, parent) = (self.palette, self.id);
        SubMenu::new().show(self.ui, &row.response, |ui| {
            contents(ui, &palette, sub, Some(parent), body);
        });
    }

    /// A rule above the next entry, which begins a new group. Nothing is
    /// drawn at the top of a menu, or twice.
    pub fn separator(&mut self) {
        self.separated = self.ui.min_rect().height() > 0.5;
    }

    /// A line that says something and cannot be chosen: what a group is,
    /// or why it is empty.
    pub fn note(&mut self, text: &str) {
        self.rule();
        let width = self.ui.available_width();
        let (rect, _) = self
            .ui
            .allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::hover());
        let color = self.palette.secondary;
        let font = theme::regular(FONT);
        let galley = elided(self.ui, text, font, color, width - ROW_PADDING * 2.0, 1);
        let at = pos2(
            rect.left() + ROW_PADDING,
            rect.center().y - galley.size().y / 2.0,
        );
        self.ui.painter().galley(at, galley, color);
    }

    /// A row drawn by the caller, `height` tall, that takes the highlight
    /// and the keyboard as an entry does.
    pub fn row(&mut self, height: f32, name: &str, enabled: bool) -> Row {
        self.rule();
        let width = self.ui.available_width();
        self.place(vec2(width, height), name, enabled, None)
    }

    /// Draws the rule a separator asked for.
    fn rule(&mut self) {
        if !std::mem::take(&mut self.separated) {
            return;
        }
        let width = self.ui.available_width();
        let (rect, _) = self
            .ui
            .allocate_exact_size(vec2(width, 9.0), Sense::hover());
        self.ui
            .painter()
            .hline(rect.x_range(), rect.center().y, (1.0, self.palette.outline));
    }

    fn labelled(&mut self, entry: Entry<'_>, sub: Option<Id>) -> Row {
        self.rule();
        let palette = self.palette;
        let font = theme::regular(FONT);
        let trailing = if sub.is_some() || entry.checked || entry.marked {
            GAP + CHEVRON
        } else {
            0.0
        };
        let leading = if entry.plain { 0.0 } else { ICON + GAP };
        let fixed = ROW_PADDING + leading + trailing + ROW_PADDING;
        let wanted = self
            .ui
            .painter()
            .layout_no_wrap(entry.label.to_owned(), font.clone(), palette.text)
            .size()
            .x
            + fixed;
        let room = self.ui.available_width();
        let most = MAX_WIDTH - f32::from(PADDING) * 2.0 - 2.0;
        let width = if room.is_finite() {
            wanted.max(room).min(most.max(room))
        } else {
            wanted.min(most)
        };
        let name = entry.named.unwrap_or(entry.label);
        let row = self.place(vec2(width, ROW_HEIGHT), name, entry.enabled, sub);
        if !self.ui.is_rect_visible(row.rect) {
            return row;
        }
        let rect = row.rect;
        let (text, quiet) = match (entry.enabled, entry.danger) {
            (false, _) => (palette.dim, palette.dim),
            (true, true) => (palette.danger, palette.danger),
            (true, false) => (palette.text, palette.secondary),
        };
        let middle = rect.center().y;
        // Share copies a link, and says so while it is the one lit.
        let icon = match entry.icon.unwrap_or_else(|| icon_for(entry.label)) {
            Icon::Share if row.lit > 0.5 => Icon::Copy,
            icon => icon,
        };
        let at = Rect::from_center_size(
            pos2(rect.left() + ROW_PADDING + ICON / 2.0, middle),
            vec2(ICON, ICON),
        );
        if !entry.plain {
            paint_icon(self.ui, icon, at, ICON, quiet);
        }
        let galley = elided(self.ui, entry.label, font, text, rect.width() - fixed, 1);
        let left = rect.left() + ROW_PADDING + leading;
        let at = pos2(left, middle - galley.size().y / 2.0);
        self.ui.painter().galley(at, galley, text);
        let end = Rect::from_center_size(
            pos2(rect.right() - ROW_PADDING - CHEVRON / 2.0, middle),
            vec2(CHEVRON, CHEVRON),
        );
        if sub.is_some() {
            paint_icon(self.ui, Icon::ChevronRight, end, CHEVRON, quiet);
        } else if entry.checked {
            paint_icon(self.ui, Icon::Check, end, CHEVRON, palette.accent);
        } else if entry.marked {
            self.ui
                .painter()
                .circle_filled(end.center(), 4.0, palette.accent);
        }
        row
    }

    /// Claims a row's place, and works out whether it is lit and whether
    /// it was chosen.
    fn place(&mut self, size: egui::Vec2, name: &str, enabled: bool, sub: Option<Id>) -> Row {
        let index = self.rows.len();
        self.rows.push((enabled, sub));
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = self.ui.allocate_exact_size(size, sense);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
        if enabled && response.hovered() {
            self.ui
                .ctx()
                .set_cursor_icon(egui::CursorIcon::PointingHand);
            // The pointer takes the highlight when it moves, not by lying
            // where the menu happened to open.
            if self.ui.input(|input| input.pointer.is_moving()) {
                self.keys.active = Some(index);
            }
        }
        let open_beyond = sub.is_some()
            && MenuState::from_id(self.ui.ctx(), self.id, |state| state.open_item) == sub;
        let active = enabled && self.keys.active == Some(index);
        let lit = hover_of(self.ui, response.id, active || open_beyond);
        if lit > 0.0 && self.ui.is_rect_visible(rect) {
            self.ui
                .painter()
                .rect_filled(rect, CornerRadius::same(4), self.highlight(lit));
        }
        let pressed = active
            && self.owner
            && self.ui.input_mut(|input| {
                input.consume_key(Modifiers::NONE, Key::Enter)
                    || input.consume_key(Modifiers::NONE, Key::Space)
            });
        Row {
            rect,
            chosen: enabled && (response.clicked() || pressed),
            response,
            lit,
        }
    }

    /// The highlight: the control surface, as the Electron app's, which is
    /// darker than the card on a dark theme and greyer on a light one. A
    /// theme whose two are one colour gets the wash rows take elsewhere.
    fn highlight(&self, lit: f32) -> Color32 {
        let palette = &self.palette;
        if palette.surface == palette.overlay {
            super::wash(self.ui, lit)
        } else {
            palette.surface.gamma_multiply(lit)
        }
    }

    /// The arrow keys, once every row is known.
    fn finish(mut self) {
        let ctx = self.ui.ctx().clone();
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|data| data.insert_temp(open_id(), pass));
        let first = self.rows.iter().position(|(enabled, _)| *enabled);
        // A highlight left on a row that has gone, or cannot be chosen,
        // moves to the first that can.
        let valid = |active: usize| self.rows.get(active).is_some_and(|(enabled, _)| *enabled);
        if !self.keys.active.is_some_and(valid) {
            self.keys.active = first;
        }
        if self.owner {
            self.navigate(&ctx);
        }
        ctx.data_mut(|data| data.insert_temp(self.id.with("keys"), self.keys));
    }

    fn navigate(&mut self, ctx: &egui::Context) {
        let pressed = |key| ctx.input_mut(|input| input.consume_key(Modifiers::NONE, key));
        let count = self.rows.len();
        let step = i32::from(pressed(Key::ArrowDown)) - i32::from(pressed(Key::ArrowUp));
        if step != 0 && count > 0 {
            let from = self
                .keys
                .active
                .unwrap_or(if step > 0 { count - 1 } else { 0 });
            let next = (1..=count)
                .map(|n| (from as i32 + step * n as i32).rem_euclid(count as i32) as usize)
                .find(|&row| self.rows[row].0);
            if next.is_some() {
                self.keys.active = next;
                ctx.request_repaint();
            }
        }
        if pressed(Key::Home) {
            self.keys.active = self.rows.iter().position(|(enabled, _)| *enabled);
        }
        if pressed(Key::End) {
            self.keys.active = self.rows.iter().rposition(|(enabled, _)| *enabled);
        }
        if pressed(Key::ArrowRight)
            && let Some((true, Some(sub))) = self.keys.active.and_then(|row| self.rows.get(row))
        {
            open_beyond(ctx, self.id, *sub);
        }
        // Back one menu; from the first, Escape is left for the popup,
        // which shuts on it.
        if let Some(parent) = self.parent
            && (pressed(Key::ArrowLeft) || pressed(Key::Escape))
        {
            MenuState::from_id(ctx, parent, |state| state.open_item = None);
        }
        if pressed(Key::Tab) {
            self.ui.close();
        }
    }

    /// A heading over a group of rows: quiet and semibold.
    pub fn heading(&mut self, text: &str) {
        self.rule();
        let width = self.ui.available_width();
        let (rect, _) = self
            .ui
            .allocate_exact_size(vec2(width, 28.0), Sense::hover());
        let at = pos2(rect.left() + ROW_PADDING, rect.center().y);
        let font = theme::semibold(11.5);
        let color = self.palette.secondary;
        text_at(self.ui, at, Align2::LEFT_CENTER, text, font, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_is_given_the_icon_the_old_menu_gave_it() {
        let cases = [
            ("Add to queue", Icon::ListMusic),
            ("Play next", Icon::PlayFilled),
            ("Add to playlist", Icon::Plus),
            ("Remove from this playlist", Icon::Trash),
            ("Go to song radio", Icon::Radio),
            ("Go to Daft Punk", Icon::User),
            ("Go to album", Icon::Disc),
            ("Save to your library", Icon::Heart),
            ("Share", Icon::Share),
            ("Pin to top", Icon::Pin),
            ("Move to folder", Icon::Folder),
            ("Delete playlist", Icon::Trash),
            ("Create a playlist", Icon::Plus),
            ("Something else", Icon::SquareLibrary),
        ];
        for (label, icon) in cases {
            assert_eq!(icon_for(label), icon, "{label}");
        }
    }

    #[test]
    fn deleting_is_told_before_what_is_deleted() {
        // "Delete folder" names a folder, and is still a deletion.
        assert_eq!(icon_for("Delete folder"), Icon::Trash);
        assert_eq!(icon_for("Remove from folder"), Icon::Trash);
    }
}
