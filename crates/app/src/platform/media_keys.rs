//! The system's media controls: the keyboard's media keys, and on Windows
//! the now-playing card with its title, artist and cover.

use std::ffi::c_void;
use std::time::{Duration, Instant};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use spotified_client::models::artwork_url;

use crate::actions::Action;
use crate::state::Playback;

/// The size of cover the system's card is given.
const COVER_PIXELS: u32 = 300;

/// What the controls were last told, so they are only told again on a
/// change: each update crosses into the system.
#[derive(PartialEq, Eq)]
struct Shown {
    track_id: String,
    playing: bool,
}

pub struct MediaKeys {
    controls: MediaControls,
    shown: Option<Shown>,
}

impl MediaKeys {
    /// Attaches to the window. `on_action` is called from the system's
    /// thread. `None` if the system offers no controls; the app works
    /// without them.
    pub fn attach(
        window: &impl HasWindowHandle,
        on_action: impl Fn(Action) + Send + 'static,
    ) -> Option<Self> {
        let hwnd = match window.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as *mut c_void),
            _ => None,
        };
        let config = PlatformConfig {
            display_name: crate::APP_NAME,
            dbus_name: "spotified",
            hwnd,
        };
        let mut controls = MediaControls::new(config)
            .inspect_err(|error| log::warn!("no media controls: {error:?}"))
            .ok()?;
        controls
            .attach(move |event| {
                if let Some(action) = action_for(event) {
                    on_action(action);
                }
            })
            .inspect_err(|error| log::warn!("media controls would not attach: {error:?}"))
            .ok()?;
        Some(Self {
            controls,
            shown: None,
        })
    }

    /// Tells the system what is playing, if that has changed.
    pub fn show(&mut self, playback: Option<&Playback>) {
        let now = playback.and_then(|playback| {
            playback.current().map(|track| Shown {
                track_id: track.id.clone(),
                playing: playback.wants_to_play(),
            })
        });
        if now == self.shown {
            return;
        }
        // Errors here are the system declining an update; the next change
        // tries again.
        match playback.and_then(|playback| Some((playback, playback.current()?))) {
            Some((playback, track)) => {
                let artists = track.artist_names();
                let cover = artwork_url(&track.artwork, COVER_PIXELS);
                let _ = self.controls.set_metadata(MediaMetadata {
                    title: Some(&track.title),
                    artist: Some(&artists),
                    album: track.album.as_ref().map(|album| album.name.as_str()),
                    cover_url: cover.as_deref(),
                    duration: (track.duration_ms > 0)
                        .then(|| Duration::from_millis(track.duration_ms)),
                });
                let progress = Some(MediaPosition(Duration::from_millis(
                    playback.position_ms(Instant::now()),
                )));
                let _ = self.controls.set_playback(if playback.wants_to_play() {
                    MediaPlayback::Playing { progress }
                } else {
                    MediaPlayback::Paused { progress }
                });
            }
            None => {
                let _ = self.controls.set_playback(MediaPlayback::Stopped);
            }
        }
        self.shown = now;
    }
}

fn action_for(event: MediaControlEvent) -> Option<Action> {
    Some(match event {
        MediaControlEvent::Play => Action::SetPlaying(true),
        MediaControlEvent::Pause | MediaControlEvent::Stop => Action::SetPlaying(false),
        MediaControlEvent::Toggle => Action::TogglePlay,
        MediaControlEvent::Next => Action::Next,
        MediaControlEvent::Previous => Action::Previous,
        MediaControlEvent::SetPosition(MediaPosition(position)) => {
            Action::Seek(position.as_millis() as u64)
        }
        _ => return None,
    })
}
