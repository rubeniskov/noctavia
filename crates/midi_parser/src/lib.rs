use anyhow::{anyhow, Result};
use midi_domain::{Note, Song, Track, TempoMap, TempoChange, ControlChange, ProgramChange};
use midly::{Header, Smf, TrackEventKind};
use std::collections::HashMap;

pub fn parse_file(data: &[u8]) -> Result<Song> {
    let smf = Smf::parse(data)?;
    let ticks_per_quarter = match smf.header.timing {
        midly::Timing::Metrical(ticks) => ticks.as_int(),
        _ => return Err(anyhow!("Only metrical timing (PPQ) is supported for now")),
    };

    let mut tracks = Vec::new();
    let mut tempo_changes = Vec::new();

    for (i, midly_track) in smf.tracks.iter().enumerate() {
        let mut notes = Vec::new();
        let mut control_changes = Vec::new();
        let mut program_changes = Vec::new();
        let mut active_notes: HashMap<(u8, u8), (u64, u8)> = HashMap::new();
        let mut current_tick = 0u64;
        let mut track_name = format!("Track {}", i);

        for event in midly_track {
            current_tick += event.delta.as_int() as u64;

            match event.kind {
                TrackEventKind::Meta(midly::MetaMessage::TrackName(name)) => {
                    track_name = String::from_utf8_lossy(name).to_string();
                }
                TrackEventKind::Meta(midly::MetaMessage::Tempo(micros)) => {
                    tempo_changes.push(TempoChange {
                        tick: current_tick,
                        micros_per_quarter: micros.as_int(),
                    });
                }
                TrackEventKind::Midi { channel, message } => {
                    let channel = channel.as_int();
                    match message {
                        midly::MidiMessage::NoteOn { key, vel } => {
                            let key = key.as_int();
                            let vel = vel.as_int();
                            if vel > 0 {
                                active_notes.insert((channel, key), (current_tick, vel));
                            } else {
                                if let Some((start_tick, velocity)) = active_notes.remove(&(channel, key)) {
                                    notes.push(Note {
                                        key,
                                        velocity,
                                        start_tick,
                                        duration_ticks: current_tick.saturating_sub(start_tick),
                                    });
                                }
                            }
                        }
                        midly::MidiMessage::NoteOff { key, .. } => {
                            let key = key.as_int();
                            if let Some((start_tick, velocity)) = active_notes.remove(&(channel, key)) {
                                notes.push(Note {
                                    key,
                                    velocity,
                                    start_tick,
                                    duration_ticks: current_tick.saturating_sub(start_tick),
                                });
                            }
                        }
                        midly::MidiMessage::Controller { controller, value } => {
                            control_changes.push(ControlChange {
                                tick: current_tick,
                                controller: controller.as_int(),
                                value: value.as_int(),
                            });
                        }
                        midly::MidiMessage::ProgramChange { program } => {
                            program_changes.push(ProgramChange {
                                tick: current_tick,
                                program: program.as_int(),
                            });
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if !notes.is_empty() || !control_changes.is_empty() || !program_changes.is_empty() {
            tracks.push(Track {
                name: track_name,
                notes,
                control_changes,
                program_changes,
            });
        }
    }

    // Sort tempo changes by tick and ensure there is a default tempo at tick 0
    tempo_changes.sort_by_key(|tc| tc.tick);
    if tempo_changes.is_empty() || tempo_changes[0].tick > 0 {
        tempo_changes.insert(0, TempoChange { tick: 0, micros_per_quarter: 500_000 }); // Default 120 BPM
    }

    Ok(Song {
        name: "Imported Song".to_string(),
        ticks_per_quarter: ticks_per_quarter as u16,
        tracks,
        tempo_map: TempoMap { changes: tempo_changes },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use midly::{Header, Smf, Track, TrackEvent, TrackEventKind, MidiMessage, MetaMessage};
    use std::fs;

    #[test]
    fn generate_test_midi() {
        let mut smf = Smf::new(Header::new(midly::Format::SingleTrack, midly::Timing::Metrical(480.into())));
        let mut track = Track::new();

        track.push(TrackEvent {
            delta: 0.into(),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Test Track")),
        });

        // C4
        track.push(TrackEvent {
            delta: 480.into(),
            kind: TrackEventKind::Midi { channel: 0.into(), message: MidiMessage::NoteOn { key: 60.into(), vel: 64.into() } },
        });
        track.push(TrackEvent {
            delta: 480.into(),
            kind: TrackEventKind::Midi { channel: 0.into(), message: MidiMessage::NoteOff { key: 60.into(), vel: 64.into() } },
        });

        smf.tracks.push(track);
        
        let _ = fs::create_dir_all("../../assets");
        smf.save("../../assets/test.mid").expect("Failed to save test MIDI");
    }
}
