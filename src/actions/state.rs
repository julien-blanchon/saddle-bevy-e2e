//! State inspection and assertion helpers.
//!
//! # Examples
//!
//! ```rust,ignore
//! use bevy_e2e::actions::state;
//!
//! Scenario::builder("menu_test")
//!     .then(state::log_state::<Screen>("initial state"))
//!     // ... transition ...
//!     .then(state::assert_state::<Screen>("in gameplay", Screen::Gameplay))
//!     .build()
//! ```

use bevy::prelude::*;

use crate::action::Action;
use crate::capture::CaptureState;
use crate::runner::ScenarioRunner;

fn short_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

fn current_frame(world: &World) -> u32 {
    world
        .get_resource::<ScenarioRunner>()
        .map(|r| r.total_frames)
        .unwrap_or(0)
}

fn log(world: &mut World, msg: String) {
    info!("[e2e] {msg}");
    if let Some(mut capture) = world.get_resource_mut::<CaptureState>() {
        capture.log(msg);
    }
}

/// Log the current value of state `S`.
///
/// Output: `[frame N] [state] label: Screen = Gameplay`
pub fn log_state<S: States + std::fmt::Debug>(label: &str) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let frame = current_frame(world);
        let type_name = short_name(std::any::type_name::<S>());

        match world.get_resource::<State<S>>() {
            Some(state) => {
                log(
                    world,
                    format!(
                        "[frame {frame}] [state] {label}: {type_name} = {:?}",
                        state.get()
                    ),
                );
            }
            None => {
                log(
                    world,
                    format!("[frame {frame}] [state] {label}: {type_name} NOT REGISTERED"),
                );
            }
        }
    }))
}

/// Assert that state `S` equals `expected`.
///
/// Uses the [`AssertionTracker`](super::assertions::AssertionTracker)
/// to record pass/fail.
///
/// Output: `[frame N] [PASS] label: Screen = Gameplay`
/// or:     `[frame N] [FAIL] label: expected Screen = Gameplay, got Menu`
pub fn assert_state<S: States + std::fmt::Debug + PartialEq + Send + Sync + 'static>(
    label: &str,
    expected: S,
) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let frame = current_frame(world);
        let type_name = short_name(std::any::type_name::<S>());

        let result = world
            .get_resource::<State<S>>()
            .map(|state| state.get() == &expected);

        // Import tracker utilities from assertions module
        use super::assertions::AssertionTracker;

        if !world.contains_resource::<AssertionTracker>() {
            world.insert_resource(AssertionTracker::default());
        }

        match result {
            Some(true) => {
                world.resource_mut::<AssertionTracker>().passed += 1;
                log(
                    world,
                    format!("[frame {frame}] [PASS] {label}: {type_name} = {expected:?}"),
                );
            }
            Some(false) => {
                let actual = world.resource::<State<S>>().get().clone();
                {
                    let mut tracker = world.resource_mut::<AssertionTracker>();
                    tracker.failed += 1;
                    tracker.failures.push(label.clone());
                }
                log(
                    world,
                    format!(
                        "[frame {frame}] [FAIL] {label}: expected {type_name} = {expected:?}, got {actual:?}"
                    ),
                );
            }
            None => {
                {
                    let mut tracker = world.resource_mut::<AssertionTracker>();
                    tracker.failed += 1;
                    tracker.failures.push(label.clone());
                }
                log(
                    world,
                    format!("[frame {frame}] [FAIL] {label}: state {type_name} not registered"),
                );
            }
        }
    }))
}
