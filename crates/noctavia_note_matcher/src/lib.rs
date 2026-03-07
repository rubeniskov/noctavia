use noctavia_midi_domain::{Note, Song};
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
    last_miss_check_idx: usize,
}

impl NoteMatcher {
    pub fn new(song: &Song) -> Self {
        let mut expected_notes = Vec::new();
        for track in &song.tracks {
            expected_notes.extend(track.notes.iter().cloned());
        }
        // Sort notes by start_secs for efficient matching
        expected_notes.sort_by(|a, b| a.start_secs.partial_cmp(&b.start_secs).unwrap());

        Self {
            expected_notes,
            matched_indices: HashSet::new(),
            score: Score::default(),
            tolerance_secs: 0.2, // 200ms tolerance
            last_miss_check_idx: 0,
        }
    }

    pub fn on_note_on(&mut self, key: u8, current_secs: f32) {
        let mut best_match: Option<(usize, f32)> = None;

        // Binary search for the first note that could match (start_secs >= current_secs - tolerance)
        let search_start_time = current_secs - self.tolerance_secs;
        let start_idx = match self.expected_notes.binary_search_by(|n| n.start_secs.partial_cmp(&search_start_time).unwrap()) {
            Ok(i) => i,
            Err(i) => i,
        };

        for i in start_idx..self.expected_notes.len() {
            let note = &self.expected_notes[i];
            if note.start_secs > current_secs + self.tolerance_secs {
                break;
            }

            if self.matched_indices.contains(&i) {
                continue;
            }

            if note.key == key {
                let diff = current_secs - note.start_secs;
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

    pub fn update_misses(&mut self, current_secs: f32) {
        // Only iterate from the last checked index
        while self.last_miss_check_idx < self.expected_notes.len() {
            let note = &self.expected_notes[self.last_miss_check_idx];
            if note.start_secs + self.tolerance_secs > current_secs {
                break;
            }

            if !self.matched_indices.contains(&self.last_miss_check_idx) {
                self.matched_indices.insert(self.last_miss_check_idx);
                self.score.misses += 1;
            }
            self.last_miss_check_idx += 1;
        }
    }
}
