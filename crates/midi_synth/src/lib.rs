use std::sync::{Arc, Mutex};
use std::time::Duration;
use rodio::Source;
use rustysynth::{Synthesizer, SynthesizerSettings, SoundFont as RustySoundFont};
use anyhow::Result;
use std::io::Read;

#[cfg(feature = "xsynth")]
use xsynth_core::{
    channel_group::{ChannelGroup, ChannelGroupConfig, SynthEvent, SynthFormat, ParallelismOptions, ThreadCount}, 
    channel::{ChannelEvent, ChannelAudioEvent, ChannelConfigEvent, ChannelInitOptions, ControlEvent},
    soundfont::{SampleSoundfont, SoundfontInitOptions, EnvelopeOptions, Interpolator, EnvelopeCurveType},
    AudioPipe, AudioStreamParams, ChannelCount,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SynthBackend {
    RustySynth,
    #[cfg(feature = "xsynth")]
    XSynth,
}

impl std::fmt::Display for SynthBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SynthBackend::RustySynth => write!(f, "RustySynth"),
            #[cfg(feature = "xsynth")]
            SynthBackend::XSynth => write!(f, "XSynth"),
        }
    }
}

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

enum Engine {
    Rusty(Synthesizer),
    #[cfg(feature = "xsynth")]
    X(ChannelGroup),
}

#[derive(Clone)]
pub struct MidiSynth {
    engine: Arc<Mutex<Engine>>,
    sample_rate: u32,
    presets: Vec<PresetInfo>,
    backend: SynthBackend,
}

impl std::fmt::Debug for MidiSynth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiSynth")
            .field("backend", &self.backend)
            .field("presets_count", &self.presets.len())
            .finish()
    }
}

impl MidiSynth {
    pub fn new_with_sf2<R: Read>(sample_rate: u32, mut sf2_data: R, backend: SynthBackend) -> Result<Self> {
        let mut data = Vec::new();
        sf2_data.read_to_end(&mut data)?;

        match backend {
            SynthBackend::RustySynth => {
                let sf = RustySoundFont::new(&mut &data[..]).map_err(|e| anyhow::anyhow!("Failed to load SoundFont: {:?}", e))?;
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
                    engine: Arc::new(Mutex::new(Engine::Rusty(synth))),
                    sample_rate,
                    presets,
                    backend,
                })
            }
            #[cfg(feature = "xsynth")]
            SynthBackend::XSynth => {
                use std::io::Write;
                let temp_path = std::env::temp_dir().join("xsynth_font.sf2");
                let mut temp_file = std::fs::File::create(&temp_path)?;
                temp_file.write_all(&data)?;

                let soundfont = SampleSoundfont::new_sf2(
                    &temp_path,
                    AudioStreamParams { sample_rate, channels: ChannelCount::Stereo },
                    SoundfontInitOptions {
                        bank: None,
                        preset: None,
                        vol_envelope_options: EnvelopeOptions {
                            attack_curve: EnvelopeCurveType::Linear,
                            decay_curve: EnvelopeCurveType::Linear,
                            release_curve: EnvelopeCurveType::Linear,
                        },
                        use_effects: true,
                        interpolator: Interpolator::Linear,
                    }
                ).map_err(|e| anyhow::anyhow!("XSynth SF2 load failed: {:?}", e))?;
                
                let x_presets = xsynth_soundfonts::sf2::load_soundfont(&temp_path, sample_rate)
                    .map_err(|e| anyhow::anyhow!("Failed to extract presets: {:?}", e))?;
                
                let presets = x_presets.iter().map(|p| PresetInfo {
                    name: format!("Bank {} Preset {}", p.bank, p.preset),
                    bank: p.bank as i32,
                    patch: p.preset as i32,
                }).collect();

                let config = ChannelGroupConfig {
                    channel_init_options: ChannelInitOptions { fade_out_killing: false },
                    format: SynthFormat::Midi,
                    audio_params: AudioStreamParams { sample_rate, channels: ChannelCount::Stereo },
                    parallelism: ParallelismOptions {
                        channel: ThreadCount::None,
                        key: ThreadCount::None,
                    },
                };
                
                let mut channel_group = ChannelGroup::new(config);
                channel_group.send_event(SynthEvent::AllChannels(ChannelEvent::Config(ChannelConfigEvent::SetSoundfonts(vec![Arc::new(soundfont)]))));

                Ok(Self {
                    engine: Arc::new(Mutex::new(Engine::X(channel_group))),
                    sample_rate,
                    presets,
                    backend,
                })
            }
        }
    }

    pub fn backend(&self) -> SynthBackend {
        self.backend
    }

    pub fn note_on(&self, channel: u8, key: u8, velocity: u8) {
        if let Ok(mut engine) = self.engine.lock() {
            match &mut *engine {
                Engine::Rusty(s) => s.note_on(channel as i32, key as i32, velocity as i32),
                #[cfg(feature = "xsynth")]
                Engine::X(cg) => cg.send_event(SynthEvent::Channel(channel.into(), ChannelEvent::Audio(ChannelAudioEvent::NoteOn { key, vel: velocity }))),
            }
        }
    }

    pub fn note_off(&self, channel: u8, key: u8) {
        if let Ok(mut engine) = self.engine.lock() {
            match &mut *engine {
                Engine::Rusty(s) => s.note_off(channel as i32, key as i32),
                #[cfg(feature = "xsynth")]
                Engine::X(cg) => cg.send_event(SynthEvent::Channel(channel.into(), ChannelEvent::Audio(ChannelAudioEvent::NoteOff { key }))),
            }
        }
    }

    pub fn control_change(&self, channel: u8, controller: u8, value: u8) {
        if let Ok(mut engine) = self.engine.lock() {
            match &mut *engine {
                Engine::Rusty(s) => s.process_midi_message(channel as i32, 0xB0, controller as i32, value as i32),
                #[cfg(feature = "xsynth")]
                Engine::X(cg) => cg.send_event(SynthEvent::Channel(channel.into(), ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(controller, value))))),
            }
        }
    }

    pub fn program_change(&self, channel: u8, program: u8) {
        if let Ok(mut engine) = self.engine.lock() {
            match &mut *engine {
                Engine::Rusty(s) => s.process_midi_message(channel as i32, 0xC0, program as i32, 0),
                #[cfg(feature = "xsynth")]
                Engine::X(cg) => cg.send_event(SynthEvent::Channel(channel.into(), ChannelEvent::Audio(ChannelAudioEvent::ProgramChange(program)))),
            }
        }
    }
    
    pub fn set_preset(&self, channel: u8, bank: i32, patch: i32) {
        if let Ok(mut engine) = self.engine.lock() {
            match &mut *engine {
                Engine::Rusty(s) => {
                    s.process_midi_message(channel as i32, 0xB0, 0, bank);
                    s.process_midi_message(channel as i32, 0xC0, patch, 0);
                }
                #[cfg(feature = "xsynth")]
                Engine::X(cg) => {
                    cg.send_event(SynthEvent::Channel(channel.into(), ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(0, bank as u8)))));
                    cg.send_event(SynthEvent::Channel(channel.into(), ChannelEvent::Audio(ChannelAudioEvent::ProgramChange(patch as u8))));
                }
            }
        }
    }

    pub fn set_master_volume(&self, volume: f32) {
        if let Ok(mut engine) = self.engine.lock() {
            match &mut *engine {
                Engine::Rusty(s) => s.set_master_volume(volume),
                #[cfg(feature = "xsynth")]
                Engine::X(_cg) => {
                }
            }
        }
    }

    pub fn all_notes_off(&self) {
        if let Ok(mut engine) = self.engine.lock() {
            match &mut *engine {
                Engine::Rusty(s) => s.note_off_all(false),
                #[cfg(feature = "xsynth")]
                Engine::X(cg) => cg.send_event(SynthEvent::AllChannels(ChannelEvent::Audio(ChannelAudioEvent::AllNotesOff))),
            }
        }
    }

    pub fn get_presets(&self) -> &[PresetInfo] {
        &self.presets
    }

    pub fn get_source(&self) -> SynthSource {
        SynthSource {
            engine: self.engine.clone(),
            sample_rate: self.sample_rate,
            buffer: Vec::new(),
            buffer_pos: 0,
        }
    }
}

pub struct SynthSource {
    engine: Arc<Mutex<Engine>>,
    sample_rate: u32,
    buffer: Vec<f32>,
    buffer_pos: usize,
}

impl Iterator for SynthSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buffer_pos >= self.buffer.len() {
            self.fill_buffer();
        }

        let res = self.buffer.get(self.buffer_pos).cloned();
        self.buffer_pos += 1;
        res.or(Some(0.0))
    }
}

impl SynthSource {
    fn fill_buffer(&mut self) {
        let block_size = 128; 
        self.buffer.resize(block_size * 2, 0.0);
        
        if let Ok(mut engine) = self.engine.lock() {
            match &mut *engine {
                Engine::Rusty(s) => {
                    let mut left = vec![0.0; block_size];
                    let mut right = vec![0.0; block_size];
                    s.render(&mut left, &mut right);
                    
                    for i in 0..block_size {
                        self.buffer[i * 2] = left[i];
                        self.buffer[i * 2 + 1] = right[i];
                    }
                }
                #[cfg(feature = "xsynth")]
                Engine::X(cg) => {
                    cg.read_samples(&mut self.buffer);
                }
            }
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
