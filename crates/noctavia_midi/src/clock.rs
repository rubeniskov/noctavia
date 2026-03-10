use crate::domain::TempoMap;

#[derive(Debug, Clone)]
pub struct Clock {
    pub current_tick: u64,
    pub current_secs: f32,
    pub ticks_per_quarter: u16,
}

impl Clock {
    pub fn new(ticks_per_quarter: u16) -> Self {
        Self {
            current_tick: 0,
            current_secs: 0.0,
            ticks_per_quarter,
        }
    }

    pub fn update(&mut self, dt_secs: f32, tempo_map: &TempoMap) {
        self.current_secs += dt_secs;
        self.current_tick = self.secs_to_ticks(self.current_secs, tempo_map);
    }

    pub fn ticks_to_secs(&self, target_tick: u64, tempo_map: &TempoMap) -> f32 {
        if tempo_map.changes.is_empty() {
            return (target_tick as f32 * 500_000.0) / (self.ticks_per_quarter as f32 * 1_000_000.0);
        }

        // Binary search for the tempo change at or before target_tick
        let idx = match tempo_map.changes.binary_search_by_key(&target_tick, |c| c.tick) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };

        let change = &tempo_map.changes[idx];
        let delta_ticks = target_tick - change.tick;
        let delta_secs = (delta_ticks as f64 * change.micros_per_quarter as f64) / (self.ticks_per_quarter as f64 * 1_000_000.0);
        
        (change.time_secs + delta_secs) as f32
    }

    pub fn secs_to_ticks(&self, target_secs: f32, tempo_map: &TempoMap) -> u64 {
        if tempo_map.changes.is_empty() {
            return (target_secs * self.ticks_per_quarter as f32 * 1_000_000.0 / 500_000.0) as u64;
        }

        let target_secs_f64 = target_secs as f64;

        // Binary search for the tempo change at or before target_secs
        // Note: binary_search_by doesn't work well with f64 directly, but tempo_map.changes should be small enough or we can use a custom search
        let mut idx = 0;
        let mut low = 0;
        let mut high = tempo_map.changes.len();
        while low < high {
            let mid = (low + high) / 2;
            if tempo_map.changes[mid].time_secs <= target_secs_f64 {
                idx = mid;
                low = mid + 1;
            } else {
                high = mid;
            }
        }

        let change = &tempo_map.changes[idx];
        let remaining_secs = target_secs_f64 - change.time_secs;
        let remaining_ticks = (remaining_secs * self.ticks_per_quarter as f64 * 1_000_000.0) / change.micros_per_quarter as f64;
        
        change.tick + remaining_ticks as u64
    }
}
