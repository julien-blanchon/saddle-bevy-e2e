//! Scenario runner — exclusive system that processes actions frame-by-frame.

use bevy::prelude::*;

use bevy::time::TimeUpdateStrategy;

use crate::action::Action;
use crate::capture::{self, CaptureState};
use crate::input;
use crate::scenario::Scenario;

/// Resource holding the active scenario and execution state.
#[derive(Resource)]
pub struct ScenarioRunner {
    /// Remaining actions to execute (front = next).
    actions: Vec<Action>,
    /// Current frame counter within the active wait/hold.
    wait_remaining: u32,
    /// Keys currently held by HoldKey actions (key, frames remaining).
    held_keys: Vec<(KeyCode, u32)>,
    /// Total frame count since scenario start.
    pub total_frames: u32,
    /// Whether the scenario has completed.
    pub finished: bool,
    /// When true, the game keeps running after scenario completion instead of exiting.
    /// Set by `Action::Handoff` — restores real-time and leaves the game live for BRP.
    pub handoff: bool,
    /// Scenario name (for logging).
    pub name: String,
    /// Active `WaitUntil` being polled (condition, label, frames remaining).
    pending_wait_until: Option<(Box<dyn Fn(&World) -> bool + Send + Sync>, String, u32)>,
}

impl ScenarioRunner {
    /// Create a new runner from a scenario.
    pub fn from_scenario(scenario: Scenario) -> (Self, CaptureState) {
        let capture = CaptureState::new(&scenario.name);
        let mut runner = Self {
            actions: scenario.actions,
            wait_remaining: 0,
            held_keys: Vec::new(),
            total_frames: 0,
            finished: false,
            handoff: false,
            name: scenario.name,
            pending_wait_until: None,
        };
        // Reverse so we can pop from the back efficiently
        runner.actions.reverse();
        (runner, capture)
    }
}

/// Internal result of polling a `WaitUntil` condition.
enum PollResult {
    Satisfied(String),
    TimedOut(String),
    Pending,
}

/// Exclusive system that advances the scenario one step per frame.
///
/// Must be added to `Update` schedule. Uses `&mut World` for maximum
/// flexibility (can manipulate any entity/resource directly).
pub fn run_scenario(world: &mut World) {
    // Early exit if runner doesn't exist or is finished
    let finished = world
        .get_resource::<ScenarioRunner>()
        .map(|r| r.finished)
        .unwrap_or(true);
    if finished {
        return;
    }

    // Increment frame counter
    world.resource_mut::<ScenarioRunner>().total_frames += 1;

    // Tick held keys
    tick_held_keys(world);

    // If we're waiting, decrement and return
    let waiting = {
        let mut runner = world.resource_mut::<ScenarioRunner>();
        if runner.wait_remaining > 0 {
            runner.wait_remaining -= 1;
            true
        } else {
            false
        }
    };
    if waiting {
        maybe_record_frame(world);
        return;
    }

    // If we have a pending WaitUntil, poll it
    let poll_result = {
        let runner = world.resource::<ScenarioRunner>();
        if let Some((ref condition, ref label, remaining)) = runner.pending_wait_until {
            if condition(world) {
                Some(PollResult::Satisfied(label.clone()))
            } else if remaining == 0 {
                Some(PollResult::TimedOut(label.clone()))
            } else {
                Some(PollResult::Pending)
            }
        } else {
            None
        }
    };
    if let Some(result) = poll_result {
        match result {
            PollResult::Satisfied(label) => {
                let frame = world.resource::<ScenarioRunner>().total_frames;
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] WaitUntil({label:?}) satisfied"));
                world.resource_mut::<ScenarioRunner>().pending_wait_until = None;
                // Fall through to process next action
            }
            PollResult::TimedOut(label) => {
                let frame = world.resource::<ScenarioRunner>().total_frames;
                warn!("[e2e] WaitUntil({label:?}) timed out at frame {frame} — continuing anyway");
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] WaitUntil({label:?}) TIMED OUT"));
                world.resource_mut::<ScenarioRunner>().pending_wait_until = None;
                // Fall through to process next action
            }
            PollResult::Pending => {
                // Decrement remaining frames
                world
                    .resource_mut::<ScenarioRunner>()
                    .pending_wait_until
                    .as_mut()
                    .unwrap()
                    .2 -= 1;
                maybe_record_frame(world);
                return; // Blocking — check again next frame
            }
        }
    }

    // Process next action(s) — some actions are instant and we process
    // multiple in the same frame until we hit a blocking action (wait/hold).
    loop {
        let action = {
            let mut runner = world.resource_mut::<ScenarioRunner>();
            runner.actions.pop()
        };

        let Some(action) = action else {
            // No more actions — scenario complete
            finish_scenario(world);
            return;
        };

        let frame = world.resource::<ScenarioRunner>().total_frames;

        match action {
            Action::WaitFrames(n) => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] WaitFrames({n})"));
                if n > 0 {
                    world.resource_mut::<ScenarioRunner>().wait_remaining = n - 1;
                    maybe_record_frame(world);
                    return; // Blocking — resume next frame
                }
            }

            Action::PressKey(key) => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] PressKey({key:?})"));
                input::simulate_key_press(world, key);
            }

            Action::ReleaseKey(key) => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] ReleaseKey({key:?})"));
                input::simulate_key_release(world, key);
            }

            Action::HoldKey { key, frames } => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] HoldKey({key:?}, {frames})"));
                input::simulate_key_press(world, key);
                world
                    .resource_mut::<ScenarioRunner>()
                    .held_keys
                    .push((key, frames));
                // Blocking — the key is held across frames
                world.resource_mut::<ScenarioRunner>().wait_remaining = frames.saturating_sub(1);
                maybe_record_frame(world);
                return;
            }

            Action::MouseMotion { delta } => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] MouseMotion({delta})"));
                input::simulate_mouse_motion(world, delta);
            }

            Action::MouseScroll { delta } => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] MouseScroll({delta})"));
                input::simulate_mouse_scroll(world, delta);
            }

            Action::PressMouseButton(button) => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] PressMouseButton({button:?})"));
                input::simulate_mouse_press(world, button);
            }

            Action::ReleaseMouseButton(button) => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] ReleaseMouseButton({button:?})"));
                input::simulate_mouse_release(world, button);
            }

            Action::Screenshot(name) => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] Screenshot({name:?})"));
                // Need to extract capture state data, then use commands
                let capture = world.resource::<CaptureState>();
                let capture_clone_output_dir = capture.output_dir.clone();
                let capture_state_for_screenshot = CaptureState {
                    output_dir: capture_clone_output_dir,
                    recording: capture.recording,
                    record_frame_index: capture.record_frame_index,
                    log_lines: Vec::new(),
                    pending_stitch: false,
                    expected_frame_count: 0,
                    stitch_complete: false,
                    last_file_count: 0,
                    stable_count: 0,
                    stitch_done_flag: None,
                };
                let mut commands = world.commands();
                capture::take_screenshot(&mut commands, &capture_state_for_screenshot, &name);
            }

            Action::StartRecording => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] StartRecording"));
                let mut capture = world.resource_mut::<CaptureState>();
                capture.recording = true;
                capture.record_frame_index = 0;
                capture.ensure_dirs();
            }

            Action::StopRecording => {
                let frame_count = world.resource::<CaptureState>().record_frame_index;
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] StopRecording ({frame_count} frames queued, waiting for async writes)"));
                let mut capture = world.resource_mut::<CaptureState>();
                capture.recording = false;
                capture.pending_stitch = true;
                capture.expected_frame_count = frame_count;
            }

            Action::Log(msg) => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] {msg}"));
            }

            Action::WaitUntil {
                label,
                condition,
                max_frames,
            } => {
                world.resource_mut::<CaptureState>().log(format!(
                    "[frame {frame}] WaitUntil({label:?}, max={max_frames})"
                ));
                // Check immediately — might already be satisfied
                if condition(world) {
                    world.resource_mut::<CaptureState>().log(format!(
                        "[frame {frame}] WaitUntil({label:?}) satisfied immediately"
                    ));
                    // Non-blocking — continue to next action
                } else {
                    // Store for polling on subsequent frames
                    world.resource_mut::<ScenarioRunner>().pending_wait_until =
                        Some((condition, label, max_frames));
                    maybe_record_frame(world);
                    return; // Blocking — poll next frame
                }
            }

            Action::Handoff => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] Handoff — keeping game running"));
                world.resource_mut::<ScenarioRunner>().handoff = true;
                // Drain remaining actions — handoff is terminal
                world.resource_mut::<ScenarioRunner>().actions.clear();
                finish_scenario(world);
                return;
            }

            Action::Custom(f) => {
                world
                    .resource_mut::<CaptureState>()
                    .log(format!("[frame {frame}] Custom(...)"));
                f(world);
            }
        }
        // Non-blocking actions: continue to next action in the same frame
    }
}

/// Tick held keys — decrement frame counters and release when done.
///
/// A key is released when its frame counter reaches zero — either it was already
/// zero at the start of the tick (safety guard) or it decremented TO zero this tick.
fn tick_held_keys(world: &mut World) {
    let expired: Vec<KeyCode> = {
        let mut runner = world.resource_mut::<ScenarioRunner>();
        let mut expired = Vec::new();
        for (key, frames) in &mut runner.held_keys {
            if *frames == 0 {
                // Already at zero — release now (safety guard)
                expired.push(*key);
            } else {
                *frames -= 1;
                if *frames == 0 {
                    // Decremented to zero this tick — release the key
                    expired.push(*key);
                }
            }
        }
        runner.held_keys.retain(|(_, f)| *f > 0);
        expired
    };

    for key in expired {
        input::simulate_key_release(world, key);
    }
}

/// If recording is active, capture a frame.
fn maybe_record_frame(world: &mut World) {
    let recording = world.resource::<CaptureState>().recording;
    if recording {
        let mut capture = world.resource_mut::<CaptureState>();
        capture.ensure_dirs();
        let path = capture
            .frames_dir()
            .join(format!("frame_{:05}.png", capture.record_frame_index));
        capture.record_frame_index += 1;

        let mut commands = world.commands();
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path));
    }
}

/// Finalize the scenario: flush log, mark finished.
///
/// When `handoff` is true, restores real-time progression and logs that the
/// game will keep running for interactive debugging (e.g. via BRP).
fn finish_scenario(world: &mut World) {
    let handoff = world.resource::<ScenarioRunner>().handoff;
    {
        let total_frames = world.resource::<ScenarioRunner>().total_frames;
        let name = world.resource::<ScenarioRunner>().name.clone();
        let mut capture = world.resource_mut::<CaptureState>();
        capture.log(format!(
            "Scenario '{name}' completed after {total_frames} frames"
        ));
        if handoff {
            capture.log(String::from(
                "Handoff: game will keep running for interactive debugging",
            ));
        }
        capture.flush_log();
    }
    world.resource_mut::<ScenarioRunner>().finished = true;

    if handoff {
        // Restore real-time so the game runs at normal speed for interactive use
        world.insert_resource(TimeUpdateStrategy::Automatic);
        info!("[e2e] Scenario finished — handoff mode, game keeps running (connect via BRP)");
    } else {
        info!("[e2e] Scenario finished — scheduling exit");
    }
}

/// System that polls for async frame writes to stabilize, then stitches on a background thread.
///
/// Bevy's render pipeline drops occasional screenshots under load, so we can't
/// wait for an exact count.  Instead we wait for the file count to stop growing
/// for `STABLE_FRAMES_THRESHOLD` consecutive frames, then stitch.
pub fn poll_and_stitch(mut capture: Option<ResMut<CaptureState>>) {
    let Some(ref mut capture) = capture else {
        return;
    };

    // Phase 1: waiting for frame writes to stabilize
    if capture.pending_stitch && capture.stitch_done_flag.is_none() {
        let current_count = capture::count_frame_files(capture);
        if current_count != capture.last_file_count {
            // Still growing — reset stability counter
            capture.last_file_count = current_count;
            capture.stable_count = 0;
        } else {
            capture.stable_count += 1;
        }

        if capture.stable_count >= capture::STABLE_FRAMES_THRESHOLD && current_count > 0 {
            info!(
                "[e2e] Frame writes stabilized: {current_count}/{} files on disk — stitching video on background thread",
                capture.expected_frame_count
            );
            // Launch stitch on a background thread
            let done_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            capture.stitch_done_flag = Some(done_flag.clone());

            let output_dir = capture.output_dir.clone();
            let frames_dir = capture.frames_dir();
            std::thread::spawn(move || {
                // Build a temporary CaptureState just for stitch_video
                let stitch_capture = CaptureState {
                    output_dir,
                    recording: false,
                    record_frame_index: 0,
                    log_lines: Vec::new(),
                    pending_stitch: false,
                    expected_frame_count: 0,
                    stitch_complete: false,
                    last_file_count: 0,
                    stable_count: 0,
                    stitch_done_flag: None,
                };
                capture::stitch_video(&stitch_capture);
                // Clean up frames after stitching
                if frames_dir.exists() {
                    let _ = std::fs::remove_dir_all(&frames_dir);
                }
                done_flag.store(true, std::sync::atomic::Ordering::Release);
            });
        }
        return;
    }

    // Phase 2: waiting for background stitch thread to finish
    if let Some(ref flag) = capture.stitch_done_flag
        && flag.load(std::sync::atomic::Ordering::Acquire)
    {
        info!("[e2e] Background stitch complete");
        capture.pending_stitch = false;
        capture.stitch_complete = true;
        capture.stitch_done_flag = None;
    }
}

/// System that exits the app after the scenario finishes and video stitching is done.
///
/// When `handoff` is true, this system does nothing — the game keeps running.
pub fn exit_after_scenario(
    runner: Option<Res<ScenarioRunner>>,
    capture: Option<Res<CaptureState>>,
    mut delay: Local<Option<u32>>,
) {
    let Some(runner) = runner else { return };
    if !runner.finished {
        return;
    }

    // Handoff mode — keep the game running, don't exit
    if runner.handoff {
        return;
    }

    // Wait for video stitching to finish before starting exit countdown
    if let Some(ref capture) = capture
        && capture.pending_stitch
    {
        return; // Still waiting for frames / stitching
    }

    let frames = delay.get_or_insert(120);
    if *frames == 0 {
        info!("[e2e] Exiting application");
        std::process::exit(0);
    } else {
        *frames -= 1;
    }
}
