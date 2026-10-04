//! The line to the relay: one thread that holds the socket, says hello,
//! takes a seat in a room, and then passes commands one way and the room
//! the other. A dropped line is picked up again with the seat's token.

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};
use serde_json::{Map, Value, json};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use super::protocol::{Incoming, Mode, Room, VERSION};

/// How long a read waits before the thread looks at what is to be sent.
const POLL: Duration = Duration::from_millis(120);
/// The relay must have said hello and ready within this.
const HANDSHAKE: Duration = Duration::from_secs(10);
const PING_EVERY: Duration = Duration::from_secs(10);
/// How often a dropped line is tried again before giving up.
const MOST_ATTEMPTS: u32 = 5;

/// How to come into a room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enter {
    Create { mode: Mode, room_name: String },
    Join { pin: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub url: String,
    /// The name others see.
    pub name: String,
    pub enter: Enter,
}

/// What the line reports.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Reconnecting,
    /// The leader has yet to let this listener in.
    Waiting,
    /// A seat was taken; this is its member id.
    Joined(String),
    /// The room as it now stands, and how far the relay's clock is ahead
    /// of this computer's, in milliseconds.
    Room {
        room: Box<Room>,
        offset_ms: f64,
    },
    /// A command was refused; the room goes on.
    Refused(String),
    /// The room is over for this listener, with the relay's reason.
    Ended(String),
    /// The line could not be made, or was lost for good.
    Failed(String),
}

enum Request {
    /// A command's own fields; the envelope is added on the way out.
    Command(Map<String, Value>),
    Status {
        status: &'static str,
        entry: Option<String>,
    },
    Leave,
}

pub struct Connection {
    requests: Sender<Request>,
}

impl Connection {
    /// Connects on a thread of its own. `deliver` is called from it.
    pub fn start(options: Options, deliver: impl Fn(Event) + Send + 'static) -> io::Result<Self> {
        let (requests, inbox) = unbounded();
        std::thread::Builder::new()
            .name("listen-together".into())
            .spawn(move || run(&options, &inbox, &deliver))?;
        Ok(Self { requests })
    }

    /// Sends a room command of `kind` with `fields` of its own.
    pub fn command(&self, kind: &str, fields: Value) {
        let mut command = match fields {
            Value::Object(fields) => fields,
            _ => Map::new(),
        };
        command.insert("kind".into(), json!(kind));
        let _ = self.requests.send(Request::Command(command));
    }

    /// Tells the room what this player is doing.
    pub fn status(&self, status: &'static str, entry: Option<String>) {
        let _ = self.requests.send(Request::Status { status, entry });
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Leave);
    }
}

/// The relay's clock is milliseconds since 1970, as JavaScript counts.
pub fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64() * 1000.0)
}

/// How a spell on the line ended.
enum Over {
    /// Left, ended or refused: nothing more to do.
    Done,
    /// The line dropped and may be picked up again.
    Dropped(String),
}

/// What is kept across a dropped line.
#[derive(Default)]
struct Seat {
    /// The room's id and this seat's token, once seated.
    credential: Option<(String, String)>,
    attempts: u32,
}

fn run(options: &Options, inbox: &Receiver<Request>, deliver: &impl Fn(Event)) {
    let mut seat = Seat::default();
    loop {
        match spell(options, &mut seat, inbox, deliver) {
            Over::Done => return,
            Over::Dropped(reason) => {
                if seat.credential.is_none() || seat.attempts >= MOST_ATTEMPTS {
                    log::warn!("listen together: {reason}");
                    deliver(Event::Failed(
                        "Disconnected from the room. Check the server and join again.".into(),
                    ));
                    return;
                }
                deliver(Event::Reconnecting);
                let wait = Duration::from_millis((500u64 << seat.attempts).min(4000));
                std::thread::sleep(wait);
                seat.attempts += 1;
            }
        }
    }
}

type Socket = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

/// One connection, from hello until it ends or drops.
fn spell(
    options: &Options,
    seat: &mut Seat,
    inbox: &Receiver<Request>,
    deliver: &impl Fn(Event),
) -> Over {
    let mut socket = match open(&options.url) {
        Ok(socket) => socket,
        Err(error) if seat.credential.is_some() => return Over::Dropped(error),
        Err(error) => {
            log::warn!("listen together: {error}");
            deliver(Event::Failed(
                "Could not reach the server. Check its address.".into(),
            ));
            return Over::Done;
        }
    };
    let opened = Instant::now();
    let mut ready = false;
    let mut joined = false;
    let mut last_ping = Instant::now();
    let mut offset_ms = 0.0;
    // What a command is made against: the room as last heard.
    let (mut revision, mut current) = (0u64, None::<String>);
    loop {
        match socket.read() {
            Ok(Message::Text(text)) => {
                let Ok(message) = serde_json::from_str::<Incoming>(&text) else {
                    continue;
                };
                match message {
                    Incoming::Hello { version } if version == VERSION => {
                        send(&mut socket, &json!({ "type": "hello", "version": VERSION }));
                    }
                    Incoming::Hello { .. } => {
                        deliver(Event::Failed(
                            "This server speaks another version of Listen Together.".into(),
                        ));
                        return Over::Done;
                    }
                    Incoming::Ready => {
                        ready = true;
                        send(&mut socket, &json!({ "type": "ping", "sent": now_ms() }));
                        send(&mut socket, &entrance(options, seat));
                    }
                    Incoming::Pong { sent, at } => offset_ms = at - (sent + now_ms()) / 2.0,
                    Incoming::Joined {
                        member,
                        token,
                        room_id,
                    } => {
                        seat.credential = Some((room_id, token));
                        seat.attempts = 0;
                        joined = true;
                        deliver(Event::Joined(member));
                    }
                    Incoming::Waiting => deliver(Event::Waiting),
                    Incoming::State { room } => {
                        revision = room.revision;
                        current.clone_from(&room.current);
                        deliver(Event::Room { room, offset_ms });
                    }
                    // A refusal before a seat is taken is the end of trying.
                    Incoming::Error { message, fatal, .. } if fatal || !joined => {
                        deliver(Event::Failed(message));
                        return Over::Done;
                    }
                    Incoming::Error { message, .. } => deliver(Event::Refused(message)),
                    Incoming::Ended { reason } => {
                        deliver(Event::Ended(reason));
                        return Over::Done;
                    }
                    Incoming::Ack | Incoming::Unknown => {}
                }
            }
            Ok(Message::Close(_)) => return Over::Dropped("the server closed the line".into()),
            // Pings are answered by the library on the next write or read.
            Ok(_) => {}
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Over::Dropped(error.to_string()),
        }
        if !ready && opened.elapsed() > HANDSHAKE {
            deliver(Event::Failed(
                "The server did not answer as a Listen Together server does.".into(),
            ));
            return Over::Done;
        }
        if joined && last_ping.elapsed() >= PING_EVERY {
            last_ping = Instant::now();
            send(&mut socket, &json!({ "type": "ping", "sent": now_ms() }));
        }
        loop {
            let request = match inbox.try_recv() {
                Ok(request) => request,
                Err(TryRecvError::Empty) => break,
                // The app let go of the line: leave the room behind it.
                Err(TryRecvError::Disconnected) => Request::Leave,
            };
            match request {
                Request::Command(mut command) => {
                    command.insert("op".into(), json!(operation_id()));
                    command.insert("base".into(), json!(revision));
                    command.entry("current").or_insert(json!(current));
                    send(
                        &mut socket,
                        &json!({ "type": "command", "command": command }),
                    );
                }
                Request::Status { status, entry } => {
                    let message = json!({ "type": "status", "status": status, "entry": entry });
                    send(&mut socket, &message);
                }
                Request::Leave => {
                    send(&mut socket, &json!({ "type": "leave" }));
                    let _ = socket.close(None);
                    let _ = socket.flush();
                    return Over::Done;
                }
            }
        }
        let _ = socket.flush();
    }
}

/// Opens the socket, with reads that give up after [`POLL`] so the thread
/// can also write.
fn open(url: &str) -> Result<Socket, String> {
    let (socket, _) = tungstenite::connect(url).map_err(|error| error.to_string())?;
    let stream = match socket.get_ref() {
        MaybeTlsStream::Plain(stream) => stream,
        MaybeTlsStream::NativeTls(stream) => stream.get_ref(),
        _ => return Err("an unexpected kind of connection".into()),
    };
    stream
        .set_read_timeout(Some(POLL))
        .map_err(|error| error.to_string())?;
    Ok(socket)
}

/// How this spell comes into the room: back to its seat if it has one.
fn entrance(options: &Options, seat: &Seat) -> Value {
    if let Some((room_id, token)) = &seat.credential {
        return json!({ "type": "resume", "roomId": room_id, "token": token });
    }
    let profile = json!({ "name": options.name });
    match &options.enter {
        Enter::Create { mode, room_name } => json!({
            "type": "create",
            "mode": mode.wire(),
            "roomName": room_name,
            "profile": profile,
        }),
        Enter::Join { pin } => json!({ "type": "join", "pin": pin, "profile": profile }),
    }
}

fn send(socket: &mut Socket, message: &Value) {
    if let Err(error) = socket.write(Message::text(message.to_string())) {
        log::debug!("listen together: not sent: {error}");
    }
}

/// A name for a command that no other command of this run shares; the
/// relay uses it to tell a retry from a new command.
fn operation_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{count:x}", now_ms() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(enter: Enter) -> Options {
        Options {
            url: "ws://localhost:8766".into(),
            name: "Ada".into(),
            enter,
        }
    }

    #[test]
    fn a_new_arrival_creates_or_joins_and_a_returning_one_resumes() {
        let create = options(Enter::Create {
            mode: Mode::Listen,
            room_name: "Friday".into(),
        });
        let message = entrance(&create, &Seat::default());
        assert_eq!(message["type"], "create");
        assert_eq!(message["mode"], "listen");
        assert_eq!(message["profile"]["name"], "Ada");

        let join = options(Enter::Join {
            pin: "01234567".into(),
        });
        assert_eq!(entrance(&join, &Seat::default())["pin"], "01234567");

        let seated = Seat {
            credential: Some(("room".into(), "token".into())),
            attempts: 2,
        };
        let message = entrance(&join, &seated);
        assert_eq!(message["type"], "resume");
        assert_eq!(message["roomId"], "room");
    }

    #[test]
    fn no_two_commands_share_a_name() {
        assert_ne!(operation_id(), operation_id());
    }
}
