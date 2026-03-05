use std::sync::{Arc, Mutex};
use std::time::Duration;
use rodio::Source;
use rustysynth::{Synthesizer, SynthesizerSettings, SoundFont};
use anyhow::Result;
use std::io::Read;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetInfo {
    pub name: String,
    pub bank: i32,
    pub patch: i32,
}

impl std::fmt::Display for PresetInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Clone)]
pub struct MidiSynth {
    synth: Arc<Mutex<Synthesizer>>,
    sample_rate: u32,
    presets: Vec<PresetInfo>,
}

impl std::fmt::Debug for MidiSynth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiSynth")
            .field("presets_count", &self.presets.len())
            .finish()
    }
}

impl MidiSynth {
    pub fn new_with_sf2<R: Read>(sample_rate: u32, mut sf2_data: R) -> Result<Self> {
        let sf = SoundFont::new(&mut sf2_data).map_err(|e| anyhow::anyhow!("Failed to load SoundFont: {:?}", e))?;
        
        let presets = sf.get_presets().iter().map(|p| PresetInfo {
            name: p.get_name().to_string(),
            bank: p.get_bank_number(),
            patch: p.get_patch_number(),
        }).collect();

        let sf_arc = Arc::new(sf);
        let mut settings = SynthesizerSettings::new(sample_rate as i32);
        settings.enable_reverb_and_chorus = true;
        
        let synth = Synthesizer::new(&sf_arc, &settings).map_err(|e| anyhow::anyhow!("Failed to create Synthesizer: {:?}", e))?;
        
        Ok(Self {
            synth: Arc::new(Mutex::new(synth)),
            sample_rate,
            presets,
        })
    }

    pub fn note_on(&self, channel: u8, key: u8, velocity: u8) {
        if let Ok(mut synth) = self.synth.lock() {
            synth.note_on(channel as i32, key as i32, velocity as i32);
        }
    }

    pub fn note_off(&self, channel: u8, key: u8) {
        if let Ok(mut synth) = self.synth.lock() {
            synth.note_off(channel as i32, key as i32);
        }
    }

    pub fn control_change(&self, channel: u8, controller: u8, value: u8) {
        if let Ok(mut synth) = self.synth.lock() {
            synth.process_midi_message(channel as i32, 0xB0, controller as i32, value as i32);
        }
    }

    pub fn program_change(&self, channel: u8, program: u8) {
        if let Ok(mut synth) = self.synth.lock() {
            // Program Change command is 0xC0
            synth.process_midi_message(channel as i32, 0xC0, program as i32, 0);
        }
    }
    
    pub fn set_preset(&self, channel: u8, bank: i32, patch: i32) {
        if let Ok(mut synth) = self.synth.lock() {
            // Bank Select MSB is CC 0
            synth.process_midi_message(channel as i32, 0xB0, 0, bank);
            // Program Change is 0xC0
            synth.process_midi_message(channel as i32, 0xC0, patch, 0);
        }
    }

    pub fn set_master_volume(&self, volume: f32) {
        if let Ok(mut synth) = self.synth.lock() {
            synth.set_master_volume(volume);
        }
    }

    pub fn all_notes_off(&self) {
        if let Ok(mut synth) = self.synth.lock() {
            synth.note_off_all(false);
        }
    }

    pub fn get_presets(&self) -> &[PresetInfo] {
        &self.presets
    }

    pub fn get_source(&self) -> SynthSource {
        SynthSource {
            synth: self.synth.clone(),
            sample_rate: self.sample_rate,
            left_buffer: Vec::new(),
            right_buffer: Vec::new(),
            buffer_pos: 0,
        }
    }
}

pub struct SynthSource {
    synth: Arc<Mutex<Synthesizer>>,
    sample_rate: u32,
    left_buffer: Vec<f32>,
    right_buffer: Vec<f32>,
    buffer_pos: usize,
}

impl Iterator for SynthSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buffer_pos >= self.left_buffer.len() * 2 {
            self.fill_buffer();
        }

        let res = if self.buffer_pos % 2 == 0 {
            self.left_buffer.get(self.buffer_pos / 2).cloned()
        } else {
            self.right_buffer.get(self.buffer_pos / 2).cloned()
        };
        self.buffer_pos += 1;
        res.or(Some(0.0))
    }
}

impl SynthSource {
    fn fill_buffer(&mut self) {
        let block_size = 64; 
        self.left_buffer.resize(block_size, 0.0);
        self.right_buffer.resize(block_size, 0.0);
        
        if let Ok(mut synth) = self.synth.lock() {
            synth.render(&mut self.left_buffer, &mut self.right_buffer);
        }
        
        self.buffer_pos = 0;
    }
}

impl Source for SynthSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
