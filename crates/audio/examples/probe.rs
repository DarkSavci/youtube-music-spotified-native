//! Says what the decoder makes of each file: its format, and the length it
//! states. A check on cached streams whose length the player cannot show.
//!
//!     cargo run -p spotified-audio --example probe -- a.audio b.audio

use std::fs::File;

use spotified_audio::decode::Decoder;

fn main() {
    for path in std::env::args().skip(1) {
        let opened = File::open(&path)
            .map_err(|error| error.to_string())
            .and_then(|file| Decoder::open(Box::new(file)).map_err(|error| error.to_string()));
        match opened {
            Ok(decoder) => println!(
                "{path}: {:?}, length {:?} ms",
                decoder.format(),
                decoder.duration_ms()
            ),
            Err(error) => println!("{path}: {error}"),
        }
    }
}
