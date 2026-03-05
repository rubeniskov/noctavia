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

## Recommended Monorepo Structure

```text
midi-trainer/
├─ Cargo.toml                    # workspace root
├─ rust-toolchain.toml
├─ README.md
├─ GEMINI.md                     # this file
├─ .cargo/
│  └─ config.toml
├─ apps/
│  └─ iced_desktop/              # executable app using iced
│     ├─ Cargo.toml
│     └─ src/
│        ├─ main.rs
│        ├─ app.rs
│        ├─ message.rs
│        ├─ theme.rs
│        └─ screens/
├─ crates/
│  ├─ midi_domain/               # core domain types (notes, song, tempo map, timing)
│  ├─ midi_parser/               # parse MIDI files into domain types
│  ├─ midi_clock/                # playback clock + tempo position mapping
│  ├─ midi_io/                   # USB MIDI input/output abstraction (midir backend)
│  ├─ piano_roll_model/          # transforms song position into renderable bars/view data
│  ├─ note_matcher/              # compare live MIDI input vs expected notes
│  ├─ app_core/                  # orchestrates session state, commands, use-cases
│  ├─ ui_iced_widgets/           # custom widgets/canvas render helpers for piano roll/keyboard
│  ├─ settings_store/            # persistence (preferences, recent files, midi devices)
│  ├─ telemetry/                 # logs/tracing/instrumentation helpers
│  └─ test_support/              # fixtures, builders, golden test helpers
├─ assets/
│  ├─ midi_examples/
│  └─ fonts/
├─ docs/
│  ├─ architecture.md
│  ├─ timing-model.md
│  ├─ rendering-model.md
│  └─ testing-strategy.md
└─ xtask/                        # optional automation for dev tasks (fixtures, lint, release)
   ├─ Cargo.toml
   └─ src/main.rs
```

---

## Crate Responsibilities (Detailed)

### `midi_domain`
**Purpose:** Pure core types and logic, no I/O.

Contains:
- `Song`
- `Track`
- `NoteEvent` / `NoteSpan`
- `TempoMap`
- `TimeSignatureMap`
- `Ticks`, `Beats`, `Micros`, `SongTime`
- `PianoKey` / MIDI note abstractions
- Basic validation rules

Why separate it:
- Reusable in tests and future frontends
- Keeps business logic independent from parser and UI

---

### `midi_parser`
**Purpose:** Parse Standard MIDI Files into `midi_domain` types.

Likely dependencies:
- `midly` (recommended starting point)

Tasks:
- Read MIDI header/tracks
- Merge note on/off pairs into note spans
- Build tempo map from meta events
- Build time signatures
- Preserve track/channel metadata
- Normalize edge cases (running status handled by parser lib)

Output:
- `ParsedSong` -> converted into `midi_domain::Song`

Notes:
- Support type 0 and type 1 first
- Type 2 can be deferred

---

### `midi_clock`
**Purpose:** Song position, playback state, tempo-aware timing, and scheduling calculations.

Responsibilities:
- Convert wall-clock time -> song time using tempo map
- Support play/pause/seek/stop
- Tempo multiplier (e.g., 0.5x, 0.75x, 1.25x)
- Loop ranges (future phase)
- Fixed-step simulation mode for tests

Key API idea:
- `Clock` trait + `Transport` state machine
- `ClockSource` abstraction (system time vs test clock)

This crate is critical because piano roll sync and note matching depend on consistent timing.

---

### `midi_io`
**Purpose:** MIDI hardware device enumeration and input event stream.

Likely dependencies:
- `midir` (input/output)
- `crossbeam-channel` or `flume` for event forwarding

Responsibilities:
- List MIDI input devices
- Connect/disconnect device
- Receive MIDI messages with timestamps
- Parse note on/off, sustain pedal, velocity
- Optional MIDI output for metronome or external keyboard lights (future)

Design tip:
- Expose a backend trait (`MidiInputBackend`) so you can test with a fake source.

---

### `piano_roll_model`
**Purpose:** Transform song data and playback position into render-ready items.

Responsibilities:
- Determine visible note bars in viewport
- Compute note Y positions based on time-to-impact and scroll speed
- Compute X positions based on key index
- Handle track/channel filtering
- Hand coloring/grouping metadata (future)
- Keyboard highlight state (expected/pressed/correct)

Output examples:
- `Vec<RenderNoteBar>`
- `KeyboardState`
- `MeasureLine` / `BeatLine`

This crate should not know anything about `iced` APIs.

---

### `note_matcher`
**Purpose:** Compare incoming live MIDI notes against expected notes from the score.

Responsibilities:
- Matching windows (early/late tolerance)
- Chord matching behavior
- Sustain pedal handling rules (future)
- Scoring events: `Correct`, `Early`, `Late`, `Miss`, `Extra`
- Per-note accuracy stats and session aggregates

This can become a strong differentiator later (practice analytics).

---

### `app_core`
**Purpose:** Application use-cases and state orchestration, independent of the UI toolkit.

Responsibilities:
- Session lifecycle (idle, file loaded, playing, paused, practicing)
- Commands (load file, play, pause, seek, connect MIDI)
- Derived view models for UI consumption
- Event routing between clock, parser, midi_io, matcher
- Error handling and domain-level messages

Pattern ideas:
- Redux-like reducer (simple and testable)
- Message/command split
- Async command handlers behind traits

---

### `ui_iced_widgets`
**Purpose:** Reusable `iced` widgets/components for visualization.

Potential widgets:
- `PianoRollCanvas`
- `KeyboardView`
- `TransportBar`
- `TempoControl`
- `DeviceSelector`
- `TrackListPanel`

Implementation notes:
- Start with `iced::widget::canvas` for custom drawing
- Keep drawing stateless; feed precomputed view models from `piano_roll_model`
- Optimize redraws later with caching and dirty regions

---

### `settings_store`
**Purpose:** Persist app settings and session preferences.

Stores:
- Recent MIDI files
- Last selected MIDI device
- UI preferences (theme, note colors)
- Input latency calibration (future)

Potential dependencies:
- `serde`, `toml` or `ron`, `directories`

---

### `telemetry`
**Purpose:** Logging and diagnostics across the workspace.

Dependencies:
- `tracing`, `tracing-subscriber`

Use cases:
- Timing drift diagnostics
- Device connection logs
- Performance frame timing (future)

---

## Architecture Overview

### High-level data flow

```text
[MIDI File] -> midi_parser -> midi_domain::Song
                              |
                              v
                         midi_clock (song position)
                              |
                +-------------+-------------+
                |                           |
                v                           v
      piano_roll_model               note_matcher <--- midi_io (USB keyboard)
                |                           |
                +-------------+-------------+
                              v
                           app_core
                              v
                       ui_iced_widgets
                              v
                        iced desktop app
```

### Separation of concerns

- **Parsing layer**: "What is the song?"
- **Timing layer**: "Where are we in the song right now?"
- **Visualization layer**: "What should be drawn now?"
- **Input layer**: "What is the user playing now?"
- **Matching layer**: "How well did they match expected notes?"
- **App orchestration layer**: "How do features interact?"

---

## MIDI Timing and Piano Roll Considerations

### MIDI timing essentials

MIDI files store note timing in **ticks**, not seconds.
To render falling notes accurately, you must convert:

- `ticks -> beats` using PPQ (ticks per quarter note)
- `beats -> microseconds/seconds` using the **tempo map**
- `song time -> screen Y` using a scroll model

### Recommended scroll model (for Synthesia-style)

Use a target "hit line" near the keyboard and define a **lookahead window**:

- Example: notes visible 4 seconds before impact
- A note at `t_now + 2s` appears halfway between top and hit line

Formula idea:
- `y = hit_line_y - ((note_time - now) / lookahead_secs) * visible_height`

Benefits:
- Easy tempo scaling
- Deterministic and intuitive
- Works well with beat line overlays

### Tempo changes

Important for classical and expressive MIDI files.
Do not assume constant BPM.

Requirements:
- Precompute tempo segments for fast tick<->time mapping
- Test with files containing multiple tempo changes

### Visual density and performance

Large MIDI files can contain many notes.

Strategies:
- Pre-index notes by start time
- Only query visible range
- Cache note geometry per zoom level (future)
- Avoid allocations in render hot path where possible

---

## `iced` UI Strategy

### Why `iced` fits

- Rust-native UI
- Good cross-platform support
- Elm-ish update model works well for deterministic app state
- `canvas` allows custom drawing for piano roll

### Known challenges with `iced`

- High-frequency animation and custom rendering require careful redraw management
- Complex custom widgets can become verbose
- Audio/MIDI callback thread integration needs clean message passing

### Recommended UI architecture

- `iced_desktop` app owns:
  - top-level window lifecycle
  - async tasks/subscriptions
  - platform dialogs (file picker)
- `app_core` owns logic and state transitions
- `ui_iced_widgets` draws from immutable view data

### Rendering approach (phased)

1. **Phase 1**: simple canvas redraw every frame / tick (acceptable for MVP)
2. **Phase 2**: dirty-region/caching for keyboard and static grid
3. **Phase 3**: optional GPU-heavy optimization if needed

---


### Visual design style notes (flat UI + GPU-accelerated effects)

Target a **clean, flat, “pro app” UI** with a **neon, shader-driven piano roll** (inspired by Synthesia-style visuals) while keeping readability and performance as first-class constraints.

**Look & feel**
- Dark, matte background with subtle vignette; thin grid lines; clear lane separators.
- Minimal, modern controls: Play/Pause, BPM/tempo, timeline, track selection, device selector.
- Rounded corners, consistent spacing scale (8px rhythm), restrained typography.

**Note bars (the hero element)**
- Rounded-rectangle bars with emissive glow and soft bloom (avoid “over-glow” that kills readability).
- Optional left-hand / right-hand color split (e.g., cyan/blue vs magenta/purple) and per-track palettes.
- Subtle gradient edges + rim light to imply depth while staying “flat UI” overall.

**Impact / interaction effects**
- An “impact line” above the keyboard where notes land.
- Particle sparks on note hits (small bursts), plus optional trailing dust for fast runs.
- Soft shockwave/ripple ring shader on impact (toggleable).
- Keyboard keys highlight on press with color-matched glow; velocity can map to brightness.

**Post-processing (optional, toggleable)**
- Bloom (thresholded), vignette, slight motion blur for falling bars.
- Very subtle chromatic aberration only on highlights (easy to overdo).
- Keep all post effects behind user settings and provide “Performance / Balanced / Cinematic” presets.

**Performance & UX constraints**
- Effects must degrade gracefully: particles count caps, trail length caps, lower sample/bloom quality.
- Prefer GPU instancing for note bars; batch by lane/track; minimize per-frame allocations.
- Always prioritize timing accuracy and input latency over visuals.

**Implementation notes (phased)**
- **MVP:** Start with `iced::canvas` for drawing bars + keyboard (flat look, basic glow via layered shapes).
- **Phase 2:** Introduce a custom renderer path (wgpu) behind a feature flag for instancing + post-processing.
- **Phase 3:** Add particle system + post stack (bloom/ripple/trails), all guarded by settings and perf budgets.

## Hardware MIDI Input Plan (USB)

### MVP support

- Enumerate input ports
- Connect to one selected device
- Receive note on/off events
- Timestamp events on arrival (monotonic clock)
- Route into `note_matcher`

### Latency considerations (future-ready)

Different systems introduce input latency and UI delay.
Plan for:
- Input latency offset setting (ms)
- Calibration mode (tap test / guided exercise)
- Separate visual vs scoring offset if necessary

### Device compatibility notes

`midir` works well for common USB MIDI keyboards, but behavior differs by OS backend.
Abstract backend details early so platform quirks do not leak into app logic.

---

## Suggested Dependencies (Initial)

### Workspace-wide (common)
- `anyhow` / `thiserror`
- `tracing`, `tracing-subscriber`
- `serde`, `serde_json` / `toml`
- `smallvec` (optional optimization)

### MIDI and timing
- `midly` (MIDI parser)
- `midir` (MIDI I/O)
- `num-traits` (optional)

### Async / messaging
- `flume` or `crossbeam-channel`
- `tokio` only if truly needed (consider staying lightweight initially)

### GUI
- `iced`
- `rfd` (file dialogs, if desired)

### Testing
- `insta` (snapshot tests for view-models, optional)
- `proptest` (timing/mapping invariants)

---

## MVP Feature Scope (Recommended)

### Must-have (v0.1)

- [ ] Open MIDI file
- [ ] Parse notes and tempo map
- [ ] Piano roll visualization (falling bars)
- [ ] Keyboard hit line
- [ ] Play/pause/stop visual transport
- [ ] Song position scrubbing (basic seek)
- [ ] MIDI device list and connect
- [ ] Live key highlight from hardware input
- [ ] Basic note matching with tolerance window

### Nice-to-have (post-MVP but early)

- [ ] Tempo multiplier (practice speed)
- [ ] Loop region A-B
- [ ] Left/right hand track filtering
- [ ] Metronome click (visual first, audio later)
- [ ] Accuracy stats panel
- [ ] Recent files and saved settings

---

## Roadmap

## Phase 0 - Foundations (1-2 weeks)

**Goal:** Create a clean monorepo and compile all crates with placeholder APIs.

Deliverables:
- Workspace skeleton
- Shared lint/format config (`clippy`, `rustfmt`)
- `tracing` setup
- Minimal `iced_desktop` window
- Stub crates and traits

Success criteria:
- `cargo check --workspace` passes
- `cargo test --workspace` runs with basic smoke tests

---

## Phase 1 - MIDI Parsing + Domain (1-2 weeks)

**Goal:** Load MIDI and produce normalized song data.

Deliverables:
- `midi_domain` types
- `midi_parser` using `midly`
- Tempo map + note span extraction
- Test fixtures for simple songs and tempo changes

Success criteria:
- Parse multiple real MIDI files reliably
- Unit tests cover note pairing and tempo conversion basics

---

## Phase 2 - Clock + Piano Roll Model (1-2 weeks)

**Goal:** Convert song timing into stable renderable view data.

Deliverables:
- `midi_clock` transport + seek/play/pause
- `piano_roll_model` visible-range note projection
- Beat/measure line generation
- Deterministic simulation tests

Success criteria:
- Given a song position, render model is deterministic
- Tempo changes are reflected correctly in bar motion

---

## Phase 3 - Iced Piano Roll UI MVP (2-4 weeks)

**Goal:** See falling notes synchronized to transport in a desktop app.

Deliverables:
- `PianoRollCanvas` widget
- Keyboard visualization + hit line
- File open flow
- Playback controls (visual transport)
- Basic timeline scrub

Success criteria:
- User can open a MIDI and watch bars fall at correct tempo
- UI remains responsive on medium/large MIDI files

---

## Phase 4 - USB MIDI Input + Live Note Follow (1-3 weeks)

**Goal:** Connect keyboard and compare played notes.

Deliverables:
- `midi_io` device enumeration/connection
- Event stream into `app_core`
- `note_matcher` basic matching logic
- On-screen feedback (correct/late/early/miss)

Success criteria:
- User sees their pressed keys in real time
- Basic scoring works on simple melodies/chords

---

## Phase 5 - Practice Features (2-4 weeks)

**Goal:** Make it useful for daily practice.

Deliverables:
- Tempo multiplier
- Loop A-B
- Track mute/filter visualization
- Session stats summary
- Settings persistence

Success criteria:
- Repeatable practice flow without restarting app
- Preferences survive restarts

---

## Phase 6 - Performance & Polish (ongoing)

**Goal:** Improve rendering smoothness and UX.

Deliverables:
- Render optimizations and caching
- Reduced allocations in hot paths
- Better error dialogs and device recovery
- Visual polish/theme refinement
- Packaging (Linux/macOS/Windows)

Success criteria:
- Smooth playback on large songs
- Stable reconnect behavior for MIDI devices

---

## Testing Strategy

### Unit tests (high priority)
- Tempo map conversion correctness
- Tick<->time conversions
- Note pairing (on/off matching)
- Visible note range calculations
- Note matching tolerance windows

### Integration tests
- Load fixture MIDI -> produce expected domain model summary
- Simulated clock + simulated MIDI input -> expected scoring events

### Golden / snapshot tests (optional but useful)
- Piano roll view-model snapshots for known timestamps
- Stats output summaries

### Manual QA checklist
- Load small/large MIDI files
- Files with tempo changes
- Connect/disconnect MIDI device while app runs
- Pause/play/seek repeatedly
- Test on at least Linux + one other OS before release

---

## Developer Experience Recommendations

- Use a workspace `justfile` or `xtask` for common commands:
  - `fmt`, `clippy`, `test`, `run`, `fixtures`, `profile`
- Keep crate APIs small and explicit
- Prefer pure functions in `piano_roll_model` and `note_matcher`
- Add `tracing` spans around parser, transport tick, and render model generation
- Use feature flags for experimental modules

Example commands:

```bash
cargo run -p iced_desktop
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Future Extensions (Beyond MVP)

- Audio playback / synth integration
- MIDI output (lighting keyboards / accompaniment)
- Hands detection and fingering suggestions
- Practice modes (wait-for-correct-note, rhythm mode)
- Quantization and MIDI editing features
- WASM/web viewer (depending on `iced` + platform constraints)
- Multiple frontends (desktop + web) sharing `app_core`
- gRPC service backend for remote session storage / analytics (aligns with a backend-frontend architecture)

---

## Suggested First Implementation Slice (Very Practical)

If starting today, implement in this exact order:

1. `midi_domain` minimal types (`Song`, `NoteSpan`, `TempoMap`)
2. `midi_parser` parse one-track MIDI file into notes
3. `midi_clock` with play/pause and constant tempo first
4. `piano_roll_model` convert visible notes into `RenderNoteBar`
5. `iced_desktop` + `PianoRollCanvas` draw static bars
6. Wire clock updates to animate bars
7. Add real tempo map support
8. Add `midir` input + live key highlights
9. Add `note_matcher`

This sequence minimizes risk and gets visible progress early.

---

## Risks and Mitigations

### Risk: Timing drift / jitter between UI and MIDI input
Mitigation:
- Centralize monotonic time source
- Timestamp MIDI events on receipt
- Keep matching logic independent from render frame timing

### Risk: `iced` animation performance for dense songs
Mitigation:
- Model/view separation
- Visible range culling
- Cached static elements (grid/keyboard)
- Profile before over-optimizing

### Risk: MIDI file edge cases
Mitigation:
- Build fixture corpus early
- Log unsupported events without crashing
- Normalize parser output into strict domain model

### Risk: Over-scoping into DAW features
Mitigation:
- Freeze MVP scope around playback visualization + note following
- Track ideas in `docs/backlog.md`

---

## Deliverables Summary

This monorepo should initially produce:

- **Desktop app** (`apps/iced_desktop`) to load MIDI and display a falling piano roll
- **Reusable Rust crates** for parsing, timing, rendering model, MIDI I/O, and note matching
- **Tests and fixtures** validating timing correctness and matching behavior
- **A scalable architecture** that can later add audio, analytics, and additional frontends

---

## Next Actions (Immediate)

1. Create workspace + crate skeletons
2. Add `midly` and parse first sample MIDI into debug output
3. Define `RenderNoteBar` and a simple `iced` canvas renderer
4. Animate with a fake clock before real transport
5. Integrate `midir` device list and live key logging

Once these are working, the project will already have a strong vertical slice.
