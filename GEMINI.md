# GEMINI.md

# MIDI Piano Roll Trainer (Rust + Iced) - Monorepo Plan and Roadmap

## Vision

Build a cross-platform Rust application that:

- Loads MIDI songs (`.mid`, `.midi`)
- Displays a falling-note piano roll (Synthesia-style) synced to tempo
- Connects to USB MIDI keyboards/controllers for live note input
- Follows user performance against the loaded song (note matching, timing feedback)
- Uses `iced` for GUI rendering and interaction
- Is organized as a Rust monorepo with modular crates for maintainability and fast iteration

The long-term goal is a developer-friendly foundation that can evolve into a full practice/training app (looping, scoring, hands separation, tempo control, metronome, etc.).

---

## Product Goals

### Core user experience (MVP)

1. Open a MIDI file.
2. Parse tracks, tempo map, time signature, and notes.
3. Show a piano keyboard + falling bars piano roll.
4. Play/scroll at the correct tempo (visual playback first, audio optional in later phase).
5. Connect a USB MIDI input device.
6. Highlight expected notes and compare against played notes.
7. Provide basic feedback (hit/miss/late/early).

### Non-goals for MVP

- Full DAW editing
- VST hosting
- Audio synthesis engine
- Advanced notation (sheet music engraving)
- Cloud sync/accounts

---

## Design Principles

- **Modular crates**: isolate parsing, timing, rendering model, MIDI I/O, app state, and UI.
- **Deterministic timing core**: timing and song position logic should be testable without GUI.
- **Transport-agnostic architecture**: define playback/clock traits so engine is not tied to a single runtime.
- **UI thinness**: `iced` views should consume view models, not own business logic.
- **Cross-platform-first**: Linux/macOS/Windows support from the start where possible.
- **Progressive enhancement**: get visual sync and note matching correct before fancy rendering.

---
