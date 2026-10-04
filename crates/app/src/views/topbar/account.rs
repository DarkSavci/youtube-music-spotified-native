//! The account, at the right of the top bar: a chip with who is signed in,
//! which opens a menu to switch account or channel and of the pages that
//! are about them; or, signed out, a chip that signs in.

use eframe::egui::{self, CornerRadius, Sense, Ui, Vec2, pos2, vec2};

use crate::actions::{Action, account_busy};
use crate::state::{Page, State};
use crate::theme::{self, Icon};
use crate::views::widgets;
use crate::views::widgets::menu::{self, Entry, Menu};

const MENU_WIDTH: f32 = 248.0;
const AVATAR: f32 = 26.0;
const CHIP_HEIGHT: f32 = 34.0;
/// The most room the name gets in the chip.
const NAME_MOST: f32 = 130.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    // Signed in, the chip opens the menu. Signed out, the chip signs in,
    // and a button beside it opens the menu, which still leads to the
    // pages that need no account: what has been played here, the notes.
    let opener = match &state.account {
        Some(account) => chip(state, ui, &account.name),
        None => {
            sign_in(state, ui, actions);
            let figure = widgets::IconButton {
                icon: Icon::User,
                size: 18.0,
                tooltip: "Your listening and more",
                active: false,
            };
            let at = ui.allocate_space(Vec2::splat(32.0)).1.center();
            figure.show_at(ui, &state.palette, at)
        }
    };
    // Asked for without a click: opened once for each time of asking.
    let id = egui::Popup::default_response_id(&opener);
    let asked = ui.data(|data| data.get_temp::<u64>(id)).unwrap_or(0);
    if asked != state.account_menu_asks {
        ui.data_mut(|data| data.insert_temp(id, state.account_menu_asks));
        egui::Popup::open_id(ui.ctx(), id);
    }
    menu::popup(&opener, &state.palette, |list| {
        list.ui.set_width(MENU_WIDTH);
        if let Some(action) = menu(state, list) {
            actions.push(action);
        }
    });
}

/// Who is signed in: a disc with their initial, and their name.
fn chip(state: &State, ui: &mut Ui, name: &str) -> egui::Response {
    let palette = &state.palette;
    let font = theme::regular(12.0);
    let text = widgets::elided(ui, name, font, palette.secondary, NAME_MOST, 1);
    let size = vec2(8.0 + AVATAR + 8.0 + text.size().x + 10.0, CHIP_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    widgets::name(ui, &response, "Account");
    let lift = widgets::hover(ui, &response);
    if lift > 0.0 {
        let fill = palette.surface.gamma_multiply(lift);
        ui.painter()
            .rect_filled(rect, CornerRadius::same(u8::MAX), fill);
    }
    let disc = egui::Rect::from_center_size(
        pos2(rect.left() + 8.0 + AVATAR / 2.0, rect.center().y),
        Vec2::splat(AVATAR),
    );
    portrait(state, ui, disc, palette.surface_active, palette.text);
    let color = crate::tint::blend(palette.secondary, palette.text, lift);
    let at = pos2(disc.right() + 8.0, rect.center().y - text.size().y / 2.0);
    ui.painter().galley(at, text, color);
    response.on_hover_text("Account")
}

/// The chip that signs in. Its name is its own, since the sidebar offers
/// to sign in as well.
fn sign_in(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let label = if state.signing_in {
        "Finish in your browser…"
    } else {
        "Sign in"
    };
    let font = theme::medium(12.0);
    let text = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, palette.text);
    let size = vec2(text.size().x + 24.0, 26.0);
    let sense = if state.signing_in {
        Sense::hover()
    } else {
        Sense::click()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    widgets::name(ui, &response, "Sign in to YouTube Music");
    let lift = widgets::hover(ui, &response);
    let fill = crate::tint::blend(palette.surface, palette.surface_active, lift);
    ui.painter()
        .rect_filled(rect, CornerRadius::same(u8::MAX), fill);
    let at = pos2(rect.left() + 12.0, rect.center().y - text.size().y / 2.0);
    ui.painter().galley(at, text, palette.text);
    if response.clicked() {
        actions.push(Action::SignIn);
    }
}

/// The picture of whoever is signed in, or their initial; a figure until
/// they are known.
fn portrait(state: &State, ui: &Ui, rect: egui::Rect, fill: egui::Color32, ink: egui::Color32) {
    let account = state.account.as_ref();
    let avatar = widgets::Avatar {
        name: account.map_or("", |account| account.name.as_str()),
        url: account.map_or("", |account| account.avatar_url.as_str()),
        fill,
        ink,
    };
    avatar.paint(ui, state, rect);
}

/// What the account menu holds. Returns what was chosen.
fn menu(state: &State, menu: &mut Menu<'_>) -> Option<Action> {
    let palette = &state.palette;
    // Who this is: a disc, a name, and the handle or what signing in gives.
    let ui = &mut *menu.ui;
    let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());
    let disc =
        egui::Rect::from_center_size(pos2(head.left() + 28.0, head.center().y), Vec2::splat(40.0));
    let fill = palette.accent.gamma_multiply(0.2);
    portrait(state, ui, disc, fill, palette.accent);
    let (name, detail) = match &state.account {
        Some(account) if account.handle.is_empty() => (account.name.as_str(), "YouTube Music"),
        Some(account) => (account.name.as_str(), account.handle.as_str()),
        None => ("Not signed in", "Sign in for your library"),
    };
    let left = disc.right() + 12.0;
    let width = head.right() - left - 8.0;
    let title = widgets::elided(ui, name, theme::semibold(14.0), palette.text, width, 1);
    ui.painter()
        .galley(pos2(left, head.center().y - 18.0), title, palette.text);
    let font = theme::regular(12.0);
    let second = widgets::elided(ui, detail, font, palette.secondary, width, 1);
    ui.painter()
        .galley(pos2(left, head.center().y + 2.0), second, palette.secondary);
    menu.separator();

    let mut chosen = switcher(state, menu);
    menu.separator();
    let unread = state.release_notes_unread();
    let pages = [
        (Icon::Clock, "Recently played", Page::History, false),
        (Icon::AudioLines, "Your listening", Page::Stats, false),
        (Icon::Headphones, "Listen Together", Page::Together, false),
        (Icon::Gift, "Release notes", Page::Changelog, unread),
        (
            Icon::Settings,
            "Accounts and settings",
            Page::Settings,
            false,
        ),
    ];
    for (icon, label, page, marked) in pages {
        if menu.entry(Entry::new(label).icon(icon).marked(marked)) {
            chosen = Some(Action::Open(page));
        }
    }
    // Signed out, the way in is the menu's "Add Google account".
    if state.account.is_some() || state.accounts.active.is_some() {
        menu.separator();
        if menu.entry(Entry::new("Sign out").icon(Icon::LogOut)) {
            chosen = Some(Action::SignOut);
        }
    }
    chosen
}

/// The other channels of the account in use and the other saved accounts,
/// to switch to, and the way to add one more. Returns what was chosen.
fn switcher(state: &State, menu: &mut Menu<'_>) -> Option<Action> {
    let saved = &state.accounts;
    let active = saved.active();
    let free = !account_busy(state);
    // An account with one channel has nothing to choose between.
    let channels = active
        .map(|account| account.channels.as_slice())
        .filter(|channels| channels.len() > 1)
        .unwrap_or_default();
    let mut chosen = None;
    if !channels.is_empty() || saved.others().next().is_some() {
        menu.heading("Switch account or channel");
    }
    for channel in channels {
        let in_use = active.is_some_and(|account| account.channel == channel.id);
        let detail = match (channel.handle.as_str(), active) {
            ("", Some(account)) => account.name.as_str(),
            (handle, _) => handle,
        };
        // The channel in use is the one the core answers for, so its
        // picture is the signed-in account's when the list has none.
        let picture = match (&state.account, channel.avatar_url.as_str()) {
            (Some(account), "") if in_use => account.avatar_url.as_str(),
            (_, url) => url,
        };
        let who = Identity {
            name: &channel.name,
            detail,
            picture,
            in_use,
            named: &format!("Channel: {}", channel.name),
        };
        if identity(state, menu, &who, free) && !in_use {
            chosen = Some(Action::SwitchChannel(channel.id.clone()));
        }
    }
    for account in saved.others() {
        let channel = account
            .channels
            .iter()
            .find(|channel| channel.id == account.channel);
        let picture = channel
            .map(|channel| channel.avatar_url.as_str())
            .filter(|url| !url.is_empty())
            .unwrap_or(&account.avatar_url);
        let who = Identity {
            name: &account.name,
            detail: account.channel_name().unwrap_or("Google account"),
            picture,
            in_use: false,
            named: &format!("Account: {}", account.name),
        };
        if identity(state, menu, &who, free) {
            chosen = Some(Action::SwitchAccount(account.id.clone()));
        }
    }
    let add = if state.signing_in {
        "Finish in your browser…"
    } else {
        "Add Google account"
    };
    if menu.entry(Entry::new(add).icon(Icon::Plus).enabled(free)) {
        chosen = Some(Action::SignIn);
    }
    chosen
}

/// A channel or an account as the menu lists it.
struct Identity<'a> {
    name: &'a str,
    /// The line under the name, which says which it is.
    detail: &'a str,
    /// The address of its picture; empty when it has none.
    picture: &'a str,
    in_use: bool,
    named: &'a str,
}

/// A channel or an account in the menu: its picture, or a disc with its
/// initial, its name over a line that says which it is, and a mark on the
/// one in use. Returns whether it was chosen.
fn identity(state: &State, menu: &mut Menu<'_>, who: &Identity<'_>, enabled: bool) -> bool {
    let palette = &state.palette;
    let row = menu.row(46.0, who.named, enabled);
    let (rect, ui) = (row.rect, &*menu.ui);
    let fade = if enabled { 1.0 } else { 0.5 };
    let disc =
        egui::Rect::from_center_size(pos2(rect.left() + 24.0, rect.center().y), Vec2::splat(30.0));
    let ink = palette.text.gamma_multiply(fade);
    let avatar = widgets::Avatar {
        name: who.name,
        url: who.picture,
        fill: palette.surface_active,
        ink,
    };
    avatar.paint(ui, state, disc);
    let left = disc.right() + 10.0;
    let width = rect.right() - left - if who.in_use { 34.0 } else { 10.0 };
    let title = widgets::elided(ui, who.name, theme::medium(13.5), ink, width, 1);
    ui.painter()
        .galley(pos2(left, rect.center().y - 16.0), title, ink);
    let font = theme::regular(11.5);
    let quiet = palette.secondary.gamma_multiply(fade);
    let second = widgets::elided(ui, who.detail, font, quiet, width, 1);
    ui.painter()
        .galley(pos2(left, rect.center().y + 2.0), second, quiet);
    if who.in_use {
        let mark = egui::Rect::from_center_size(
            pos2(rect.right() - 18.0, rect.center().y),
            Vec2::splat(17.0),
        );
        widgets::paint_icon(ui, Icon::CircleCheck, mark, 17.0, palette.accent);
    }
    if row.chosen {
        menu.ui.close();
    }
    row.chosen
}
