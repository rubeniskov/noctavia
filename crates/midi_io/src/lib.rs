use midir::{MidiInput, MidiInputConnection};
use crossbeam_channel::Sender;
use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy)]
pub enum MidiEvent {
    NoteOn { key: u8, velocity: u8 },
    NoteOff { key: u8 },
}

pub struct MidiInputHandler {
    _connection: Option<MidiInputConnection<()>>,
}

impl MidiInputHandler {
    pub fn list_ports() -> Result<Vec<String>> {
        let midi_in = MidiInput::new("rusthesia-probe").map_err(|e| anyhow!("Failed to create MIDI input: {}", e))?;
        let ports = midi_in.ports();
        let mut names = Vec::new();
        for port in ports {
            names.push(midi_in.port_name(&port).map_err(|e| anyhow!("Failed to get port name: {}", e))?);
        }
        Ok(names)
    }

    pub fn new(sender: Sender<MidiEvent>) -> Result<Self> {
        Self::new_with_port(sender, 0)
    }

    pub fn new_with_port(sender: Sender<MidiEvent>, port_index: usize) -> Result<Self> {
        let midi_in = MidiInput::new("rusthesia-input").map_err(|e| anyhow!("Failed to create MIDI input: {}", e))?;
        let ports = midi_in.ports();
        
        if let Some(port) = ports.get(port_index) {
            let port_name = midi_in.port_name(port).unwrap_or_else(|_| "Unknown".to_string());
            tracing::info!("Connecting to MIDI port: {}", port_name);
            
            let connection = midi_in.connect(port, "rusthesia-conn", move |_stamp, message, _| {
                if message.len() >= 3 {
                    let status = message[0] & 0xF0;
                    let key = message[1];
                    let vel = message[2];
                    
                    match status {
                        0x90 if vel > 0 => {
                            let _ = sender.send(MidiEvent::NoteOn { key, velocity: vel });
                        }
                        0x90 | 0x80 => {
                            let _ = sender.send(MidiEvent::NoteOff { key });
                        }
                        _ => {}
                    }
                }
            }, ()).map_err(|e| anyhow!("Failed to connect to MIDI port: {}", e))?;
            
            Ok(Self { _connection: Some(connection) })
        } else {
            Err(anyhow!("MIDI port index {} not found", port_index))
        }
    }
}
