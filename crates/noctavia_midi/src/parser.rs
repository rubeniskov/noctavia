use anyhow::{anyhow, Result};
use crate::domain::{Note, Song, Track, TempoMap, TempoChange, ControlChange, ProgramChange};
use midly::{Smf, TrackEventKind};
use std::collections::HashMap;

pub fn parse_file(data: &[u8]) -> Result<Song> {
    let smf = Smf::parse(data)?;
    let ticks_per_quarter = match smf.header.timing {
        midly::Timing::Metrical(ticks) => ticks.as_int(),
        _ => return Err(anyhow!("Only metrical timing (PPQ) is supported for now")),
    };

    let mut tracks_raw = Vec::new();
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
                        time_secs: 0.0, // Will calculate later
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
                                        start_secs: 0.0,
                                        duration_secs: 0.0,
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
                                    start_secs: 0.0,
                                    duration_secs: 0.0,
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
            tracks_raw.push((track_name, notes, control_changes, program_changes));
        }
    }

    // Sort tempo changes and calculate absolute times
    tempo_changes.sort_by_key(|tc| tc.tick);
    if tempo_changes.is_empty() || tempo_changes[0].tick > 0 {
        tempo_changes.insert(0, TempoChange { tick: 0, time_secs: 0.0, micros_per_quarter: 500_000 });
    }

    let mut current_time = 0.0f64;
    let mut current_tick = 0u64;
    let mut current_micros = 500_000u32;

    for i in 0..tempo_changes.len() {
        let delta_ticks = tempo_changes[i].tick - current_tick;
        current_time += (delta_ticks as f64 * current_micros as f64) / (ticks_per_quarter as f64 * 1_000_000.0);
        tempo_changes[i].time_secs = current_time;
        current_tick = tempo_changes[i].tick;
        current_micros = tempo_changes[i].micros_per_quarter;
    }

    let tempo_map = TempoMap { changes: tempo_changes };

    // Finalize tracks by calculating note seconds
    let tracks = tracks_raw.into_iter().map(|(name, mut notes, control_changes, program_changes)| {
        for note in &mut notes {
            note.start_secs = ticks_to_secs(note.start_tick, &tempo_map, ticks_per_quarter as u16);
            let end_secs = ticks_to_secs(note.start_tick + note.duration_ticks, &tempo_map, ticks_per_quarter as u16);
            note.duration_secs = end_secs - note.start_secs;
        }
        // Ensure notes are sorted by start time
        notes.sort_by(|a, b| a.start_secs.partial_cmp(&b.start_secs).unwrap());
        
        Track {
            name,
            notes,
            control_changes,
            program_changes,
        }
    }).collect();

    Ok(Song {
        name: "Imported Song".to_string(),
        ticks_per_quarter: ticks_per_quarter as u16,
        tracks,
        tempo_map,
    })
}

fn ticks_to_secs(target_tick: u64, tempo_map: &TempoMap, ticks_per_quarter: u16) -> f32 {
    let idx = match tempo_map.changes.binary_search_by_key(&target_tick, |c| c.tick) {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    };

    let change = &tempo_map.changes[idx];
    let delta_ticks = target_tick - change.tick;
    let delta_secs = (delta_ticks as f64 * change.micros_per_quarter as f64) / (ticks_per_quarter as f64 * 1_000_000.0);
    
    (change.time_secs + delta_secs) as f32
}
