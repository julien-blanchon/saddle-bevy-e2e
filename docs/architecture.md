# Architecture

## Overview

`saddle-bevy-e2e` is a frame-by-frame E2E testing framework for Bevy games. It runs inside the Bevy app loop, injecting scripted actions (key presses, mouse moves, waits, screenshots) each frame and capturing output for verification.

## Core Design

### Deterministic Timing

The plugin forces `TimeUpdateStrategy::ManualDuration(1/60s)` so that:
- Frame counts are deterministic regardless of real wall-clock time
- `Timer`-based game systems (fire rate, cooldowns) behave identically across runs
- Scenarios specified in frame counts produce reproducible results

### Execution Flow

```
E2EPlugin (Update, in E2ESet)
│
├── run_scenario (exclusive system)
│   ├── Read current Action from ScenarioRunner
│   ├── Execute action (inject input, take screenshot, wait, etc.)
│   └── Advance to next action when current completes
│
├── poll_and_stitch
│   ├── Poll async screenshot write tasks
│   └── Stitch video from frames when recording ends
│
└── exit_after_scenario
    └── Exit app when all actions complete
```

### Key Resources

| Resource | Purpose |
|----------|---------|
| `ScenarioRunner` | Tracks current action index, frame counters, completion state |
| `CaptureState` | Manages output directory, screenshot queue, video recording state |

### Action Model

`Action` is an enum representing a single frame-level step:

- **WaitFrames(n)** — Do nothing for n frames
- **WaitUntil(predicate)** — Poll a `&World` predicate each frame until true (with max frame timeout)
- **PressKey / ReleaseKey** — Inject keyboard input
- **MoveMouse / ClickMouse** — Inject mouse input
- **Screenshot(name)** — Capture current frame, downscale via ffmpeg
- **StartRecording / StopRecording** — Record frame sequences, stitch to MP4
- **Custom(closure)** — Arbitrary `&mut World` access for project-specific setup

### Scenario vs Snapshot

**Scenario**: Linear sequence of Actions. You script the full user journey frame by frame.

**Snapshot**: Declarative "set up world state, wait for settle, capture." Compiles down to a Scenario internally (setup action → wait frames → screenshot).

## Output

All output goes to `e2e_output/<scenario_name>/`:
- `*.png` — Named screenshots (downscaled to ~720p)
- `*.mp4` — Stitched video recordings
- `log.txt` — Frame-by-frame execution log

## Integration Pattern

```rust
// In your game binary (behind feature flag):
app.add_plugins(E2EPlugin);
app.configure_sets(Update, E2ESet.before(YourInputSet));

let scenario = my_scenario();
saddle_bevy_e2e::init_scenario(&mut app, scenario);
```

The `E2ESet` must run **before** your input-reading systems so that simulated key presses are visible in the same frame they're injected.
