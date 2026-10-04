//! The player against a core that is not there, and one that serves
//! something other than a film: both must end as a failure that is said,
//! with nothing left running and nothing brought down.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::*;

/// Waits for the player to say something other than that it is loading.
fn outcome(player: &Player) -> Status {
    let begun = Instant::now();
    while begun.elapsed() < Duration::from_secs(20) {
        let status = player.status();
        if status != Status::Loading {
            return status;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Status::Loading
}

#[test]
fn a_core_that_does_not_answer_is_a_failure_that_is_said() {
    // Bound and dropped: nothing listens there now.
    let port = TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .expect("a free port");
    let woken = Arc::new(AtomicUsize::new(0));
    let wake = {
        let woken = woken.clone();
        move || {
            woken.fetch_add(1, Ordering::Relaxed);
        }
    };
    let address = format!("http://127.0.0.1:{port}/v1/video-stream/abcdefghijk");
    let player = Player::start(address, wake);
    assert!(matches!(outcome(&player), Status::Failed(_)));
    // The window was told, so the cover and the way to retry get drawn.
    assert!(woken.load(Ordering::Relaxed) >= 1);
    assert!(player.take(0).is_none());
}

#[test]
fn a_stream_that_is_not_a_film_is_a_failure_that_is_said() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("an address").port();
    // Answers every request with a page of text, as an error page would be.
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            let body = "<html>Video could not be loaded.</html>".repeat(40);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(body.as_bytes());
        }
    });
    let address = format!("http://127.0.0.1:{port}/v1/video-stream/abcdefghijk");
    let player = Player::start(address, || {});
    match outcome(&player) {
        Status::Failed(reason) => assert!(reason.contains("not a film"), "{reason}"),
        other => panic!("ended as {other:?}"),
    }
}
