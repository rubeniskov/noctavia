use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Program, event};
use iced::widget::{column, text, container, row, pick_list, button, slider, horizontal_space, vertical_space, scrollable, checkbox};
use iced::{Color, Element, Length, Point, Rectangle, Size, Theme, Renderer, Subscription, Alignment};
use iced::mouse;
use std::time::{Duration, Instant};
use midi_domain::Song;
use midi_clock::Clock;
use midi_io::{MidiEvent, MidiInputHandler};
use note_matcher::{NoteMatcher, Score};
use midi_synth::{MidiSynth, PresetInfo};
use std::path::PathBuf;
use std::collections::HashSet;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use clap::Parser;

use iced::futures::SinkExt;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    midi: Option<PathBuf>,
}

pub fn main() -> iced::Result {
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    let initial_song = if let Some(path) = args.midi {
        if let Ok(data) = std::fs::read(&path) {
            midi_parser::parse_file(&data).ok()
        } else {
            None
        }
    } else {
        None
    };

    iced::application("Rusthesia", MidiTrainer::update, MidiTrainer::view)
        .subscription(MidiTrainer::subscription)
        .theme(|_| Theme::Dark)
        .run_with(move || {
            let mut app = MidiTrainer::default();
            if let Some(song) = initial_song {
                app.load_song(song);
            }
            
            app.midi_ports = MidiInputHandler::list_ports().unwrap_or_default();
            if !app.midi_ports.is_empty() {
                app.selected_port = Some(app.midi_ports[0].clone());
            }

            (app, iced::Task::none())
        })
}

struct MidiTrainer {
    last_tick: Instant,
    clock: Clock,
    song: Option<Song>,
    active_keys: HashSet<u8>,
    matcher: Option<NoteMatcher>,
    
    midi_ports: Vec<String>,
    selected_port: Option<String>,
    midi_status: String,

    // Audio
    _audio_stream: Option<OutputStream>,
    audio_handle: Option<OutputStreamHandle>,
    synth: Option<MidiSynth>,
    synth_sink: Option<Sink>,
    presets: Vec<PresetInfo>,
    selected_preset: Option<PresetInfo>,

    // UI State
    is_playing: bool,
    bpm: f32,
    muted_tracks: HashSet<usize>,
    active_mouse_key: Option<u8>,
    reverb_enabled: bool,
    chorus_enabled: bool,
}

impl Default for MidiTrainer {
    fn default() -> Self {
        let (stream, handle) = OutputStream::try_default().ok().unzip();
        Self {
            last_tick: Instant::now(),
            clock: Clock::new(480),
            song: None,
            active_keys: HashSet::new(),
            matcher: None,
            midi_ports: Vec::new(),
            selected_port: None,
            midi_status: String::from("No device selected"),
            _audio_stream: stream,
            audio_handle: handle,
            synth: None,
            synth_sink: None,
            presets: Vec::new(),
            selected_preset: None,
            is_playing: true,
            bpm: 120.0,
            muted_tracks: HashSet::new(),
            active_mouse_key: None,
            reverb_enabled: true,
            chorus_enabled: true,
        }
    }
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
    TogglePlay,
    Seek(f32),
    ToggleTrack(usize),
    BpmChanged(f32),
    MouseNoteOn(u8),
    MouseNoteOff(u8),
    MouseNoteDrag(u8),
    ToggleReverb(bool),
    ToggleChorus(bool),
}

impl MidiTrainer {
    fn load_song(&mut self, song: Song) {
        self.clock = Clock::new(song.ticks_per_quarter);
        self.matcher = Some(NoteMatcher::new(&song));
        if let Some(synth) = &self.synth {
            synth.all_notes_off();
        }
        self.last_tick = Instant::now();
        self.song = Some(song);
        self.is_playing = true;
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
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
                            if self.muted_tracks.contains(&t_idx) { continue; }
                            
                            for note in &track.notes {
                                let note_end_tick = note.start_tick + note.duration_ticks;

                                if note.start_tick >= old_tick && note.start_tick < new_tick {
                                    if let Some(synth) = &self.synth {
                                        synth.note_on(t_idx as u8, note.key, note.velocity);
                                    }
                                }

                                if note_end_tick >= old_tick && note_end_tick < new_tick {
                                    if let Some(synth) = &self.synth {
                                        synth.note_off(t_idx as u8, note.key);
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
                        let ticks_per_secs = song.ticks_per_quarter as f32 * 2.0; 
                        matcher.update_misses(self.clock.current_secs, ticks_per_secs);
                    }
                }
            }
            Message::Midi(event) => {
                match event {
                    MidiEvent::NoteOn { key, velocity } => {
                        self.active_keys.insert(key);
                        if let Some(synth) = &self.synth {
                            // Use channel 15 for live input or similar
                            synth.note_on(15, key, velocity);
                        }
                        if let Some(ref mut matcher) = self.matcher {
                            if let Some(ref song) = self.song {
                                let ticks_per_secs = song.ticks_per_quarter as f32 * 2.0;
                                matcher.on_note_on(key, self.clock.current_secs, self.clock.current_tick, ticks_per_secs);
                            }
                        }
                    }
                    MidiEvent::NoteOff { key } => {
                        self.active_keys.remove(&key);
                        if let Some(synth) = &self.synth {
                            synth.note_off(15, key);
                        }
                    }
                    MidiEvent::ControlChange { controller, value } => {
                        if let Some(synth) = &self.synth {
                            synth.control_change(15, controller, value);
                        }
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
                return iced::Task::perform(
                    async {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter("MIDI", &["mid", "midi"])
                            .pick_file()
                            .await;
                        
                        if let Some(file) = file {
                            let data = file.read().await;
                            midi_parser::parse_file(&data).ok()
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
                    }
                );
            }
            Message::OpenSF2Dialog => {
                return iced::Task::perform(
                    async {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter("SoundFont", &["sf2"])
                            .pick_file()
                            .await;
                        
                        if let Some(file) = file {
                            let data = file.read().await;
                            MidiSynth::new_with_sf2(44100, std::io::Cursor::new(data)).ok()
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
                    }
                );
            }
            Message::SongLoaded(song) => {
                self.load_song(song);
            }
            Message::SF2Loaded(synth) => {
                self.presets = synth.get_presets().to_vec();
                if !self.presets.is_empty() {
                    self.selected_preset = Some(self.presets[0].clone());
                }
                
                if let Some(handle) = &self.audio_handle {
                    if let Ok(sink) = Sink::try_new(handle) {
                        sink.append(synth.get_source());
                        self.synth_sink = Some(sink);
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
            Message::TogglePlay => {
                self.is_playing = !self.is_playing;
                if !self.is_playing {
                    if let Some(synth) = &self.synth {
                        synth.all_notes_off();
                    }
                }
            }
            Message::Seek(val) => {
                if let Some(ref song) = self.song {
                    self.clock.current_secs = val;
                    self.clock.current_tick = self.clock.secs_to_ticks(val, &song.tempo_map);
                    if let Some(synth) = &self.synth {
                        synth.all_notes_off();
                    }
                }
            }
            Message::ToggleTrack(idx) => {
                if self.muted_tracks.contains(&idx) {
                    self.muted_tracks.remove(&idx);
                } else {
                    self.muted_tracks.insert(idx);
                }
            }
            Message::BpmChanged(val) => {
                self.bpm = val;
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
                self.active_mouse_key = Some(key);
                if let Some(synth) = &self.synth {
                    synth.note_on(15, key, 100);
                }
            }
            Message::MouseNoteOff(key) => {
                self.active_keys.remove(&key);
                self.active_mouse_key = None;
                if let Some(synth) = &self.synth {
                    synth.note_off(15, key);
                }
            }
            Message::MouseNoteDrag(key) => {
                if let Some(old_key) = self.active_mouse_key {
                    if old_key != key {
                        // Release old
                        self.active_keys.remove(&old_key);
                        if let Some(synth) = &self.synth {
                            synth.note_off(15, old_key);
                        }
                        
                        // Press new
                        self.active_keys.insert(key);
                        self.active_mouse_key = Some(key);
                        if let Some(synth) = &self.synth {
                            synth.note_on(15, key, 100);
                        }
                    }
                }
            }
        }
        iced::Task::none()
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
                text("TRACKS").size(16).color(Color::from_rgb(0.5, 0.5, 0.5)),
                vertical_space().height(10),
                scrollable(
                    column(
                        self.song.as_ref().map(|s| {
                            s.tracks.iter().enumerate().map(|(i, t)| {
                                let is_muted = self.muted_tracks.contains(&i);
                                let color = get_track_color(i);
                                button(
                                    row![
                                        container(horizontal_space().width(10)).width(5).height(20).style(move |_| container::Style {
                                            background: Some(color.into()),
                                            ..Default::default()
                                        }),
                                        text(&t.name).size(14).color(if is_muted { Color::from_rgb(0.3, 0.3, 0.3) } else { Color::WHITE }),
                                    ].spacing(10).align_y(Alignment::Center)
                                )
                                .on_press(Message::ToggleTrack(i))
                                .style(button::secondary)
                                .into()
                            }).collect()
                        }).unwrap_or_else(Vec::new)
                    ).spacing(5)
                ),
                vertical_space().height(Length::Fill),
                text(format!("Total Notes: {}", self.song.as_ref().map(|s| s.tracks.iter().map(|t| t.notes.len()).sum::<usize>()).unwrap_or(0))).size(12).color(Color::from_rgb(0.4, 0.4, 0.4)),
            ]
            .padding(20)
            .width(250)
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
                horizontal_space().width(20),
                button("Open MIDI").on_press(Message::OpenFileDialog),
                button("Open SF2").on_press(Message::OpenSF2Dialog),
                horizontal_space().width(20),
                if !self.presets.is_empty() {
                    Element::from(
                        row![
                            checkbox("Reverb", self.reverb_enabled).on_toggle(Message::ToggleReverb),
                            checkbox("Chorus", self.chorus_enabled).on_toggle(Message::ToggleChorus),
                            pick_list(&self.presets[..], self.selected_preset.clone(), Message::PresetSelected)
                                .placeholder("Select Instrument")
                                .width(200)
                        ].spacing(10).align_y(Alignment::Center)
                    )
                } else {
                    text("No SF2 loaded").color(Color::from_rgb(0.4, 0.4, 0.4)).into()
                },
                horizontal_space().width(Length::Fill),
                pick_list(self.midi_ports.clone(), self.selected_port.clone(), Message::PortSelected)
                    .placeholder("Select MIDI Device"),
                button("Refresh").on_press(Message::RefreshPorts).style(button::secondary),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .padding(15)
        )
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Color::from_rgb(0.07, 0.07, 0.1).into()),
            ..Default::default()
        });

        // --- Bottom Transport ---
        let footer = container(
            row![
                button(if self.is_playing { "Pause" } else { "Play" }).on_press(Message::TogglePlay).width(80),
                text(format!("{:02}:{:02}", (self.clock.current_secs / 60.0) as i32, (self.clock.current_secs % 60.0) as i32)).size(14),
                slider(0.0..=300.0, self.clock.current_secs, Message::Seek).width(Length::Fill),
                text("BPM").size(12),
                text(format!("{}", self.bpm as i32)).size(14),
                column![
                    text(format!("Hits: {}", score.hits)).color(Color::from_rgb(0.0, 1.0, 0.0)).size(12),
                    text(format!("Misses: {}", score.misses)).color(Color::from_rgb(1.0, 0.0, 0.0)).size(12),
                ]
            ]
            .spacing(20)
            .align_y(Alignment::Center)
            .padding(15)
        )
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Color::from_rgb(0.07, 0.07, 0.1).into()),
            ..Default::default()
        });

        let main_content = column![
            header,
            Canvas::new(self).width(Length::Fill).height(Length::Fill),
            footer,
        ];

        row![sidebar, main_content].into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let timer = iced::time::every(Duration::from_millis(16)).map(Message::Tick);
        let midi_sub = if let Some(ref selected_port) = self.selected_port {
            let port_index = self.midi_ports.iter().position(|p| p == selected_port).unwrap_or(0);
            Subscription::run_with_id(format!("midi-{}", selected_port), MidiTrainer::midi_subscription(port_index))
        } else {
            Subscription::none()
        };
        
        let keyboard_sub = iced::keyboard::on_key_press(|key, _modifiers| {
            match key {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Space) => Some(Message::TogglePlay),
                _ => None,
            }
        });

        Subscription::batch(vec![timer, midi_sub, keyboard_sub])
    }

    fn midi_subscription(port_index: usize) -> impl iced::futures::Stream<Item = Message> {
        iced::stream::channel(100, move |mut output| async move {
            let (tx, rx) = crossbeam_channel::unbounded();
            if let Ok(_handler) = MidiInputHandler::new_with_port(tx, port_index) {
                let _ = output.send(Message::MidiStatus(String::from("Connected"))).await;
                loop {
                    while let Ok(event) = rx.try_recv() { let _ = output.send(Message::Midi(event)).await; }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        })
    }
}

fn get_track_color(idx: usize) -> Color {
    match idx % 4 {
        0 => Color::from_rgb(0.0, 0.8, 1.0), // Cyan
        1 => Color::from_rgb(1.0, 0.0, 0.5), // Magenta
        2 => Color::from_rgb(0.0, 1.0, 0.4), // Green
        _ => Color::from_rgb(1.0, 0.8, 0.0), // Yellow
    }
}

impl Program<Message> for MidiTrainer {
    type State = ();

    fn update(&self, _state: &mut Self::State, event: event::Event, bounds: Rectangle, cursor: mouse::Cursor) -> (event::Status, Option<Message>) {
        let cursor_position = if let Some(p) = cursor.position_in(bounds) {
            p
        } else {
            return (event::Status::Ignored, None);
        };

        let keyboard_height = 100.0;
        let hit_line_y = bounds.height - keyboard_height;

        if cursor_position.y >= hit_line_y {
            let key_width = bounds.width / 88.0;
            let key_index = (cursor_position.x / key_width).floor() as u8 + 21;
            let key = key_index.clamp(21, 108);

            match event {
                event::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    return (event::Status::Captured, Some(Message::MouseNoteOn(key)));
                }
                event::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    if let Some(active_key) = self.active_mouse_key {
                         return (event::Status::Captured, Some(Message::MouseNoteOff(active_key)));
                    }
                }
                event::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                    if let Some(old_key) = self.active_mouse_key {
                        if old_key != key {
                            return (event::Status::Captured, Some(Message::MouseNoteDrag(key)));
                        }
                    }
                }
                _ => {}
            }
        } else if let Some(active_key) = self.active_mouse_key {
            // If mouse moves out of keyboard while pressed, release it
            match event {
                event::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    return (event::Status::Captured, Some(Message::MouseNoteOff(active_key)));
                }
                event::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                    // Released if dragged outside the piano
                    return (event::Status::Captured, Some(Message::MouseNoteOff(active_key)));
                }
                _ => {}
            }
        }

        (event::Status::Ignored, None)
    }

    fn draw(&self, _state: &Self::State, renderer: &Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let keyboard_height = 100.0;
        let hit_line_y = bounds.height - keyboard_height;
        let lookahead_secs = 4.0;
        let key_width = bounds.width / 88.0;

        // Draw grid lines
        let beat_spacing_secs = 0.5;
        let mut t = (self.clock.current_secs / beat_spacing_secs).floor() * beat_spacing_secs;
        while t < self.clock.current_secs + lookahead_secs {
            if t >= self.clock.current_secs {
                let y = hit_line_y - ((t - self.clock.current_secs) / lookahead_secs) * hit_line_y;
                frame.stroke(&Path::line(Point::new(0.0, y), Point::new(bounds.width, y)), 
                    canvas::Stroke::default().with_color(Color::from_rgb(0.15, 0.15, 0.2)).with_width(1.0));
            }
            t += beat_spacing_secs;
        }

        // Draw notes
        if let Some(ref song) = self.song {
            for (t_idx, track) in song.tracks.iter().enumerate() {
                if self.muted_tracks.contains(&t_idx) { continue; }
                let color = get_track_color(t_idx);
                
                for note in &track.notes {
                    let note_start_secs = self.clock.ticks_to_secs(note.start_tick, &song.tempo_map);
                    let note_end_secs = self.clock.ticks_to_secs(note.start_tick + note.duration_ticks, &song.tempo_map);
                    
                    if note_end_secs > self.clock.current_secs && note_start_secs < self.clock.current_secs + lookahead_secs {
                        let x = (note.key as f32 - 21.0) * key_width;
                        let y_start = hit_line_y - ((note_start_secs - self.clock.current_secs) / lookahead_secs) * hit_line_y;
                        let y_end = hit_line_y - ((note_end_secs - self.clock.current_secs) / lookahead_secs) * hit_line_y;
                        
                        frame.fill_rectangle(
                            Point::new(x + 1.0, y_end),
                            Size::new(key_width - 2.0, (y_start - y_end).max(4.0)),
                            color
                        );
                        frame.fill_rectangle(Point::new(x + 1.0, y_end), Size::new(key_width - 2.0, 2.0), Color::WHITE);
                    }
                }
            }
        }

        // Draw keyboard
        for i in 21..=108 {
            let x = (i as f32 - 21.0) * key_width;
            let is_active = self.active_keys.contains(&i);
            let note_in_octave = i % 12;
            let is_black = [1, 3, 6, 8, 10].contains(&note_in_octave);
            
            let key_color = if is_active { Color::from_rgb(1.0, 1.0, 0.5) } 
                            else if is_black { Color::from_rgb(0.05, 0.05, 0.05) } 
                            else { Color::from_rgb(0.95, 0.95, 0.95) };

            frame.fill_rectangle(Point::new(x, hit_line_y), Size::new(key_width - 1.0, keyboard_height), key_color);
            if !is_black {
                frame.stroke(&Path::rectangle(Point::new(x, hit_line_y), Size::new(key_width - 1.0, keyboard_height)),
                    canvas::Stroke::default().with_color(Color::from_rgb(0.8, 0.8, 0.8)).with_width(0.5));
            }
        }

        vec![frame.into_geometry()]
    }
}
