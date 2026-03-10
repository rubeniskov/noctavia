# Noctavia

![logo](./assets/logo.svg)

A Synthesia-style MIDI trainer built with Rust and Iced.

## Project Structure

This project is a Rust monorepo:

- [`crates/noctavia`](./crates/noctavia): Main Iced application entry point.
- [`crates/noctavia_app_core`](./crates/noctavia_app_core): Shared application logic and state management.
- [`crates/noctavia_midi`](./crates/noctavia_midi): Consolidated MIDI logic including domain models, file parsing, timing clock, hardware I/O, and synthesizer.
- [`crates/noctavia_note_matcher`](./crates/noctavia_note_matcher): Logic for comparing user input against a song.
- [`crates/noctavia_piano_roll_model`](./crates/noctavia_piano_roll_model): Rendering model for the falling-notes view.
- [`crates/noctavia_settings_store`](./crates/noctavia_settings_store): Persistence for user configuration and preferences.
- [`crates/noctavia_telemetry`](./crates/noctavia_telemetry): Logging and performance monitoring.
- [`crates/noctavia_ui_iced_widgets`](./crates/noctavia_ui_iced_widgets): Custom reusable Iced UI components.
- [`crates/noctavia_ui_transport`](./crates/noctavia_ui_transport): UI controls for playback (play, pause, seek).

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
