pub mod domain;
pub mod parser;
pub mod clock;
pub mod io;
pub mod synth;

// Re-export common types
pub use domain::*;
pub use parser::parse_file;
pub use clock::Clock;
pub use io::{MidiInputHandler, MidiEvent};
pub use synth::{MidiSynth, SynthBackend, PresetInfo, SynthSource};
