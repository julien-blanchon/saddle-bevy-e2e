//! Snapshot — declarative state-based visual capture.
//!
//! Unlike [`Scenario`] which scripts frame-by-frame player actions,
//! a [`Snapshot`] jumps directly into a target game state and photographs it.
//!
//! Use snapshots when you want to verify *what something looks like*
//! without caring about *how the user got there*.
//!
//! # Example
//!
//! ```rust,ignore
//! Snapshot::builder("snap_victory")
//!     .description("Victory screen with P1 winning at 10 VP")
//!     .setup(|world| {
//!         // Inject all state needed to render the target screen
//!         world.insert_resource(VictoryResult { /* ... */ });
//!         world.resource_mut::<NextState<Screen>>().set(Screen::GameOver);
//!     })
//!     .settle(90)
//!     .capture("victory_default")
//!     .build()
//! ```
//!
//! A `Snapshot` compiles down to a [`Scenario`] via [`Snapshot::into_scenario`],
//! so the existing runner, capture, and exit infrastructure is fully reused.

use crate::action::Action;
use crate::scenario::Scenario;
use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Snapshot type
// ---------------------------------------------------------------------------

/// A declarative visual capture — set up state, wait, photograph.
pub struct Snapshot {
    name: String,
    description: String,
    steps: Vec<SnapshotStep>,
}

/// Internal step in a snapshot sequence.
enum SnapshotStep {
    /// Mutate the world (inject resources, transition states, configure camera).
    Setup(Box<dyn FnOnce(&mut World) + Send + Sync>),
    /// Wait N frames for transitions and rendering to settle.
    Settle(u32),
    /// Take a named screenshot.
    Capture(String),
    /// Begin video recording.
    StartRecording,
    /// Stop video recording and stitch.
    StopRecording,
    /// Log a message.
    Log(String),
    /// Raw action pass-through (e.g. WaitUntil).
    RawAction(Action),
}

impl Snapshot {
    /// Start building a snapshot with the given name.
    ///
    /// Convention: use `snap_` prefix for snapshot names (e.g. `snap_victory`).
    pub fn builder(name: impl Into<String>) -> SnapshotBuilder {
        SnapshotBuilder::new(name)
    }

    /// Convert this snapshot into a [`Scenario`] that the existing runner can execute.
    ///
    /// The generated scenario has the structure:
    /// 1. Log: `Snapshot: <name>`
    /// 2. For each step: Setup → Settle → Capture (repeating)
    /// 3. Log: "Snapshot complete"
    pub fn into_scenario(self) -> Scenario {
        let mut actions = Vec::new();

        actions.push(Action::Log(format!("Snapshot: {}", self.name)));

        for step in self.steps {
            match step {
                SnapshotStep::Setup(f) => {
                    actions.push(Action::Custom(f));
                }
                SnapshotStep::Settle(n) => {
                    actions.push(Action::WaitFrames(n));
                }
                SnapshotStep::Capture(name) => {
                    actions.push(Action::Screenshot(name));
                }
                SnapshotStep::StartRecording => {
                    actions.push(Action::StartRecording);
                }
                SnapshotStep::StopRecording => {
                    actions.push(Action::StopRecording);
                }
                SnapshotStep::Log(msg) => {
                    actions.push(Action::Log(msg));
                }
                SnapshotStep::RawAction(action) => {
                    actions.push(action);
                }
            }
        }

        actions.push(Action::Log("Snapshot complete".into()));

        Scenario {
            name: self.name,
            description: self.description,
            actions,
        }
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Fluent builder for constructing [`Snapshot`]s.
pub struct SnapshotBuilder {
    name: String,
    description: String,
    steps: Vec<SnapshotStep>,
}

impl SnapshotBuilder {
    /// Create a new builder with the given snapshot name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            steps: Vec::new(),
        }
    }

    /// Set the snapshot description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Add a world-mutation step (state transitions, resource injection, camera config).
    ///
    /// Multiple `setup` calls are allowed — they execute in order.
    pub fn setup(mut self, f: impl FnOnce(&mut World) + Send + Sync + 'static) -> Self {
        self.steps.push(SnapshotStep::Setup(Box::new(f)));
        self
    }

    /// Wait N frames for state transitions and rendering to settle.
    ///
    /// Call this after `setup()` to give Bevy time to process state changes,
    /// spawn entities, and render the first frame.
    ///
    /// If not called explicitly, no automatic settle is inserted — you control the timing.
    pub fn settle(mut self, frames: u32) -> Self {
        self.steps.push(SnapshotStep::Settle(frames));
        self
    }

    /// Take a named screenshot at the current point.
    pub fn capture(mut self, name: impl Into<String>) -> Self {
        self.steps.push(SnapshotStep::Capture(name.into()));
        self
    }

    /// Convenience: setup + settle + capture in one call.
    ///
    /// Equivalent to `.setup(f).settle(settle_frames).capture(name)`.
    /// Use for single-shot snapshots or when adding additional viewpoints.
    pub fn setup_and_capture(
        self,
        name: impl Into<String>,
        settle_frames: u32,
        f: impl FnOnce(&mut World) + Send + Sync + 'static,
    ) -> Self {
        self.setup(f).settle(settle_frames).capture(name)
    }

    /// Add a subsequent world-mutation step (e.g. move camera for another angle).
    ///
    /// Alias for `setup()` — reads better after initial setup for additional viewpoints:
    /// ```rust,ignore
    /// .setup(|w| { /* initial state */ })
    /// .settle(90)
    /// .capture("view_1")
    /// .then_setup(|w| { /* move camera */ })
    /// .settle(60)
    /// .capture("view_2")
    /// ```
    pub fn then_setup(self, f: impl FnOnce(&mut World) + Send + Sync + 'static) -> Self {
        self.setup(f)
    }

    /// Begin video recording.
    pub fn record(mut self) -> Self {
        self.steps.push(SnapshotStep::StartRecording);
        self
    }

    /// Stop video recording and stitch.
    pub fn stop_record(mut self) -> Self {
        self.steps.push(SnapshotStep::StopRecording);
        self
    }

    /// Add a log message.
    pub fn log(mut self, msg: impl Into<String>) -> Self {
        self.steps.push(SnapshotStep::Log(msg.into()));
        self
    }

    /// Inject a raw [`Action`] (e.g. `WaitUntil` for asset loading).
    ///
    /// Use this for actions that don't map to snapshot-specific steps.
    pub fn action(mut self, action: Action) -> Self {
        self.steps.push(SnapshotStep::RawAction(action));
        self
    }

    /// End the snapshot but keep the game running for interactive debugging.
    ///
    /// Appends a `Handoff` action as the last step — the game continues at
    /// real-time speed after the snapshot completes, ready for BRP connection.
    pub fn handoff(mut self) -> Self {
        self.steps.push(SnapshotStep::RawAction(Action::Handoff));
        self
    }

    /// Finalize and return the [`Snapshot`].
    pub fn build(self) -> Snapshot {
        Snapshot {
            name: self.name,
            description: self.description,
            steps: self.steps,
        }
    }
}
