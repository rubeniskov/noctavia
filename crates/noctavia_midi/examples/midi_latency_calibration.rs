use anyhow::{anyhow, Result};
use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::unbounded;
use midir::MidiInput;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// MIDI input port index
    #[arg(short, long, default_value_t = 0)]
    midi_port: usize,

    /// Audio input peak threshold
    #[arg(short, long, default_value_t = 0.15)]
    threshold: f32,

    /// List available MIDI ports and exit
    #[arg(short, long)]
    list: bool,
}

enum Event {
    MidiNoteOn { timestamp: Instant },
    AudioPeak { timestamp: Instant, peak: f32 },
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("--- MIDI Latency Calibration (Timing Only) ---");
    println!("Measures the raw lead/lag between MIDI and Audio Peaks.");
    
    // 1. Setup MIDI
    let midi_in = MidiInput::new("noctavia-latency-probe")?;
    let ports = midi_in.ports();
    
    if args.list {
        println!("\nAvailable MIDI ports:");
        for (i, p) in ports.iter().enumerate() {
            println!("{}: {}", i, midi_in.port_name(p)?);
        }
        return Ok(());
    }

    if ports.is_empty() { return Err(anyhow!("No MIDI input ports found.")); }

    let port_idx = args.midi_port;
    let port = ports.get(port_idx).ok_or_else(|| anyhow!("Invalid MIDI port index"))?;
    println!("Using MIDI port {}: {}", port_idx, midi_in.port_name(port)?);

    let (event_tx, event_rx) = unbounded();
    let tx_clone = event_tx.clone();
    let _conn = midi_in.connect(port, "latency-test", move |_stamp, message, _| {
        if message.len() >= 3 && (message[0] & 0xF0) == 0x90 && message[2] > 0 {
            let _ = tx_clone.send(Event::MidiNoteOn { timestamp: Instant::now() });
        }
    }, ()).map_err(|e| anyhow!("Failed to connect: {}", e))?;

    // 2. Setup Audio
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or_else(|| anyhow!("No audio input device"))?;
    let config = device.default_input_config()?;
    let channels = config.channels() as usize;

    let threshold = args.threshold;
    let tx_clone = event_tx.clone();
    let mut last_peak_time = Instant::now();

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut local_peak = 0.0f32;
            for frame in data.chunks_exact(channels) {
                let sample = frame.iter().sum::<f32>() / channels as f32;
                if sample.abs() > local_peak {
                    local_peak = sample.abs();
                }
            }

            if local_peak > threshold {
                let now = Instant::now();
                if now.duration_since(last_peak_time) > Duration::from_millis(300) {
                    let _ = tx_clone.send(Event::AudioPeak { 
                        timestamp: now, 
                        peak: local_peak 
                    });
                    last_peak_time = now;
                }
            }
        },
        |err| eprintln!("Audio error: {}", err),
        None
    )?;

    stream.play()?;

    println!("\n--- CALIBRATION RUNNING ---\n");

    let mut last_midi: Option<Instant> = None;
    loop {
        if let Ok(event) = event_rx.recv() {
            match event {
                Event::MidiNoteOn { timestamp } => {
                    if let Some(t) = last_midi {
                        if timestamp.duration_since(t) < Duration::from_millis(200) { continue; }
                    }
                    println!("[MIDI] NoteOn received");
                    last_midi = Some(timestamp);
                }
                Event::AudioPeak { timestamp, peak } => {
                    if let Some(midi_time) = last_midi {
                        if timestamp.duration_since(midi_time) > Duration::from_secs(1) {
                            last_midi = None; continue;
                        }
                        let diff = if timestamp > midi_time { timestamp.duration_since(midi_time) } else { midi_time.duration_since(timestamp) };
                        let direction = if timestamp > midi_time { "AFTER" } else { "BEFORE" };
                        
                        println!("[AUDIO] Peak ({:.2}) - {:?} {} MIDI event", peak, diff, direction);
                        
                        let ms = diff.as_millis();
                        let rating = if timestamp > midi_time {
                            if ms > 40 { "\x1b[32mELITE (Huge Lead)\x1b[0m" }
                            else if ms > 20 { "\x1b[32mEXCELLENT\x1b[0m" }
                            else { "\x1b[33mGOOD\x1b[0m" }
                        } else { "\x1b[31mPOOR (LAG)\x1b[0m" };
                        
                        println!(">>> MIDI Lead: {}ms - {}\n", ms, rating);
                        last_midi = None;
                    }
                }
            }
        }
    }
}
