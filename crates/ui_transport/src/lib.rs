use iced::widget::{button, column, container, row, slider, text};
use iced::{Alignment, Color, Element, Length};
use midi_clock::Clock;
use note_matcher::Score;

pub struct TransportBar<'a, Message> {
    is_playing: bool,
    clock: &'a Clock,
    bpm: f32,
    score: Score,
    on_toggle_play: Message,
    on_seek: Box<dyn Fn(f32) -> Message + 'a>,
}

impl<'a, Message: 'a + Clone> TransportBar<'a, Message> {
    pub fn new(
        is_playing: bool,
        clock: &'a Clock,
        bpm: f32,
        score: Score,
        on_toggle_play: Message,
        on_seek: impl Fn(f32) -> Message + 'a,
    ) -> Self {
        Self {
            is_playing,
            clock,
            bpm,
            score,
            on_toggle_play,
            on_seek: Box::new(on_seek),
        }
    }

    pub fn view(self) -> Element<'a, Message> {
        container(
            row![
                button(if self.is_playing { "Pause" } else { "Play" })
                    .on_press(self.on_toggle_play)
                    .width(80),
                text(format!(
                    "{:02}:{:02}",
                    (self.clock.current_secs / 60.0) as i32,
                    (self.clock.current_secs % 60.0) as i32
                ))
                .size(14),
                slider(0.0..=300.0, self.clock.current_secs, move |val| (self.on_seek)(val))
                    .width(Length::Fill),
                text("BPM").size(12),
                text(format!("{}", self.bpm as i32)).size(14),
                column![
                    text(format!("Hits: {}", self.score.hits))
                        .color(Color::from_rgb(0.0, 1.0, 0.0))
                        .size(12),
                    text(format!("Misses: {}", self.score.misses))
                        .color(Color::from_rgb(1.0, 0.0, 0.0))
                        .size(12),
                ]
            ]
            .spacing(20)
            .align_y(Alignment::Center)
            .padding(15),
        )
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Color::from_rgb(0.07, 0.07, 0.1).into()),
            ..Default::default()
        })
        .into()
    }
}
