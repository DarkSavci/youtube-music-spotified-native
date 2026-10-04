//! The room's rules as this app knows them: who may do what, and what to
//! say when the room changes. The relay decides; these mirror it, so that a
//! listener gets the answer here rather than a refused round trip for every
//! press.

use spotified_client::models::Track;

use super::protocol::{Entry, Mode, Room, SongRequest};

/// The most songs a room's queue holds.
const QUEUE_MOST: usize = 500;
/// The most radio that waits in a queue at once.
const RADIO_MOST: usize = 50;
/// Fewer songs than this after the current one, and the leader adds radio.
pub const RADIO_LOW: usize = 5;
/// How many songs a room is remembered to have had.
const HEARD_MOST: usize = 1000;

impl Room {
    pub fn leads(&self, member: &str) -> bool {
        self.owner == member
    }

    /// Whether `member` answers song requests: the leader, or a DJ.
    pub fn answers(&self, member: &str) -> bool {
        self.leads(member)
            || self
                .member(member)
                .is_some_and(|member| member.role == "dj")
    }

    /// Whether what `member` adds waits for approval instead of queueing: a
    /// guest in a room that takes requests, unless it accepts them all.
    pub fn requesting(&self, member: &str) -> bool {
        self.mode == Mode::Contributions && !self.auto_accept && !self.may_control(member)
    }

    /// Whether `member` may add songs at all: in a listen-only room only
    /// those who steer do.
    pub fn may_add(&self, member: &str) -> bool {
        self.may_control(member) || self.mode == Mode::Contributions
    }

    /// Whether `member` may take `entry` out of the queue: those who steer
    /// any but the current one, a guest only what they added themselves.
    pub fn may_remove(&self, member: &str, entry: &Entry) -> bool {
        self.current.as_deref() != Some(entry.id.as_str())
            && (self.may_control(member)
                || (self.mode == Mode::Contributions && entry.added_by.id == member))
    }

    /// Whether `member` may take back the last edit: whoever made it, or
    /// the leader, for as long as the relay would allow it.
    pub fn may_undo(&self, member: &str, server_now: f64) -> bool {
        self.undo.as_ref().is_some_and(|undo| {
            undo.revision == self.revision
                && (undo.by == member || self.leads(member))
                && server_now < undo.expires
        })
    }

    /// The queue from the current song on, with each entry's place in it.
    pub fn upcoming(&self) -> impl Iterator<Item = (usize, &Entry)> {
        let from = self.current().map_or(0, |(index, _)| index);
        self.queue.iter().enumerate().skip(from)
    }

    /// The requests `member` sees: all of them for those who answer, a
    /// guest only their own.
    pub fn requests_for<'a>(&'a self, member: &'a str) -> impl Iterator<Item = &'a SongRequest> {
        let all = self.answers(member);
        self.requests
            .iter()
            .filter(move |request| all || request.by.id == member)
    }

    /// How many of those who are connected have answered the ready check,
    /// and how many are connected.
    pub fn ready(&self) -> (usize, usize) {
        let connected = self.members.iter().filter(|member| member.connected);
        let ready = connected.clone().filter(|member| member.ready).count();
        (ready, connected.count())
    }

    /// The entries after the current one.
    fn after_current(&self) -> &[Entry] {
        let from = self.current().map_or(0, |(index, _)| index + 1);
        &self.queue[from.min(self.queue.len())..]
    }

    /// How many radio songs the room will still take. Radio is nobody's
    /// pick, so it is held to the room's size and to how much of it may
    /// wait at once, not to anyone's limit.
    pub fn radio_allowance(&self) -> usize {
        let space = QUEUE_MOST.saturating_sub(self.queue.len()).min(100);
        let waiting = self.after_current().iter().filter(|entry| entry.radio);
        space.min(RADIO_MOST.saturating_sub(waiting.count()))
    }

    /// How many songs wait after the current one.
    pub fn left(&self) -> usize {
        self.after_current().len()
    }

    /// The entry after the current one, which "play next" goes before.
    pub fn next_entry(&self) -> Option<&Entry> {
        self.after_current().first()
    }
}

/// Notes the songs the room has had, played or jumped past, most recent
/// last. The relay refuses radio that repeats them; leaving them out here
/// as well keeps a radio batch from shrinking to nothing once it has.
pub fn remember_heard(heard: &mut Vec<String>, room: &Room) {
    let played = room.current().map_or(0, |(index, _)| index + 1);
    let recent = room.history.len().saturating_sub(50);
    let had = room.history[recent..]
        .iter()
        .chain(&room.queue[..played.min(room.queue.len())]);
    for entry in had {
        heard.retain(|id| *id != entry.track.id);
        heard.push(entry.track.id.clone());
    }
    let over = heard.len().saturating_sub(HEARD_MOST);
    heard.drain(..over);
}

/// The radio songs worth adding after `seed`: playable, not the seed, and
/// nothing the room has or has had.
pub fn fresh_radio(found: Vec<Track>, seed: &str, room: &Room, heard: &[String]) -> Vec<Track> {
    let mut seen: Vec<&str> = room
        .queue
        .iter()
        .chain(&room.history)
        .map(|entry| entry.track.id.as_str())
        .chain(heard.iter().map(String::as_str))
        .chain([seed])
        .collect();
    let mut fresh: Vec<Track> = Vec::new();
    for track in &found {
        if track.playable && !seen.contains(&track.id.as_str()) {
            seen.push(&track.id);
            fresh.push(track.clone());
        }
    }
    fresh.truncate(room.radio_allowance());
    fresh
}

/// What to tell this listener about song requests when the room goes from
/// `previous` to `room`: the leader about new ones, a guest about theirs
/// being sent and what became of them. `withdrawn` are requests this
/// listener cancelled, whose going is not news.
pub fn request_news(
    previous: &Room,
    room: &Room,
    me: &str,
    withdrawn: &mut Vec<String>,
) -> Vec<String> {
    let mut news = Vec::new();
    let was = |id: &str| previous.requests.iter().any(|request| request.id == id);
    let is = |id: &str| room.requests.iter().any(|request| request.id == id);
    let arrived: Vec<&SongRequest> = room
        .requests
        .iter()
        .filter(|request| !was(&request.id))
        .collect();
    let others: Vec<&&SongRequest> = arrived.iter().filter(|r| r.by.id != me).collect();
    if room.answers(me) {
        match others[..] {
            [] => {}
            [one] => news.push(format!("{} requested “{}”.", one.by.name, one.track.title)),
            _ => news.push(format!("{} new song requests.", others.len())),
        }
    }
    match arrived.len() - others.len() {
        0 => {}
        1 => news.push("Request sent. The leader decides what plays.".to_owned()),
        sent => news.push(format!(
            "{sent} requests sent. The leader decides what plays."
        )),
    }
    // Accept all, or a change of mode, answers several at once: they are
    // told in one sentence.
    let (mut added, mut refused) = (Vec::new(), Vec::new());
    for request in &previous.requests {
        if request.by.id != me || is(&request.id) {
            continue;
        }
        if let Some(at) = withdrawn.iter().position(|id| *id == request.id) {
            withdrawn.remove(at);
            continue;
        }
        let accepted = room
            .queue
            .iter()
            .chain(&room.history)
            .any(|entry| entry.request.as_deref() == Some(request.id.as_str()));
        if accepted {
            added.push(request.track.title.as_str());
        } else {
            refused.push(request.track.title.as_str());
        }
    }
    news.extend(request_outcome(&added, &refused));
    news
}

fn request_outcome(added: &[&str], refused: &[&str]) -> Option<String> {
    Some(match (added, refused) {
        ([], []) => return None,
        ([one], []) => format!("“{one}” was added to the queue."),
        (_, []) => format!("{} of your requests were added to the queue.", added.len()),
        ([], [one]) => format!("Your request for “{one}” wasn’t added."),
        ([], _) => format!("{} of your requests weren’t added.", refused.len()),
        _ => format!(
            "{} of your requests {} added to the queue, {} {}.",
            added.len(),
            if added.len() == 1 { "was" } else { "were" },
            refused.len(),
            if refused.len() == 1 {
                "wasn’t"
            } else {
                "weren’t"
            },
        ),
    })
}

/// Whether the room was taken somewhere else in the same song between two
/// states: someone sought, which a player follows at once rather than as
/// drift it may wait to correct.
pub fn jumped(previous: &Room, room: &Room) -> bool {
    if previous.current != room.current {
        return false;
    }
    let run = if previous.playing {
        (room.at - previous.at).max(0.0)
    } else {
        0.0
    };
    (previous.position_ms + run - room.position_ms).abs() > 1000.0
}

#[cfg(test)]
mod tests;
