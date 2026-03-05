use midi_domain::{TempoMap, TempoChange};

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
        let mut total_secs = 0.0;
        let mut current_tick = 0u64;
        let mut current_micros_per_quarter = 500_000u32;

        for change in &tempo_map.changes {
            if change.tick > target_tick {
                break;
            }
            
            let delta_ticks = change.tick - current_tick;
            total_secs += (delta_ticks as f32 * current_micros_per_quarter as f32) / (self.ticks_per_quarter as f32 * 1_000_000.0);
            
            current_tick = change.tick;
            current_micros_per_quarter = change.micros_per_quarter;
        }

        let remaining_ticks = target_tick - current_tick;
        total_secs += (remaining_ticks as f32 * current_micros_per_quarter as f32) / (self.ticks_per_quarter as f32 * 1_000_000.0);

        total_secs
    }

    pub fn secs_to_ticks(&self, target_secs: f32, tempo_map: &TempoMap) -> u64 {
        let mut current_secs = 0.0;
        let mut current_tick = 0u64;
        let mut current_micros_per_quarter = 500_000u32;

        for change in &tempo_map.changes {
            let delta_ticks = change.tick - current_tick;
            let delta_secs = (delta_ticks as f32 * current_micros_per_quarter as f32) / (self.ticks_per_quarter as f32 * 1_000_000.0);
            
            if current_secs + delta_secs > target_secs {
                break;
            }

            current_secs += delta_secs;
            current_tick = change.tick;
            current_micros_per_quarter = change.micros_per_quarter;
        }

        let remaining_secs = target_secs - current_secs;
        let remaining_ticks = (remaining_secs * self.ticks_per_quarter as f32 * 1_000_000.0) / current_micros_per_quarter as f32;
        
        current_tick + remaining_ticks as u64
    }
}
