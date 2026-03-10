use midir::{MidiInput, MidiInputConnection};
use crossbeam_channel::Sender;
use anyhow::{anyhow, Result};
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub enum MidiEvent {
    NoteOn { key: u8, velocity: u8, timestamp: Instant },
    NoteOff { key: u8, timestamp: Instant },
    ControlChange { controller: u8, value: u8, timestamp: Instant },
}

pub struct MidiInputHandler {
    _connection: Option<MidiInputConnection<()>>,
}

impl MidiInputHandler {
    pub fn list_ports() -> Result<Vec<String>> {
        let midi_in = MidiInput::new("noctavia-probe").map_err(|e| anyhow!("Failed to create MIDI input: {}", e))?;
        let ports = midi_in.ports();
        let mut names = Vec::new();
        for port in ports {
            names.push(midi_in.port_name(&port).map_err(|e| anyhow!("Failed to get port name: {}", e))?);
        }
        Ok(names)
    }

    pub fn new(sender: Sender<MidiEvent>, synth: Option<crate::synth::MidiSynth>) -> Result<Self> {
        Self::new_with_port(sender, 0, synth)
    }

    pub fn new_with_port(sender: Sender<MidiEvent>, port_index: usize, synth: Option<crate::synth::MidiSynth>) -> Result<Self> {
        let midi_in = MidiInput::new("noctavia-input").map_err(|e| anyhow!("Failed to create MIDI input: {}", e))?;
        let ports = midi_in.ports();
        
        if let Some(port) = ports.get(port_index) {
            let port_name = midi_in.port_name(port).unwrap_or_else(|_| "Unknown".to_string());
            tracing::info!("Connecting to MIDI port: {}", port_name);
            
            let connection = midi_in.connect(port, "noctavia-conn", move |_stamp, message, _| {
                let timestamp = Instant::now();
                if message.len() >= 3 {
                    let status = message[0] & 0xF0;
                    let key = message[1];
                    let vel = message[2];
                    
                    let event = match status {
                        0x90 if vel > 0 => {
                            Some(MidiEvent::NoteOn { key, velocity: vel, timestamp })
                        }
                        0x90 | 0x80 => {
                            Some(MidiEvent::NoteOff { key, timestamp })
                        }
                        0xB0 => {
                            Some(MidiEvent::ControlChange { controller: key, value: vel, timestamp })
                        }
                        _ => None
                    };

                    if let Some(event) = event {
                        if let Some(s) = &synth {
                            s.process_event(&event);
                        }
                        let _ = sender.send(event);
                    }
                }
            }, ()).map_err(|e| anyhow!("Failed to connect to MIDI port: {}", e))?;
            
            Ok(Self { _connection: Some(connection) })
        } else {
            Err(anyhow!("MIDI port index {} not found", port_index))
        }
    }
}
