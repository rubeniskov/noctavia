use clap::Parser;
use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, text,
    Space,
};
use iced::{Alignment, Color, Element, Length, Subscription, Theme, Task};
use noctavia_midi::{Clock, Song, MidiEvent, MidiInputHandler, MidiSynth, PresetInfo, SynthBackend};
use noctavia_note_matcher::{NoteMatcher, Score};
use noctavia_ui_iced_widgets::{get_track_color, PianoRoll};
use noctavia_ui_transport::TransportBar;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::futures::SinkExt;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    midi: Option<PathBuf>,
    #[arg(short, long)]
    font: Option<PathBuf>,
    /// Sound bank
    #[arg(short, long, alias = "sb")]
    sound_bank: Option<PathBuf>,
    /// Initial MIDI port index
    #[arg(short = 'p', long)]
    midi_port: Option<usize>,
}

#[derive(Debug, Clone)]
enum Message {
    Tick(Instant),
    Midi(MidiEvent),
    MidiStatus(String),
    PortSelected(String),
    RefreshPorts,
    OpenFileDialog,
    OpenSF2Dialog,
    SongLoaded(Song),
    SF2Loaded(MidiSynth),
    PresetSelected(PresetInfo),
    BackendSelected(SynthBackend),
    TogglePlay,
    Seek(f32),
    ToggleTrack(usize),
    ToggleReverb(bool),
    ToggleChorus(bool),
    MouseNoteOn(u8),
    MouseNoteOff(u8),
    MouseNoteDrag(u8, u8),
}

pub fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let initial_song = if let Some(path) = &args.midi {
        if let Ok(data) = std::fs::read(path) {
            noctavia_midi::parse_file(&data).ok()
        } else {
            None
        }
    } else {
        None
    };

    let initial_song = Arc::new(initial_song);
    let font_path = Arc::new(args.font.clone());
    let sf2_path = Arc::new(args.sound_bank.clone());

    let boot = move || {
        let mut app = MidiTrainer::default();
        let mut tasks = Vec::new();

        if let Some(song) = (*initial_song).clone() {
            app.load_song(song);
        }

        // Load initial SF2 if provided
        if let Some(path) = (*sf2_path).clone() {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(synth) = MidiSynth::new_with_sf2(
                    44100,
                    std::io::Cursor::new(data),
                    SynthBackend::RustySynth,
                ) {
                    tasks.push(Task::done(Message::SF2Loaded(synth)));
                }
            }
        }

        // Embedded default font
        const DEFAULT_MUSIC_FONT: &[u8] = include_bytes!("../../../assets/MusicFont.ttf");

        // Load music font (either from CLI arg or the embedded default)
        let (data, is_custom) = if let Some(path) = (*font_path).clone() {
            (std::fs::read(path).ok(), true)
        } else {
            (Some(DEFAULT_MUSIC_FONT.to_vec()), false)
        };

        if let Some(data) = data {
            tasks.push(
                iced::font::load(std::borrow::Cow::Owned(data)).map(|_| Message::RefreshPorts),
            );
            app.music_font = Some(iced::Font {
                family: if is_custom {
                    iced::font::Family::Name("Custom Music Font")
                } else {
                    iced::font::Family::Name("Noto Music")
                },
                ..Default::default()
            });
        }

        app.midi_ports = MidiInputHandler::list_ports().unwrap_or_default();
        if let Some(port_idx) = args.midi_port {
            if let Some(port_name) = app.midi_ports.get(port_idx) {
                app.selected_port = Some(port_name.clone());
            }
        } else if !app.midi_ports.is_empty() {
            app.selected_port = Some(app.midi_ports[0].clone());
        }

        (app, Task::batch(tasks))
    };

    iced::application(boot, MidiTrainer::update, MidiTrainer::view)
        .title("Noctavia")
        .subscription(MidiTrainer::subscription)
        .theme(MidiTrainer::theme)
        .run()
}

struct MidiTrainer {
    last_tick: Instant,
    clock: Clock,
    song: Option<Song>,
    active_keys: HashSet<u8>,
    song_active_keys: HashMap<u8, i32>,
    matcher: Option<NoteMatcher>,

    midi_ports: Vec<String>,
    selected_port: Option<String>,
    midi_status: String,
    midi_latency: Option<Duration>,

    // Audio
    cpal_stream: Option<cpal::Stream>,
    synth: Option<MidiSynth>,
    synth_id: usize,
    presets: Vec<PresetInfo>,
    selected_preset: Option<PresetInfo>,

    // UI State
    is_playing: bool,
    bpm: f32,
    muted_tracks: HashSet<usize>,
    reverb_enabled: bool,
    chorus_enabled: bool,
    selected_backend: SynthBackend,
    music_font: Option<iced::Font>,
}

impl Default for MidiTrainer {
    fn default() -> Self {
        Self {
            last_tick: Instant::now(),
            clock: Clock::new(480),
            song: None,
            active_keys: HashSet::new(),
            song_active_keys: HashMap::new(),
            matcher: None,
            midi_ports: Vec::new(),
            selected_port: None,
            midi_status: String::from("Disconnected"),
            midi_latency: None,
            cpal_stream: None,
            synth: None,
            synth_id: 0,
            presets: Vec::new(),
            selected_preset: None,
            is_playing: false,
            bpm: 120.0,
            muted_tracks: HashSet::new(),
            reverb_enabled: false,
            chorus_enabled: false,
            selected_backend: SynthBackend::RustySynth,
            music_font: None,
        }
    }
}

impl MidiTrainer {
    fn load_song(&mut self, song: Song) {
        self.clock = Clock::new(song.ticks_per_quarter);
        self.matcher = Some(NoteMatcher::new(&song));
        if let Some(synth) = &self.synth {
            synth.all_notes_off();
        }
        self.song_active_keys.clear();
        self.last_tick = Instant::now();
        self.song = Some(song);
        self.is_playing = true;
    }

    fn theme(&self) -> Theme {
        Theme::CatppuccinMacchiato
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick(now) => {
                let dt = if self.is_playing {
                    now.duration_since(self.last_tick).as_secs_f32()
                } else {
                    0.0
                };
                self.last_tick = now;

                if let Some(ref song) = self.song {
                    let old_tick = self.clock.current_tick;
                    self.clock.update(dt, &song.tempo_map);
                    let new_tick = self.clock.current_tick;

                    if self.is_playing && new_tick > old_tick {
                        for (t_idx, track) in song.tracks.iter().enumerate() {
                            if self.muted_tracks.contains(&t_idx) {
                                continue;
                            }

                            // Binary search for notes starting in [old_tick, new_tick)
                            let start_idx = match track
                                .notes
                                .binary_search_by(|n| n.start_tick.cmp(&old_tick))
                            {
                                Ok(i) => i,
                                Err(i) => i,
                            };

                            for note in &track.notes[start_idx..] {
                                if note.start_tick >= new_tick {
                                    break;
                                }

                                if let Some(synth) = &self.synth {
                                    synth.note_on(t_idx as u8, note.key, note.velocity);
                                }
                                *self.song_active_keys.entry(note.key).or_insert(0) += 1;
                            }

                            // For note offs, we search from (old_tick - some_buffer) to catch notes that end now
                            let off_search_start =
                                old_tick.saturating_sub(song.ticks_per_quarter as u64 * 4); // search 4 beats back for endings
                            let off_start_idx = match track
                                .notes
                                .binary_search_by(|n| n.start_tick.cmp(&off_search_start))
                            {
                                Ok(i) => i,
                                Err(i) => i,
                            };

                            for note in &track.notes[off_start_idx..] {
                                if note.start_tick >= new_tick {
                                    break;
                                }
                                let note_end_tick = note.start_tick + note.duration_ticks;
                                if note_end_tick >= old_tick && note_end_tick < new_tick {
                                    if let Some(synth) = &self.synth {
                                        synth.note_off(t_idx as u8, note.key);
                                    }
                                    if let Some(count) = self.song_active_keys.get_mut(&note.key) {
                                        *count -= 1;
                                        if *count <= 0 {
                                            self.song_active_keys.remove(&note.key);
                                        }
                                    }
                                }
                            }

                            for cc in &track.control_changes {
                                if cc.tick >= old_tick && cc.tick < new_tick {
                                    if let Some(synth) = &self.synth {
                                        synth.control_change(t_idx as u8, cc.controller, cc.value);
                                    }
                                }
                            }

                            for pc in &track.program_changes {
                                if pc.tick >= old_tick && pc.tick < new_tick {
                                    if let Some(synth) = &self.synth {
                                        synth.program_change(t_idx as u8, pc.program);
                                    }
                                }
                            }
                        }
                    }

                    if let Some(ref mut matcher) = self.matcher {
                        matcher.update_misses(self.clock.current_secs);
                    }
                }
            }
            Message::Midi(event) => {
                let now = Instant::now();
                match event {
                    MidiEvent::NoteOn {
                        key,
                        velocity: _,
                        timestamp,
                    } => {
                        self.midi_latency = Some(now.duration_since(timestamp));
                        self.active_keys.insert(key);
                        // synth processing is now handled directly in MidiInputHandler
                        if let Some(ref mut matcher) = self.matcher {
                            matcher.on_note_on(key, self.clock.current_secs);
                        }
                    }
                    MidiEvent::NoteOff { key, timestamp } => {
                        self.midi_latency = Some(now.duration_since(timestamp));
                        self.active_keys.remove(&key);
                    }
                    MidiEvent::ControlChange {
                        controller: _,
                        value: _,
                        timestamp,
                    } => {
                        self.midi_latency = Some(now.duration_since(timestamp));
                    }
                }
            }
            Message::MidiStatus(status) => {
                self.midi_status = status;
            }
            Message::PortSelected(port) => {
                self.selected_port = Some(port);
                self.midi_status = String::from("Connecting...");
            }
            Message::RefreshPorts => {
                self.midi_ports = MidiInputHandler::list_ports().unwrap_or_default();
                if self.selected_port.is_none() && !self.midi_ports.is_empty() {
                    self.selected_port = Some(self.midi_ports[0].clone());
                }
            }
            Message::OpenFileDialog => {
                return Task::perform(
                    async {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter("MIDI", &["mid", "midi"])
                            .pick_file()
                            .await;

                        if let Some(file) = file {
                            let data = file.read().await;
                            noctavia_midi::parse_file(&data).ok()
                        } else {
                            None
                        }
                    },
                    |song| {
                        if let Some(song) = song {
                            Message::SongLoaded(song)
                        } else {
                            Message::RefreshPorts
                        }
                    },
                );
            }
            Message::OpenSF2Dialog => {
                let backend = self.selected_backend;
                return Task::perform(
                    async move {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter("SoundFont", &["sf2"])
                            .pick_file()
                            .await;

                        if let Some(file) = file {
                            let data = file.read().await;
                            MidiSynth::new_with_sf2(44100, std::io::Cursor::new(data), backend).ok()
                        } else {
                            None
                        }
                    },
                    |synth| {
                        if let Some(synth) = synth {
                            Message::SF2Loaded(synth)
                        } else {
                            Message::RefreshPorts
                        }
                    },
                );
            }
            Message::SongLoaded(song) => {
                self.load_song(song);
            }
            Message::SF2Loaded(synth) => {
                self.synth_id += 1;
                self.presets = synth.get_presets().to_vec();
                if !self.presets.is_empty() {
                    self.selected_preset = Some(self.presets[0].clone());
                }

                // Setup CPAL stream for low latency
                let host = cpal::default_host();
                if let Some(device) = host.default_output_device() {
                    if let Ok(config) = device.default_output_config() {
                        let mut source = synth.get_source();
                        let stream_config = cpal::StreamConfig {
                            channels: config.channels(),
                            sample_rate: config.sample_rate(),
                            // 256 samples buffer size (approx 5.8ms at 44.1kHz)
                            buffer_size: cpal::BufferSize::Fixed(256),
                        };

                        let stream = device.build_output_stream(
                            &stream_config,
                            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                                for sample in data.iter_mut() {
                                    *sample = source.next().unwrap_or(0.0);
                                }
                            },
                            |err| eprintln!("Audio error: {}", err),
                            None
                        );

                        if let Ok(s) = stream {
                            if let Ok(_) = s.play() {
                                self.cpal_stream = Some(s);
                            }
                        }
                    }
                }
                
                self.synth = Some(synth);
            }
            Message::PresetSelected(preset) => {
                if let Some(synth) = &self.synth {
                    // Set preset for all channels for now
                    for i in 0..16 {
                        synth.set_preset(i, preset.bank, preset.patch);
                    }
                }
                self.selected_preset = Some(preset);
            }
            Message::BackendSelected(backend) => {
                self.selected_backend = backend;
            }
            Message::TogglePlay => {
                self.is_playing = !self.is_playing;
                if !self.is_playing {
                    if let Some(synth) = &self.synth {
                        synth.all_notes_off();
                    }
                    self.song_active_keys.clear();
                }
            }
            Message::Seek(val) => {
                if let Some(ref song) = self.song {
                    self.clock.current_secs = val;
                    self.clock.current_tick = self.clock.secs_to_ticks(val, &song.tempo_map);
                    if let Some(synth) = &self.synth {
                        synth.all_notes_off();
                    }
                    self.song_active_keys.clear();
                }
            }
            Message::ToggleTrack(idx) => {
                if self.muted_tracks.contains(&idx) {
                    self.muted_tracks.remove(&idx);
                } else {
                    self.muted_tracks.insert(idx);
                }
            }
            Message::ToggleReverb(enabled) => {
                self.reverb_enabled = enabled;
                if let Some(synth) = &self.synth {
                    for i in 0..16 {
                        synth.control_change(i, 91, if enabled { 40 } else { 0 });
                    }
                }
            }
            Message::ToggleChorus(enabled) => {
                self.chorus_enabled = enabled;
                if let Some(synth) = &self.synth {
                    for i in 0..16 {
                        synth.control_change(i, 93, if enabled { 40 } else { 0 });
                    }
                }
            }
            Message::MouseNoteOn(key) => {
                self.active_keys.insert(key);
                if let Some(synth) = &self.synth {
                    synth.note_on(15, key, 100);
                }
            }
            Message::MouseNoteOff(key) => {
                self.active_keys.remove(&key);
                if let Some(synth) = &self.synth {
                    synth.note_off(15, key);
                }
            }
            Message::MouseNoteDrag(old_key, new_key) => {
                // Release old
                self.active_keys.remove(&old_key);
                if let Some(synth) = &self.synth {
                    synth.note_off(15, old_key);
                }

                // Press new
                self.active_keys.insert(new_key);
                if let Some(synth) = &self.synth {
                    synth.note_on(15, new_key, 100);
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let score = if let Some(ref matcher) = self.matcher {
            matcher.score
        } else {
            Score::default()
        };

        // --- Sidebar ---
        let sidebar = container(
            column![
                text("TRACKS")
                    .size(16)
                    .color(Color::from_rgb(0.5, 0.5, 0.5)),
                Space::new().height(10),
                scrollable(
                    column(
                        self.song
                            .as_ref()
                            .map(|s| {
                                s.tracks
                                    .iter()
                                    .enumerate()
                                    .map(|(i, t)| {
                                        let is_muted = self.muted_tracks.contains(&i);
                                        let color = get_track_color(i);
                                        button(
                                            row![
                                                container(Space::new().width(5))
                                                    .height(20)
                                                    .style(move |_| container::Style {
                                                        background: Some(color.into()),
                                                        ..Default::default()
                                                    }),
                                                text(&t.name).size(14).color(if is_muted {
                                                    Color::from_rgb(0.3, 0.3, 0.3)
                                                } else {
                                                    Color::WHITE
                                                }),
                                            ]
                                            .spacing(10)
                                            .align_y(Alignment::Center),
                                        )
                                        .on_press(Message::ToggleTrack(i))
                                        .style(button::secondary)
                                        .into()
                                    })
                                    .collect()
                            })
                            .unwrap_or_else(Vec::new)
                    )
                    .spacing(5)
                ),
                Space::new().height(Length::Fill),
                text(format!(
                    "Total Notes: {}",
                    self.song
                        .as_ref()
                        .map(|s| s.tracks.iter().map(|t| t.notes.len()).sum::<usize>())
                        .unwrap_or(0)
                ))
                .size(12)
                .color(Color::from_rgb(0.4, 0.4, 0.4)),
            ]
            .padding(20)
            .width(250),
        )
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Color::from_rgb(0.05, 0.05, 0.07).into()),
            ..Default::default()
        });

        // --- Top Bar ---
        let header = container(
            row![
                text("MIDI Trainer").size(20),
                Space::new().width(20),
                button("Open MIDI").on_press(Message::OpenFileDialog),
                button("Open SF2").on_press(Message::OpenSF2Dialog),
                pick_list(
                    &[
                        SynthBackend::RustySynth,
                        #[cfg(feature = "xsynth")]
                        SynthBackend::XSynth
                    ][..],
                    Some(self.selected_backend),
                    Message::BackendSelected
                )
                .width(120),
                Space::new().width(20),
                if !self.presets.is_empty() {
                    Element::from(
                        row![
                            checkbox(self.reverb_enabled).on_toggle(Message::ToggleReverb),
                            text("Reverb"),
                            checkbox(self.chorus_enabled).on_toggle(Message::ToggleChorus),
                            text("Chorus"),
                            pick_list(
                                &self.presets[..],
                                self.selected_preset.clone(),
                                Message::PresetSelected
                            )
                            .placeholder("Select Instrument")
                            .width(200)
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                    )
                } else {
                    text("No SF2 loaded")
                        .color(Color::from_rgb(0.4, 0.4, 0.4))
                        .into()
                },
                Space::new().width(Length::Fill),
                Element::from(if let Some(latency) = self.midi_latency {
                    text(format!("{:.1}ms", latency.as_secs_f64() * 1000.0))
                        .size(14)
                        .color(if latency.as_millis() > 20 {
                            Color::from_rgb(1.0, 0.3, 0.3)
                        } else {
                            Color::from_rgb(0.3, 1.0, 0.3)
                        })
                } else {
                    text("0ms").size(14).color(Color::from_rgb(0.4, 0.4, 0.4))
                }),
                Space::new().width(10),
                pick_list(
                    self.midi_ports.clone(),
                    self.selected_port.clone(),
                    Message::PortSelected
                )
                .placeholder("Select MIDI Device"),
                button("Refresh")
                    .on_press(Message::RefreshPorts)
                    .style(button::secondary),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .padding(15),
        )
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Color::from_rgb(0.07, 0.07, 0.1).into()),
            ..Default::default()
        });


        let middle_content = row![
            sidebar,
            PianoRoll::new(
                self.song.as_ref(),
                &self.clock,
                &self.active_keys,
                &self.song_active_keys,
                &self.muted_tracks,
                Message::MouseNoteOn,
                Message::MouseNoteOff,
                Message::MouseNoteDrag,
            )
            .music_font(self.music_font)
            .view(),
        ]
        .height(Length::Fill);

        let transport = TransportBar::new(
            self.is_playing,
            &self.clock,
            self.bpm,
            score,
            Message::TogglePlay,
            Message::Seek,
        )
        .view();

        column![header, middle_content, transport].into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let timer = iced::time::every(Duration::from_millis(16)).map(Message::Tick);
        let midi_sub = if let Some(ref selected_port) = self.selected_port {
            let port_index = self
                .midi_ports
                .iter()
                .position(|p| p == selected_port)
                .unwrap_or(0);
            
            // Re-run subscription when port OR synth changes
            Subscription::run_with(
                (selected_port.clone(), port_index, self.synth.clone(), self.synth_id),
                |(_port, idx, synth, _id)| {
                    MidiTrainer::midi_subscription(*idx, synth.clone())
                },
            )
        } else {
            Subscription::none()
        };

        let keyboard_sub = iced::event::listen().filter_map(|event| {
            if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event {
                match key {
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Space) => {
                        Some(Message::TogglePlay)
                    }
                    _ => None,
                }
            } else {
                None
            }
        });

        Subscription::batch(vec![timer, midi_sub, keyboard_sub])
    }

    fn midi_subscription(port_index: usize, synth: Option<MidiSynth>) -> impl iced::futures::Stream<Item = Message> {
        iced::stream::channel(100, move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            let (tx, rx) = crossbeam_channel::unbounded();
            if let Ok(_handler) = MidiInputHandler::new_with_port(tx, port_index, synth) {
                let _ = output
                    .send(Message::MidiStatus(String::from("Connected")))
                    .await;
                loop {
                    while let Ok(event) = rx.try_recv() {
                        let _ = output.send(Message::Midi(event)).await;
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        })
    }
}
