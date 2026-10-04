//! What the Listen Together page can ask for, beyond entering a room.

use spotified_client::models::Track;

use super::Tab;
use super::protocol::{Mode, Policy};

/// One of the leader's settings, changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Mode(Mode),
    Policy(Policy),
    /// The leader lets each new listener in.
    JoinApproval(bool),
    /// No one new may join.
    Locked(bool),
    /// Requests go straight into the queue.
    AutoAccept(bool),
    Duplicates(bool),
    VoteSkip(bool),
}

/// The room's own play, pause and skip buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Previous,
    Toggle,
    Next,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// Choose a saved server by its id.
    SelectServer(String),
    /// Open the form on the chosen server, or on a new one; or close it.
    ToggleManage,
    ServerName(String),
    ServerAddress(String),
    SaveServer,
    /// See whether the address in the form answers as a relay does.
    TestServer,
    /// The test ended: well, or with why not.
    Tested(Result<(), String>),
    /// Empty the form for another server.
    AddAnother,
    /// Ask whether the server in the form is really to be forgotten.
    RemoveServer,
    /// Show the account's picture to the room, or only an initial.
    SharePicture(bool),
    DismissError,

    ShowTab(Tab),
    ToggleSettings,
    /// Open the panel that asks how to leave.
    OpenLeave,
    Stay,
    /// The member to lead next; empty for whoever the relay picks.
    NextLeader(String),
    /// Leave, handing the room to the member chosen.
    Leave,
    EndRoom,
    Set(Setting),
    /// "Songs per guest" was typed in.
    LimitText(String),
    /// The caret left "Songs per guest": send it if it is a new limit.
    CommitLimit,
    RotatePin,
    /// Pause everyone and ask whether they are ready.
    ReadyCheck,
    /// Start in three seconds, ready or not.
    StartSoon,
    Ready,
    VoteSkip,
    Transport(Transport),
    /// Start the room's song here again, after it would not play.
    RetryPlayback,
    /// Make a listener a DJ, or a DJ a listener again.
    Role {
        member: String,
        dj: bool,
    },
    MakeLeader(String),
    /// Ask whether a listener is really to be removed.
    RemoveListener {
        member: String,
        name: String,
    },
    /// Let someone who is waiting in, or turn them away.
    Admit(String),
    TurnAway(String),
    /// Put requested songs in the queue: next, or at its end.
    Accept {
        requests: Vec<String>,
        next: bool,
    },
    Decline(Vec<String>),
    /// Withdraw a request of one's own.
    Withdraw(String),
    /// Add a song to the room, or ask for it where the leader approves.
    Add(Box<Track>),
    /// Play a song now, then songs like it.
    Radio(Box<Track>),
    /// Play this entry of the queue now.
    Jump(String),
    Remove(String),
    /// Take back the last edit of the queue.
    Undo,
    /// The room's search field changed.
    Search(String),
    /// Typing has paused: send the search.
    RunSearch,
    /// Make a playlist of what the room has played.
    SaveHistory,
    /// Bring this player back to the room, now.
    Resync,
    /// Say what happens in the room as it happens, or not.
    Notifications(bool),
    /// Show or hide the video when someone who steers the room does.
    FollowVideo(bool),
}
