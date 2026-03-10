# Noctavia

![logo](./assets/logo.svg)

A Synthesia-style MIDI trainer built with Rust and Iced.

## Project Structure

This project is a Rust monorepo:

- [`noctavia`](./crates/noctavia): Main Iced application entry point.
- [`noctavia_app_core`](./crates/noctavia_app_core): Shared application logic and state management.
- [`noctavia_midi_clock`](./crates/noctavia_midi_clock): Precise timing and playback clock for MIDI events.
- [`noctavia_midi_domain`](./crates/noctavia_midi_domain): Core domain types (Notes, Songs, Tracks).
- [`noctavia_midi_io`](./crates/noctavia_midi_io): Hardware MIDI input/output abstraction.
- [`noctavia_midi_parser`](./crates/noctavia_midi_parser): MIDI file decoding and track extraction.
- [`noctavia_midi_synth`](./crates/noctavia_midi_synth): Built-in software synthesizer.
- [`noctavia_note_matcher`](./crates/noctavia_note_matcher): Logic for comparing user input against a song.
- [`noctavia_piano_roll_model`](./crates/noctavia_piano_roll_model): Rendering model for the falling-notes view.
- [`noctavia_settings_store`](./crates/noctavia_settings_store): Persistence for user configuration and preferences.
- [`noctavia_telemetry`](./crates/noctavia_telemetry): Logging and performance monitoring.
- [`noctavia_ui_iced_widgets`](./crates/noctavia_ui_iced_widgets): Custom reusable Iced UI components.
- [`noctavia_ui_transport`](./crates/noctavia_ui_transport): UI controls for playback (play, pause, seek).

## Requirements

- Rust (latest stable)
- System MIDI libraries (e.g., `libasound2-dev` on Linux)

## Running

To run the application:

```bash
cargo run
```

To run tests:

```bash
cargo test --workspace
```


## Resources
- https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html
