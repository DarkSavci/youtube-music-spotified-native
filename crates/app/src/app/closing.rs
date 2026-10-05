//! What is done as the app closes, in the order it has to be done.

use std::time::Duration;

use super::App;
use crate::{settings, update};

/// How long the room is given to hear that this listener has left. Longer
/// and closing the app feels stuck; without it the others watch a seat
/// "reconnecting" until the relay gives up on it.
const GOODBYE: Duration = Duration::from_millis(800);

impl App {
    pub(super) fn shut_down(&mut self) {
        // What moved since the last save, such as the mini player's place.
        if let Err(error) = settings::save(&self.paths.settings_file(), &self.state.settings) {
            log::warn!("settings could not be saved: {error}");
        }
        // The room first: it is other people who are kept waiting.
        if let Some(line) = self.together.take() {
            line.close(GOODBYE);
        }
        self.milkdrop.close();
        // Stop the core here, while the log is still open to say so.
        self.session = None;
        self.backend = None;
        self.sidecar = None;
        self.install_waiting_update();
        log::info!("closed");
        log::logger().flush();
    }

    /// An update downloaded and checked is installed as the app closes,
    /// without a window and without starting the app again: nothing is
    /// interrupted, and the next start is the new version. Not when the
    /// installer is already running because the update was asked for now.
    fn install_waiting_update(&self) {
        let update::Status::Ready { installer, version } = &self.state.update else {
            return;
        };
        if self.installing || self.demo {
            return;
        }
        match update::install(installer, update::Then::StayClosed) {
            Ok(()) => log::info!("installing {version} on the way out"),
            Err(error) => log::warn!("{version} could not be installed on the way out: {error}"),
        }
    }
}
