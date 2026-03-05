# Rusthesia - MIDI Piano Roll Trainer

A Synthesia-style MIDI trainer built with Rust and Iced.

## Project Structure

This project is a Rust monorepo:

- `apps/midi_trainer`: Main Iced application.
- `crates/midi_domain`: Core domain types (Notes, Songs, etc.).
- `crates/midi_parser`: MIDI file parsing logic.
- `crates/midi_clock`: Timing and playback clock.
- `crates/midi_io`: MIDI hardware input abstraction.
- `crates/piano_roll_model`: Rendering model for the piano roll.
- `crates/note_matcher`: Note matching and scoring logic.
- `crates/ui_iced_widgets`: Custom Iced widgets.

## Requirements

- Rust (latest stable)
- System MIDI libraries (e.g., `libasound2-dev` on Linux)

## Running

To run the application:

```bash
cargo run -p midi_trainer
```

To run tests:

```bash
cargo test --workspace
```
