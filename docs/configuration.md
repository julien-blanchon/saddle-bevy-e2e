# Configuration

## Plugin Setup

```rust
use saddle_bevy_e2e::prelude::*;

app.add_plugins(E2EPlugin);
```

The plugin automatically:
- Sets `TimeUpdateStrategy::ManualDuration(1/60s)` for deterministic timing
- Registers runner systems in `E2ESet`

## System Ordering

Place `E2ESet` before your input systems:

```rust
app.configure_sets(Update, E2ESet.before(GameSet::Input));
```

## Scenario Builder

```rust
let scenario = Scenario::builder("my_test")
    .description("Optional description")
    .then(Action::WaitFrames(60))          // Wait 1 second (60fps)
    .then(Action::PressKey(KeyCode::KeyW)) // Press W
    .then(Action::WaitFrames(30))
    .then(Action::ReleaseKey(KeyCode::KeyW))
    .then(Action::Screenshot("after_move".into()))
    .build();
```

## Snapshot Builder

```rust
let snapshot = Snapshot::builder("victory_screen")
    .description("Verify victory UI layout")
    .setup(|world| {
        // Directly manipulate world state
    })
    .settle(90) // Wait 90 frames for animations to settle
    .capture("final_state")
    .build();

// Convert to scenario for the runner
let scenario = snapshot.into_scenario();
```

## Action Reference

| Action | Description |
|--------|-------------|
| `WaitFrames(n)` | Wait n frames |
| `WaitUntil(predicate, max_frames)` | Wait until condition is true or timeout |
| `PressKey(KeyCode)` | Simulate key press |
| `ReleaseKey(KeyCode)` | Simulate key release |
| `MoveMouse(Vec2)` | Move mouse to position |
| `ClickMouse(MouseButton)` | Click mouse button |
| `Screenshot(name)` | Capture screenshot |
| `StartRecording` | Begin video recording |
| `StopRecording` | End recording, stitch MP4 |
| `Custom(closure)` | Arbitrary `&mut World` access |

## Output Directory

Default: `e2e_output/<scenario_name>/`

Contents:
- `*.png` — Screenshots (auto-downscaled to ~720p via ffmpeg)
- `*.mp4` — Video recordings
- `log.txt` — Execution log

## Feature Flags

| Feature | Description |
|---------|-------------|
| `json` | Enables `serde` + `serde_json` for scenario serialization |

## Requirements

- **Bevy 0.18**
- **ffmpeg** (optional) — For screenshot downscaling and video stitching. The framework falls back gracefully if ffmpeg is not installed (raw resolution PNGs, no video).
