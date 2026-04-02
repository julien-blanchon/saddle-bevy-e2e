//! Assertion actions — soft pass/fail checks logged to the scenario output.
//!
//! Each assertion logs `[PASS]` or `[FAIL]` and updates an [`AssertionTracker`]
//! resource with cumulative counts. Use [`log_summary`] at the end of a
//! scenario to print the overall result.
//!
//! Assertions are **soft** — they log failures but do not abort the scenario,
//! so all checks run and you get the full picture. For hard assertions that
//! panic, use `Action::Custom` with `assert!` / `panic!`.
//!
//! # Examples
//!
//! ```rust,ignore
//! use bevy_e2e::actions::assertions;
//!
//! Scenario::builder("combat_test")
//!     .then(assertions::entity_exists::<Player>("player spawned"))
//!     .then(assertions::entity_count::<Enemy>("enemies", 5))
//!     .then(assertions::resource_satisfies::<Score>("score > 0", |s| s.value > 0))
//!     .then(assertions::log_summary("final"))
//!     .build()
//! ```

use bevy::prelude::*;

use crate::action::Action;
use crate::capture::CaptureState;
use crate::runner::ScenarioRunner;

// ---------------------------------------------------------------------------
// Assertion tracker resource
// ---------------------------------------------------------------------------

/// Tracks cumulative assertion pass/fail counts within a scenario.
///
/// Inserted lazily on the first assertion call.
#[derive(Resource, Default, Debug)]
pub struct AssertionTracker {
    /// Number of assertions that passed.
    pub passed: u32,
    /// Number of assertions that failed.
    pub failed: u32,
    /// Labels of failed assertions for the summary.
    pub failures: Vec<String>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

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

fn record_result(world: &mut World, passed: bool, label: &str) {
    if !world.contains_resource::<AssertionTracker>() {
        world.insert_resource(AssertionTracker::default());
    }
    let mut tracker = world.resource_mut::<AssertionTracker>();
    if passed {
        tracker.passed += 1;
    } else {
        tracker.failed += 1;
        tracker.failures.push(label.to_string());
    }
}

// ---------------------------------------------------------------------------
// Entity assertions
// ---------------------------------------------------------------------------

/// Assert that at least one entity with component `C` exists.
///
/// Output: `[frame N] [PASS] label: entity with TypeName exists`
/// or:     `[frame N] [FAIL] label: no entity with TypeName found`
pub fn entity_exists<C: Component>(label: &str) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let count = world.query_filtered::<(), With<C>>().iter(world).count();
        let type_name = short_name(std::any::type_name::<C>());
        let frame = current_frame(world);
        let passed = count > 0;

        record_result(world, passed, &label);
        if passed {
            log(
                world,
                format!(
                    "[frame {frame}] [PASS] {label}: entity with {type_name} exists ({count} found)"
                ),
            );
        } else {
            log(
                world,
                format!("[frame {frame}] [FAIL] {label}: no entity with {type_name} found"),
            );
        }
    }))
}

/// Assert that **no** entity with component `C` exists.
///
/// Useful for verifying cleanup (e.g. all enemies despawned).
pub fn no_entity<C: Component>(label: &str) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let count = world.query_filtered::<(), With<C>>().iter(world).count();
        let type_name = short_name(std::any::type_name::<C>());
        let frame = current_frame(world);
        let passed = count == 0;

        record_result(world, passed, &label);
        if passed {
            log(
                world,
                format!("[frame {frame}] [PASS] {label}: no entity with {type_name} (as expected)"),
            );
        } else {
            log(
                world,
                format!(
                    "[frame {frame}] [FAIL] {label}: expected 0 entities with {type_name}, found {count}"
                ),
            );
        }
    }))
}

/// Assert that the entity count with component `C` equals `expected`.
pub fn entity_count<C: Component>(label: &str, expected: usize) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let actual = world.query_filtered::<(), With<C>>().iter(world).count();
        let type_name = short_name(std::any::type_name::<C>());
        let frame = current_frame(world);
        let passed = actual == expected;

        record_result(world, passed, &label);
        if passed {
            log(
                world,
                format!("[frame {frame}] [PASS] {label}: {actual} entities with {type_name}"),
            );
        } else {
            log(
                world,
                format!(
                    "[frame {frame}] [FAIL] {label}: expected {expected} entities with {type_name}, found {actual}"
                ),
            );
        }
    }))
}

/// Assert that the entity count with component `C` is within `[min, max]` (inclusive).
pub fn entity_count_range<C: Component>(label: &str, min: usize, max: usize) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let actual = world.query_filtered::<(), With<C>>().iter(world).count();
        let type_name = short_name(std::any::type_name::<C>());
        let frame = current_frame(world);
        let passed = actual >= min && actual <= max;

        record_result(world, passed, &label);
        if passed {
            log(
                world,
                format!(
                    "[frame {frame}] [PASS] {label}: {actual} entities with {type_name} (range {min}..={max})"
                ),
            );
        } else {
            log(
                world,
                format!(
                    "[frame {frame}] [FAIL] {label}: expected {min}..={max} entities with {type_name}, found {actual}"
                ),
            );
        }
    }))
}

// ---------------------------------------------------------------------------
// Resource assertions
// ---------------------------------------------------------------------------

/// Assert that a resource satisfies a condition.
///
/// The `check` closure receives `&R` and must return `true` for the assertion
/// to pass. If the resource doesn't exist, the assertion fails.
///
/// # Example
///
/// ```rust,ignore
/// assertions::resource_satisfies::<Score>("score positive", |s| s.value > 0)
/// ```
pub fn resource_satisfies<R: Resource>(
    label: &str,
    check: impl Fn(&R) -> bool + Send + Sync + 'static,
) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let type_name = short_name(std::any::type_name::<R>());
        let frame = current_frame(world);

        let result = world.get_resource::<R>().map(&check);

        match result {
            Some(true) => {
                record_result(world, true, &label);
                log(
                    world,
                    format!("[frame {frame}] [PASS] {label}: {type_name} assertion passed"),
                );
            }
            Some(false) => {
                record_result(world, false, &label);
                log(
                    world,
                    format!("[frame {frame}] [FAIL] {label}: {type_name} assertion failed"),
                );
            }
            None => {
                record_result(world, false, &label);
                log(
                    world,
                    format!("[frame {frame}] [FAIL] {label}: resource {type_name} not found"),
                );
            }
        }
    }))
}

/// Assert that a resource exists.
pub fn resource_exists<R: Resource>(label: &str) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let type_name = short_name(std::any::type_name::<R>());
        let frame = current_frame(world);
        let passed = world.contains_resource::<R>();

        record_result(world, passed, &label);
        if passed {
            log(
                world,
                format!("[frame {frame}] [PASS] {label}: resource {type_name} exists"),
            );
        } else {
            log(
                world,
                format!("[frame {frame}] [FAIL] {label}: resource {type_name} not found"),
            );
        }
    }))
}

// ---------------------------------------------------------------------------
// Component assertions
// ---------------------------------------------------------------------------

/// Assert that component `C` on the first matching entity satisfies a condition.
///
/// Queries all entities with `C` and checks the first one found.
/// Fails if no entity with `C` exists.
///
/// # Example
///
/// ```rust,ignore
/// assertions::component_satisfies::<Health>("player alive", |h| h.current > 0)
/// ```
pub fn component_satisfies<C: Component>(
    label: &str,
    check: impl Fn(&C) -> bool + Send + Sync + 'static,
) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let type_name = short_name(std::any::type_name::<C>());
        let frame = current_frame(world);

        let result = {
            let mut query = world.query::<&C>();
            query.iter(world).next().map(&check)
        };

        match result {
            Some(true) => {
                record_result(world, true, &label);
                log(
                    world,
                    format!("[frame {frame}] [PASS] {label}: {type_name} assertion passed"),
                );
            }
            Some(false) => {
                record_result(world, false, &label);
                log(
                    world,
                    format!("[frame {frame}] [FAIL] {label}: {type_name} assertion failed"),
                );
            }
            None => {
                record_result(world, false, &label);
                log(
                    world,
                    format!("[frame {frame}] [FAIL] {label}: no entity with {type_name} found"),
                );
            }
        }
    }))
}

/// Assert that component `C` on the first entity with marker `M` satisfies a condition.
///
/// # Example
///
/// ```rust,ignore
/// assertions::component_where::<Health, Player>("player alive", |h| h.current > 0)
/// ```
pub fn component_where<C: Component, M: Component>(
    label: &str,
    check: impl Fn(&C) -> bool + Send + Sync + 'static,
) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let comp_name = short_name(std::any::type_name::<C>());
        let marker_name = short_name(std::any::type_name::<M>());
        let frame = current_frame(world);

        let result = {
            let mut query = world.query_filtered::<&C, With<M>>();
            query.iter(world).next().map(&check)
        };

        match result {
            Some(true) => {
                record_result(world, true, &label);
                log(
                    world,
                    format!(
                        "[frame {frame}] [PASS] {label}: {comp_name} on {marker_name} assertion passed"
                    ),
                );
            }
            Some(false) => {
                record_result(world, false, &label);
                log(
                    world,
                    format!(
                        "[frame {frame}] [FAIL] {label}: {comp_name} on {marker_name} assertion failed"
                    ),
                );
            }
            None => {
                record_result(world, false, &label);
                log(
                    world,
                    format!("[frame {frame}] [FAIL] {label}: no entity with {marker_name} found"),
                );
            }
        }
    }))
}

// ---------------------------------------------------------------------------
// Custom assertion
// ---------------------------------------------------------------------------

/// Assert that a custom world condition holds.
///
/// The `check` closure has full `&World` access and returns `true` for pass.
///
/// # Example
///
/// ```rust,ignore
/// assertions::custom("no orphan transforms", |world| {
///     let mut query = world.query_filtered::<(), (With<Transform>, Without<Parent>, Without<Camera>)>();
///     query.iter(world).count() < 5
/// })
/// ```
pub fn custom(label: &str, check: impl Fn(&World) -> bool + Send + Sync + 'static) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let frame = current_frame(world);
        let passed = check(world);

        record_result(world, passed, &label);
        if passed {
            log(world, format!("[frame {frame}] [PASS] {label}"));
        } else {
            log(world, format!("[frame {frame}] [FAIL] {label}"));
        }
    }))
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

/// Log the cumulative assertion pass/fail summary.
///
/// Place this at the end of a scenario to get a clear result.
///
/// Output:
/// ```text
/// [frame N] [assertions] label: 5 passed, 2 FAILED
///   Failed: player alive, enemies spawned
/// ```
pub fn log_summary(label: &str) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let frame = current_frame(world);
        let (passed, failed, failures) = world
            .get_resource::<AssertionTracker>()
            .map(|t| (t.passed, t.failed, t.failures.clone()))
            .unwrap_or((0, 0, vec![]));

        let total = passed + failed;
        let status = if failed == 0 {
            "ALL PASSED"
        } else {
            "HAS FAILURES"
        };

        let mut msg = format!(
            "[frame {frame}] [assertions] {label}: {status} ({passed}/{total} passed, {failed} failed)"
        );

        if !failures.is_empty() {
            msg.push_str(&format!("\n  Failed: {}", failures.join(", ")));
        }

        log(world, msg);
    }))
}
