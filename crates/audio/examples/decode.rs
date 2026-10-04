//! Decodes a file to a 16-bit WAV, optionally starting from a position.
//!
//! The check that the decoder handles a real stream: play the result.
//!
//!     cargo run -p spotified-audio --example decode -- in.webm out.wav [seconds]

use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::time::Instant;

use spotified_audio::decode::{Decoder, Format};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, output, rest @ ..] = args.as_slice() else {
        return Err("usage: decode <input> <output.wav> [start seconds]".into());
    };
    let start_ms = match rest.first() {
        Some(seconds) => (seconds.parse::<f64>()? * 1000.0) as u64,
        None => 0,
    };

    let began = Instant::now();
    let mut decoder = Decoder::open(Box::new(File::open(input)?))?;
    let format = decoder.format();
    println!(
        "{} Hz, {} channels, duration {:?} ms",
        format.sample_rate,
        format.channels,
        decoder.duration_ms()
    );
    if start_ms > 0 {
        decoder.seek(start_ms)?;
    }

    let mut wav = BufWriter::new(File::create(output)?);
    write_header(&mut wav, format, 0)?;
    let mut samples = 0u64;
    let mut first_position = None;
    while let Some(chunk) = decoder.next_chunk()? {
        first_position.get_or_insert(chunk.position_ms);
        for sample in chunk.samples {
            let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
            wav.write_all(&value.to_le_bytes())?;
        }
        samples += chunk.samples.len() as u64;
    }
    wav.seek(SeekFrom::Start(0))?;
    write_header(&mut wav, format, samples * 2)?;

    let seconds = samples as f64 / format.channels as f64 / f64::from(format.sample_rate);
    println!(
        "first chunk at {:?} ms; decoded {seconds:.2} s of audio in {:.2} s",
        first_position,
        began.elapsed().as_secs_f64()
    );
    Ok(())
}

fn write_header(out: &mut impl Write, format: Format, data_bytes: u64) -> io::Result<()> {
    let channels = format.channels as u16;
    let block_align = channels * 2;
    let data_bytes = data_bytes as u32;
    out.write_all(b"RIFF")?;
    out.write_all(&(36 + data_bytes).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&channels.to_le_bytes())?;
    out.write_all(&format.sample_rate.to_le_bytes())?;
    out.write_all(&(format.sample_rate * u32::from(block_align)).to_le_bytes())?;
    out.write_all(&block_align.to_le_bytes())?;
    out.write_all(&16u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&data_bytes.to_le_bytes())
}
