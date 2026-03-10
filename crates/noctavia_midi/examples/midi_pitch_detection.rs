use anyhow::{anyhow, Result};
use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::unbounded;
use midir::MidiInput;
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::{Arc, Mutex};
use wide::f32x8;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// MIDI input port index (optional, for comparison)
    #[arg(short, long)]
    midi_port: Option<usize>,

    /// Audio input peak threshold
    #[arg(short, long, default_value_t = 0.10)]
    threshold: f32,

    /// List available MIDI ports and exit
    #[arg(short, long)]
    list: bool,

    /// Max number of simultaneous notes to detect
    #[arg(short = 'n', long, default_value_t = 6)]
    max_notes: usize,
}

enum Event {
    MidiNoteOn { key: u8 },
    MidiNoteOff { key: u8 },
    AudioResult { 
        detected_notes: Vec<(u8, f32)>, // (key, hz)
    },
}

struct PitchDetectionState {
    is_collecting: bool,
    buffer: Vec<f32>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("--- Polyphonic MIDI Pitch Detection Test ---");
    
    // 1. Setup MIDI (Optional)
    let midi_in = MidiInput::new("noctavia-pitch-probe")?;
    let ports = midi_in.ports();

    if args.list {
        println!("\nAvailable MIDI ports:");
        for (i, p) in ports.iter().enumerate() {
            println!("{}: {}", i, midi_in.port_name(p)?);
        }
        return Ok(());
    }

    let (event_tx, event_rx) = unbounded();

    let _conn = if let Some(port_idx) = args.midi_port {
        let port = ports.get(port_idx).ok_or_else(|| anyhow!("Invalid MIDI port index: {}", port_idx))?;
        println!("Using MIDI port {}: {} for comparison", port_idx, midi_in.port_name(port)?);
        
        let tx_clone = event_tx.clone();
        let conn = midi_in.connect(port, "pitch-test", move |_stamp, message, _| {
            if message.len() >= 3 {
                let status = message[0] & 0xF0;
                let key = message[1];
                let vel = message[2];
                if status == 0x90 && vel > 0 {
                    let _ = tx_clone.send(Event::MidiNoteOn { key });
                } else if status == 0x80 || (status == 0x90 && vel == 0) {
                    let _ = tx_clone.send(Event::MidiNoteOff { key });
                }
            }
        }, ()).map_err(|e| anyhow!("Failed to connect: {}", e))?;
        Some(conn)
    } else {
        println!("Running in Audio-only mode");
        None
    };

    // 2. Setup Audio
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or_else(|| anyhow!("No audio input device"))?;
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;

    const FFT_SIZE: usize = 8192;
    let state = Arc::new(Mutex::new(PitchDetectionState {
        is_collecting: false,
        buffer: Vec::with_capacity(FFT_SIZE),
    }));

    let threshold = args.threshold;
    let tx_clone = event_tx.clone();
    let state_clone = state.clone();
    let max_notes = args.max_notes;

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut s = state_clone.lock().unwrap();
            for frame in data.chunks_exact(channels) {
                let sample = frame.iter().sum::<f32>() / channels as f32;
                if !s.is_collecting {
                    if sample.abs() > threshold {
                        s.is_collecting = true;
                        s.buffer.clear();
                    }
                } else {
                    s.buffer.push(sample);
                    if s.buffer.len() >= FFT_SIZE {
                        let notes = detect_pitches(&s.buffer, sample_rate, max_notes);
                        let _ = tx_clone.send(Event::AudioResult {
                            detected_notes: notes,
                        });
                        s.is_collecting = false;
                    }
                }
            }
        },
        |err| eprintln!("Audio error: {}", err),
        None
    )?;

    stream.play()?;

    println!("Listening... Exit with Ctrl+C\n");

    let mut active_midi_notes = std::collections::HashSet::new();

    loop {
        if let Ok(event) = event_rx.recv() {
            match event {
                Event::MidiNoteOn { key } => {
                    active_midi_notes.insert(key);
                    println!("[MIDI] Note ON:  {} ({:.1}Hz)", note_name(key), midi_to_hz(key));
                }
                Event::MidiNoteOff { key } => {
                    active_midi_notes.remove(&key);
                    println!("[MIDI] Note OFF: {}", note_name(key));
                }
                Event::AudioResult { detected_notes } => {
                    if detected_notes.is_empty() {
                        println!("[AUDIO] Silence or low confidence");
                        continue;
                    }

                    print!("[AUDIO] Chords: ");
                    for (dk, hz) in &detected_notes {
                        let mut match_str = "";
                        if active_midi_notes.contains(dk) {
                            match_str = " ✅";
                        } else if active_midi_notes.iter().any(|&mk| (mk as i32 - *dk as i32).abs() == 12) {
                            match_str = " 🆗";
                        }

                        print!("{} ({:.1}Hz){}  ", note_name(*dk), hz, match_str);
                    }
                    
                    // Check for missing MIDI notes
                    let mut missing = Vec::new();
                    for &mk in &active_midi_notes {
                        if !detected_notes.iter().any(|(dk, _)| *dk == mk) {
                            missing.push(note_name(mk));
                        }
                    }
                    if !missing.is_empty() {
                        print!("| Missing: \x1b[31m{:?}\x1b[0m", missing);
                    }
                    println!();
                }
            }
        }
    }
}

fn detect_pitches(samples: &[f32], sample_rate: f32, max_notes: usize) -> Vec<(u8, f32)> {
    let n = samples.len();
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);
    let mut buffer = vec![Complex::new(0.0, 0.0); n];
    let pi2_n = 2.0 * std::f32::consts::PI / (n as f32 - 1.0);
    
    // SIMD Hamming Window
    let chunks = n / 8;
    for c in 0..chunks {
        let i_base = c * 8;
        let mut idx_arr = [0.0f32; 8];
        for j in 0..8 { idx_arr[j] = (i_base + j) as f32; }
        let i_simd = f32x8::from(idx_arr);
        let cos_val = (i_simd * pi2_n).cos();
        let win_simd = f32x8::splat(0.54) - f32x8::splat(0.46) * cos_val;
        let mut s_arr = [0.0f32; 8];
        s_arr.copy_from_slice(&samples[i_base..i_base+8]);
        let s_simd = f32x8::from(s_arr);
        let res_simd = s_simd * win_simd;
        let res_arr: [f32; 8] = res_simd.into();
        for j in 0..8 { buffer[i_base + j] = Complex::new(res_arr[j], 0.0); }
    }
    for i in (chunks * 8)..n {
        let window = 0.54 - 0.46 * (pi2_n * i as f32).cos();
        buffer[i] = Complex::new(samples[i] * window, 0.0);
    }

    fft.process(&mut buffer);

    // SIMD Magnitudes
    let mut magnitudes = vec![0.0f32; n / 2];
    let mag_chunks = (n / 2) / 8;
    for c in 0..mag_chunks {
        let i_base = c * 8;
        let mut re = [0.0f32; 8];
        let mut im = [0.0f32; 8];
        for j in 0..8 { re[j] = buffer[i_base + j].re; im[j] = buffer[i_base + j].im; }
        let re_simd = f32x8::from(re);
        let im_simd = f32x8::from(im);
        let mag_sq = re_simd * re_simd + im_simd * im_simd;
        let mag_simd = mag_sq.sqrt();
        let res_arr: [f32; 8] = mag_simd.into();
        magnitudes[i_base..i_base+8].copy_from_slice(&res_arr);
    }
    for i in (mag_chunks * 8)..(n / 2) { magnitudes[i] = buffer[i].norm(); }

    // Harmonic Product Spectrum
    let mut hps = magnitudes.clone();
    for h in 2..=3 {
        for i in 0..(n / 2 / h) { hps[i] *= magnitudes[i * h]; }
    }

    // Find multiple peaks
    let min_bin = (30.0 * n as f32 / sample_rate) as usize;
    let max_bin = (3000.0 * n as f32 / sample_rate) as usize;
    let end = std::cmp::min(max_bin, hps.len() - 2);

    let mut peaks = Vec::new();
    for i in min_bin..end {
        if hps[i] > hps[i-1] && hps[i] > hps[i+1] {
            peaks.push((i, hps[i]));
        }
    }

    // Sort by HPS magnitude and take top N
    peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    let mut detected = Vec::new();
    let global_max = peaks.first().map(|p| p.1).unwrap_or(0.0);
    
    for (idx, val) in peaks.into_iter().take(max_notes * 2) {
        // Only consider peaks within 5% of the global max to filter noise
        if val < global_max * 0.05 { continue; }

        // Parabolic interpolation for better frequency
        let alpha = magnitudes[idx - 1];
        let beta = magnitudes[idx];
        let gamma = magnitudes[idx + 1];
        let denom = alpha - 2.0 * beta + gamma;
        let p = if denom.abs() > 1e-9 { 0.5 * (alpha - gamma) / denom } else { 0.0 };
        let freq = (idx as f32 + p) * sample_rate / n as f32;
        let note = (69.0 + 12.0 * (freq / 440.0).log2()).round() as u8;

        // Dedup notes (don't add same note twice if multiple bins point to it)
        if !detected.iter().any(|(n, _)| *n == note) {
            detected.push((note, freq));
        }
        
        if detected.len() >= max_notes { break; }
    }

    detected
}

fn midi_to_hz(midi_key: u8) -> f32 { 440.0 * 2.0f32.powf((midi_key as f32 - 69.0) / 12.0) }
fn note_name(midi_key: u8) -> String {
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", names[(midi_key % 12) as usize], (midi_key / 12) as i32 - 1)
}
