use anyhow::{anyhow, Result};
use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::{Arc, Mutex};
use wide::f32x8;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Audio input peak threshold
    #[arg(short, long, default_value_t = 0.10)]
    threshold: f32,
}

struct PitchDetectionState {
    is_collecting: bool,
    buffer: Vec<f32>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("--- Pitch Detection Test (SIMD + HPS) ---");
    println!("Listens to audio and detects the musical note.");
    
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
    let state_clone = state.clone();

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
                        let (key, hz) = detect_pitch(&s.buffer, sample_rate);
                        if let (Some(k), Some(h)) = (key, hz) {
                            println!("[DETECTED] Note: {} ({:.1}Hz)", note_name(k), h);
                        } else {
                            println!("[DETECTED] ??? (Low confidence)");
                        }
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
    loop { std::thread::sleep(std::time::Duration::from_millis(100)); }
}

fn detect_pitch(samples: &[f32], sample_rate: f32) -> (Option<u8>, Option<f32>) {
    let n = samples.len();
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);
    let mut buffer = vec![Complex::new(0.0, 0.0); n];
    let pi2_n = 2.0 * std::f32::consts::PI / (n as f32 - 1.0);
    
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

    let mut hps = magnitudes.clone();
    for h in 2..=3 {
        for i in 0..(n / 2 / h) { hps[i] *= magnitudes[i * h]; }
    }

    let min_bin = (30.0 * n as f32 / sample_rate) as usize;
    let max_bin = (3000.0 * n as f32 / sample_rate) as usize;
    let mut max_index = 0;
    let mut max_val = 0.0;
    for i in min_bin..std::cmp::min(max_bin, hps.len() - 1) {
        if hps[i] > max_val { max_val = hps[i]; max_index = i; }
    }
    if max_val < 0.0000001 || max_index < 1 { return (None, None); }
    let alpha = magnitudes[max_index - 1];
    let beta = magnitudes[max_index];
    let gamma = magnitudes[max_index + 1];
    let denom = alpha - 2.0 * beta + gamma;
    let p = if denom.abs() > 1e-9 { 0.5 * (alpha - gamma) / denom } else { 0.0 };
    let freq = (max_index as f32 + p) * sample_rate / n as f32;
    let note = 69.0 + 12.0 * (freq / 440.0).log2();
    (Some(note.round() as u8), Some(freq))
}

fn note_name(midi_key: u8) -> String {
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", names[(midi_key % 12) as usize], (midi_key / 12) as i32 - 1)
}
