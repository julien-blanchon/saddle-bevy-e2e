//! E2E visual testing framework for Bevy.
//!
//! Provides a scenario DSL for scripting game actions frame-by-frame,
//! screenshot and video capture with ffmpeg post-processing, and
//! declarative snapshot testing.
//!
//! # Usage
//!
//! ```bash
//! cargo run --features e2e -- my_scenario
//! ```

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

pub mod action;
pub mod actions;
pub mod capture;
pub mod input;
pub mod runner;
pub mod scenario;
pub mod snapshot;

use runner::ScenarioRunner;
use scenario::Scenario;

/// Prelude — import everything you need for E2E testing.
pub mod prelude {
    pub use crate::action::Action;
    pub use crate::capture::CaptureState;
    pub use crate::runner::ScenarioRunner;
    pub use crate::scenario::{Scenario, ScenarioBuilder};
    pub use crate::snapshot::{Snapshot, SnapshotBuilder};
    pub use crate::{E2EPlugin, E2ESet, init_scenario};
}

/// SystemSet for all E2E runner systems.
///
/// Configure this set to run **before** your game's input-reading systems
/// so that simulated key presses are visible in the same frame:
///
/// ```rust,ignore
/// app.configure_sets(Update, E2ESet.before(GameSet::Input));
/// ```
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct E2ESet;

/// Plugin that registers the E2E runner systems.
///
/// After adding this plugin, call [`init_scenario`] to set up the scenario to run.
pub struct E2EPlugin;

impl Plugin for E2EPlugin {
    fn build(&self, app: &mut App) {
        // Force deterministic 60fps time progression so that frame-count-based
        // scenario timing matches timer-based game systems (weapon fire rate,
        // reload, etc.). Without this, Time::delta() uses real wall-clock time
        // and frame counts become non-deterministic.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            1.0 / 60.0,
        )));

        // Chain: run_scenario (exclusive) executes first, then poll_and_stitch waits for
        // async frame writes before stitching video, then exit_after_scenario handles exit.
        app.add_systems(
            Update,
            (
                runner::run_scenario,
                runner::poll_and_stitch,
                runner::exit_after_scenario,
            )
                .chain()
                .in_set(E2ESet),
        );
    }
}

/// Initialize the E2E runner resources from a scenario.
/// Call this after adding the `E2EPlugin` to the app.
pub fn init_scenario(app: &mut App, scenario: Scenario) {
    let (runner, capture) = ScenarioRunner::from_scenario(scenario);
    app.insert_resource(runner);
    app.insert_resource(capture);
}
