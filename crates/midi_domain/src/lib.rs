use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub key: u8,
    pub velocity: u8,
    pub start_tick: u64,
    pub duration_ticks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub name: String,
    pub ticks_per_quarter: u16,
    pub tracks: Vec<Track>,
    pub tempo_map: TempoMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TempoMap {
    pub changes: Vec<TempoChange>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TempoChange {
    pub tick: u64,
    pub micros_per_quarter: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ControlChange {
    pub tick: u64,
    pub controller: u8,
    pub value: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub notes: Vec<Note>,
    pub control_changes: Vec<ControlChange>,
}
