use iced::widget::canvas::{self, Frame, Geometry, Path, Program, event, Canvas};
use iced::{Color, Element, Point, Rectangle, Size, Theme, Renderer, Font};
use iced::mouse;
use midi_domain::Song;
use midi_clock::Clock;
use std::collections::{HashSet, HashMap};

pub struct PianoRoll<'a, Message> {
    song: Option<&'a Song>,
    clock: &'a Clock,
    user_active_keys: &'a HashSet<u8>,
    song_active_keys: &'a HashMap<u8, i32>,
    muted_tracks: &'a HashSet<usize>,
    music_font: Option<Font>,
    on_note_on: Box<dyn Fn(u8) -> Message + 'a>,
    on_note_off: Box<dyn Fn(u8) -> Message + 'a>,
    on_note_drag: Box<dyn Fn(u8, u8) -> Message + 'a>,
}

impl<'a, Message: 'a> PianoRoll<'a, Message> {
    pub fn new(
        song: Option<&'a Song>,
        clock: &'a Clock,
        user_active_keys: &'a HashSet<u8>,
        song_active_keys: &'a HashMap<u8, i32>,
        muted_tracks: &'a HashSet<usize>,
        on_note_on: impl Fn(u8) -> Message + 'a,
        on_note_off: impl Fn(u8) -> Message + 'a,
        on_note_drag: impl Fn(u8, u8) -> Message + 'a,
    ) -> Self {
        Self {
            song,
            clock,
            user_active_keys,
            song_active_keys,
            muted_tracks,
            music_font: None,
            on_note_on: Box::new(on_note_on),
            on_note_off: Box::new(on_note_off),
            on_note_drag: Box::new(on_note_drag),
        }
    }

    pub fn music_font(mut self, font: Option<Font>) -> Self {
        self.music_font = font;
        self
    }

    pub fn view(self) -> Element<'a, Message> {
        Canvas::new(self)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into()
    }
}

#[derive(Default)]
pub struct State {
    active_mouse_key: Option<u8>,
}

impl<'a, Message: 'a> Program<Message> for PianoRoll<'a, Message> {
    type State = State;

    fn update(&self, state: &mut Self::State, event: event::Event, bounds: Rectangle, cursor: mouse::Cursor) -> (event::Status, Option<Message>) {
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
                    state.active_mouse_key = Some(key);
                    return (event::Status::Captured, Some((self.on_note_on)(key)));
                }
                event::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    if let Some(active_key) = state.active_mouse_key {
                         state.active_mouse_key = None;
                         return (event::Status::Captured, Some((self.on_note_off)(active_key)));
                    }
                }
                event::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                    if let Some(old_key) = state.active_mouse_key {
                        if old_key != key {
                            state.active_mouse_key = Some(key);
                            return (event::Status::Captured, Some((self.on_note_drag)(old_key, key)));
                        }
                    }
                }
                _ => {}
            }
        } else if let Some(active_key) = state.active_mouse_key {
            match event {
                event::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    state.active_mouse_key = None;
                    return (event::Status::Captured, Some((self.on_note_off)(active_key)));
                }
                event::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                    state.active_mouse_key = None;
                    return (event::Status::Captured, Some((self.on_note_off)(active_key)));
                }
                _ => {}
            }
        }

        (event::Status::Ignored, None)
    }

    fn draw(&self, _state: &Self::State, renderer: &Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let keyboard_height = 100.0;
        let staff_height = 150.0;
        let hit_line_y = bounds.height - keyboard_height;
        let roll_top_y = staff_height;
        let lookahead_secs = 4.0;
        let key_width = bounds.width / 88.0;

        // --- Draw Staff Background ---
        frame.fill_rectangle(Point::new(0.0, 0.0), Size::new(bounds.width, staff_height), Color::from_rgb(0.08, 0.08, 0.1));
        
        // Draw Grand Staff Lines
        let staff_center_y = staff_height / 2.0;
        let line_spacing = 8.0;
        let staff_stroke = canvas::Stroke::default().with_color(Color::from_rgb(0.3, 0.3, 0.4)).with_width(1.0);
        
        // Treble Staff (top)
        for i in 0..5 {
            let y = staff_center_y - 20.0 - (i as f32 * line_spacing);
            frame.stroke(&Path::line(Point::new(0.0, y), Point::new(bounds.width, y)), staff_stroke);
        }
        // Bass Staff (bottom)
        for i in 0..5 {
            let y = staff_center_y + 20.0 + (i as f32 * line_spacing);
            frame.stroke(&Path::line(Point::new(0.0, y), Point::new(bounds.width, y)), staff_stroke);
        }

        // Draw Clefs if font is available
        if let Some(font) = self.music_font {
            // Treble Clef: U+1D11E
            frame.fill_text(canvas::Text {
                content: String::from("\u{1D11E}"),
                position: Point::new(20.0, staff_center_y - 20.0 - (line_spacing * 3.0)),
                size: (line_spacing * 4.5).into(),
                font,
                color: Color::WHITE,
                ..Default::default()
            });
            // Bass Clef: U+1D122
            frame.fill_text(canvas::Text {
                content: String::from("\u{1D122}"),
                position: Point::new(20.0, staff_center_y + 20.0 - (line_spacing * 0.5)),
                size: (line_spacing * 3.5).into(),
                font,
                color: Color::WHITE,
                ..Default::default()
            });
        }

        // Draw staff vertical marker (current time)
        let staff_now_x = 100.0;
        frame.stroke(&Path::line(Point::new(staff_now_x, 0.0), Point::new(staff_now_x, staff_height)), 
            canvas::Stroke::default().with_color(Color::from_rgb(1.0, 0.5, 0.0)).with_width(2.0));

        // --- Draw Staff Notes ---
        let staff_secs_per_px = 0.02; // 50px per second
        if let Some(song) = self.song {
            for (t_idx, track) in song.tracks.iter().enumerate() {
                if self.muted_tracks.contains(&t_idx) { continue; }
                let color = get_track_color(t_idx);
                
                for note in &track.notes {
                    let note_start_secs = self.clock.ticks_to_secs(note.start_tick, &song.tempo_map);
                    let note_end_secs = self.clock.ticks_to_secs(note.start_tick + note.duration_ticks, &song.tempo_map);
                    
                    if note_end_secs > self.clock.current_secs - 2.0 && note_start_secs < self.clock.current_secs + 8.0 {
                        let x_start = staff_now_x + (note_start_secs - self.clock.current_secs) / staff_secs_per_px;
                        
                        if let Some(y) = get_note_y(note.key, staff_center_y, line_spacing) {
                            if let Some(font) = self.music_font {
                                let symbol = get_note_symbol(note.duration_ticks, song.ticks_per_quarter);
                                frame.fill_text(canvas::Text {
                                    content: String::from(symbol),
                                    position: Point::new(x_start - 4.0, y - line_spacing * 0.8),
                                    size: (line_spacing * 2.5).into(),
                                    font,
                                    color,
                                    ..Default::default()
                                });
                            } else {
                                let x_end = staff_now_x + (note_end_secs - self.clock.current_secs) / staff_secs_per_px;
                                frame.stroke(&Path::line(Point::new(x_start, y), Point::new(x_end, y)), 
                                    canvas::Stroke::default().with_color(color).with_width(4.0));
                                frame.fill_rectangle(Point::new(x_start - 2.0, y - 4.0), Size::new(8.0, 8.0), color);
                            }
                        }
                    }
                }
            }
        }

        // --- Draw Piano Roll Grid Lines ---
        let beat_spacing_secs = 0.5;
        let mut t = (self.clock.current_secs / beat_spacing_secs).floor() * beat_spacing_secs;
        while t < self.clock.current_secs + lookahead_secs {
            if t >= self.clock.current_secs {
                let y = hit_line_y - ((t - self.clock.current_secs) / lookahead_secs) * (hit_line_y - roll_top_y);
                if y >= roll_top_y {
                    frame.stroke(&Path::line(Point::new(0.0, y), Point::new(bounds.width, y)), 
                        canvas::Stroke::default().with_color(Color::from_rgb(0.15, 0.15, 0.2)).with_width(1.0));
                }
            }
            t += beat_spacing_secs;
        }

        // --- Draw Piano Roll Notes ---
        if let Some(song) = self.song {
            for (t_idx, track) in song.tracks.iter().enumerate() {
                if self.muted_tracks.contains(&t_idx) { continue; }
                let color = get_track_color(t_idx);
                
                for note in &track.notes {
                    let note_start_secs = self.clock.ticks_to_secs(note.start_tick, &song.tempo_map);
                    let note_end_secs = self.clock.ticks_to_secs(note.start_tick + note.duration_ticks, &song.tempo_map);
                    
                    if note_end_secs > self.clock.current_secs && note_start_secs < self.clock.current_secs + lookahead_secs {
                        let x = (note.key as f32 - 21.0) * key_width;
                        let y_start = hit_line_y - ((note_start_secs - self.clock.current_secs) / lookahead_secs) * (hit_line_y - roll_top_y);
                        let y_end = hit_line_y - ((note_end_secs - self.clock.current_secs) / lookahead_secs) * (hit_line_y - roll_top_y);
                        
                        let y_start_clamped = y_start.min(hit_line_y);
                        let y_end_clamped = y_end.max(roll_top_y);

                        if y_start_clamped > y_end_clamped {
                            let width = key_width - 2.0;
                            let height = (y_start_clamped - y_end_clamped).max(4.0);
                            frame.fill_rectangle(
                                Point::new(x + 1.0, y_end_clamped),
                                Size::new(width.max(1.0), height),
                                color
                            );
                            frame.fill_rectangle(Point::new(x + 1.0, y_end_clamped), Size::new(width.max(1.0), 2.0), Color::WHITE);
                        }
                    }
                }
            }
        }

        // Draw keyboard
        for i in 21..=108 {
            let x = (i as f32 - 21.0) * key_width;
            let is_song_active = self.song_active_keys.contains_key(&i);
            let is_user_active = self.user_active_keys.contains(&i);
            let is_active = is_song_active || is_user_active;
            
            let note_in_octave = i % 12;
            let is_black = [1, 3, 6, 8, 10].contains(&note_in_octave);
            
            // Draw Glow
            if is_active {
                let glow_height = 150.0;
                let glow_color = if is_user_active { Color::from_rgba(1.0, 1.0, 0.5, 0.5) } else { Color::from_rgba(0.0, 0.8, 1.0, 0.4) };
                
                let width = key_width - 1.0;
                if width > 0.0 {
                    frame.fill_rectangle(
                        Point::new(x, hit_line_y - glow_height),
                        Size::new(width, glow_height),
                        canvas::Fill {
                            style: canvas::Style::Gradient(canvas::Gradient::Linear(
                                canvas::gradient::Linear::new(Point::new(x, hit_line_y), Point::new(x, hit_line_y - glow_height))
                                    .add_stop(0.0, glow_color)
                                    .add_stop(1.0, Color::TRANSPARENT),
                            )),
                            ..Default::default()
                        }
                    );
                }
            }

            let key_color = if is_active { Color::from_rgb(1.0, 1.0, 0.5) } 
                            else if is_black { Color::from_rgb(0.05, 0.05, 0.05) } 
                            else { Color::from_rgb(0.95, 0.95, 0.95) };

            let width = key_width - 1.0;
            if width > 0.0 {
                frame.fill_rectangle(Point::new(x, hit_line_y), Size::new(width, keyboard_height), key_color);
                if !is_black {
                    frame.stroke(&Path::rectangle(Point::new(x, hit_line_y), Size::new(width, keyboard_height)),
                        canvas::Stroke::default().with_color(Color::from_rgb(0.8, 0.8, 0.8)).with_width(0.5));
                }
            }
        }

        vec![frame.into_geometry()]
    }
}

pub fn get_track_color(idx: usize) -> Color {
    match idx % 4 {
        0 => Color::from_rgb(0.0, 0.8, 1.0), // Cyan
        1 => Color::from_rgb(1.0, 0.0, 0.5), // Magenta
        2 => Color::from_rgb(0.0, 1.0, 0.4), // Green
        _ => Color::from_rgb(1.0, 0.8, 0.0), // Yellow
    }
}

fn get_note_y(key: u8, center_y: f32, spacing: f32) -> Option<f32> {
    let notes_in_octave = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];
    let octave = (key / 12) as i32 - 1; 
    let note_idx = (key % 12) as usize;
    let diatonic_pos = (octave * 7) + notes_in_octave[note_idx];
    
    let middle_c_pos = 28;
    let relative_pos = diatonic_pos - middle_c_pos;
    
    if key >= 60 {
        Some(center_y - 12.0 - (relative_pos as f32 * spacing * 0.5))
    } else {
        Some(center_y + 12.0 - (relative_pos as f32 * spacing * 0.5))
    }
}

fn get_note_symbol(duration_ticks: u64, ticks_per_quarter: u16) -> &'static str {
    let quarter = ticks_per_quarter as f32;
    let dur = duration_ticks as f32;
    
    if dur > quarter * 3.0 {
        "\u{1D15D}" // Whole note
    } else if dur > quarter * 1.5 {
        "\u{1D15E}" // Half note
    } else if dur > quarter * 0.75 {
        "\u{1D15F}" // Quarter note
    } else if dur > quarter * 0.375 {
        "\u{1D160}" // Eighth note
    } else {
        "\u{1D161}" // Sixteenth note
    }
}
