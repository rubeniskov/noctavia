use midi_domain::{Note, Song};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Default)]
pub struct Score {
    pub hits: u32,
    pub early: u32,
    pub late: u32,
    pub misses: u32,
}

pub struct NoteMatcher {
    expected_notes: Vec<Note>,
    matched_indices: HashSet<usize>,
    pub score: Score,
    tolerance_secs: f32,
}

impl NoteMatcher {
    pub fn new(song: &Song) -> Self {
        let mut expected_notes = Vec::new();
        for track in &song.tracks {
            expected_notes.extend(track.notes.iter().cloned());
        }
        // Sort notes by start tick for efficient matching
        expected_notes.sort_by_key(|n| n.start_tick);

        Self {
            expected_notes,
            matched_indices: HashSet::new(),
            score: Score::default(),
            tolerance_secs: 0.2, // 200ms tolerance
        }
    }

    pub fn on_note_on(&mut self, key: u8, current_secs: f32, _current_tick: u64, ticks_per_secs: f32) {
        let mut best_match: Option<(usize, f32)> = None;

        for (i, note) in self.expected_notes.iter().enumerate() {
            if self.matched_indices.contains(&i) {
                continue;
            }

            if note.key == key {
                let note_start_secs = note.start_tick as f32 / ticks_per_secs;
                let diff = current_secs - note_start_secs;

                if diff.abs() <= self.tolerance_secs {
                    if let Some((_, best_diff)) = best_match {
                        if diff.abs() < best_diff.abs() {
                            best_match = Some((i, diff));
                        }
                    } else {
                        best_match = Some((i, diff));
                    }
                }
            }
        }

        if let Some((index, diff)) = best_match {
            self.matched_indices.insert(index);
            if diff < -0.05 {
                self.score.early += 1;
            } else if diff > 0.05 {
                self.score.late += 1;
            } else {
                self.score.hits += 1;
            }
        }
    }

    pub fn update_misses(&mut self, current_secs: f32, ticks_per_secs: f32) {
        for (i, note) in self.expected_notes.iter().enumerate() {
            if self.matched_indices.contains(&i) {
                continue;
            }

            let note_start_secs = note.start_tick as f32 / ticks_per_secs;
            if current_secs > note_start_secs + self.tolerance_secs {
                self.matched_indices.insert(i);
                self.score.misses += 1;
            }
        }
    }
}
