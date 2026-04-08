# Saddle Bevy E2E

E2E visual testing framework for [Bevy](https://bevyengine.org/) games.

Provides a frame-by-frame scenario DSL, screenshot capture, video recording via ffmpeg, and declarative snapshot testing — all integrated with Bevy's ECS.

## Features

- **Scenario DSL** — Script player actions (key presses, mouse movement, waits) frame-by-frame with a fluent builder API
- **Snapshot testing** — Jump to a game state and photograph it without scripting input sequences
- **Screenshot capture** — Named screenshots, auto-downscaled to ~720p via ffmpeg
- **Video recording** — Record frame sequences and stitch into MP4 via ffmpeg
- **Deterministic timing** — Forces 60fps `ManualDuration` so frame-count-based scenarios are reproducible
- **Conditional waits** — `WaitUntil` polls a world condition each frame (e.g. "assets loaded")
- **Custom actions** — Arbitrary `&mut World` closures for project-specific logic

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
saddle-bevy-e2e = { git = "https://github.com/julien-blanchon/saddle-bevy-e2e", optional = true }

[features]
e2e = ["saddle-bevy-e2e", "bevy/bevy_dev_tools"]
```

### Define a scenario

```rust
use saddle_bevy_e2e::prelude::*;

fn my_scenario() -> Scenario {
    Scenario::builder("smoke_test")
        .description("Launch the game and screenshot the menu")
        .then(Action::WaitFrames(120))
        .then(Action::Screenshot("menu".into()))
        .then(Action::WaitFrames(30))
        .build()
}
```

### Wire it up

```rust
use bevy::prelude::*;
use saddle_bevy_e2e::prelude::*;

fn main() {
    let scenario = my_scenario();

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(E2EPlugin)
        // Order E2ESet before your input systems so injected
        // key presses are visible in the same frame.
        // .configure_sets(Update, E2ESet.before(MyGameSet::Input))
        .add_plugins(|app: &mut App| saddle_bevy_e2e::init_scenario(app, scenario))
        .run();
}
```

### Define a snapshot

```rust
use saddle_bevy_e2e::prelude::*;

fn snap_victory() -> Scenario {
    Snapshot::builder("snap_victory")
        .description("Victory screen with final score")
        .setup(|world| {
            // Inject state directly — no input scripting needed
            // world.insert_resource(GameResult { winner: 1 });
            // world.resource_mut::<NextState<Screen>>().set(Screen::Victory);
        })
        .settle(90)
        .capture("victory")
        .build()
        .into_scenario()
}
```

### Run

```bash
cargo run --features e2e -- my_scenario
```

Output goes to `e2e_output/<scenario_name>/` with screenshots, video, and a log file.

## Architecture

```
src/
├── lib.rs        # E2EPlugin, E2ESet, init_scenario, prelude
├── action.rs     # Action enum (WaitFrames, PressKey, Screenshot, ...)
├── capture.rs    # CaptureState, screenshot/video helpers
├── input.rs      # Input simulation (keyboard, mouse)
├── runner.rs     # ScenarioRunner exclusive system
├── scenario.rs   # Scenario + ScenarioBuilder
├── snapshot.rs   # Snapshot + SnapshotBuilder
└── actions/      # High-level reusable action helpers
    ├── mod.rs
    ├── assets.rs # wait_for_assets()
    └── camera.rs # Camera actions (placeholder)
```

### Key types

| Type | Role |
|------|------|
| `E2EPlugin` | Registers runner systems in `Update` schedule |
| `E2ESet` | SystemSet for ordering (place `.before(YourInputSet)`) |
| `Scenario` | Named sequence of `Action`s |
| `Snapshot` | Declarative state → capture (compiles to `Scenario`) |
| `Action` | Single step: wait, input, screenshot, record, custom |
| `ScenarioRunner` | Resource tracking execution state |
| `CaptureState` | Resource tracking output directory and recording |

### Ordering

The plugin adds all systems to `E2ESet`. You should configure this set to run **before** your input-reading systems so that simulated key presses are visible in the same frame:

```rust
app.configure_sets(Update, E2ESet.before(GameSet::Input));
```

## Requirements

- **Bevy 0.18**
- **ffmpeg** (optional) — Required for screenshot downscaling and video stitching. Falls back gracefully if not installed.

## Testing Note

This crate intentionally does **not** contain its own `examples/lab` app.
`saddle-bevy-e2e` is the reusable framework that powers the downstream lab apps in the runtime crates, so the most meaningful E2E verification for framework changes happens through representative consumer labs rather than a self-hosted lab here.

When changing capture, runner, input injection, or snapshot behavior, validate at least one representative downstream lab in addition to this crate's direct Rust-level checks and examples.

## License

MIT-0 — see [LICENSE](LICENSE).
