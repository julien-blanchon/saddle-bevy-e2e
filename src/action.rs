//! Core action vocabulary for E2E scenarios.

use bevy::prelude::*;

/// A single step in an E2E scenario.
///
/// Core actions are generic and reusable across any Bevy project.
/// Project-specific behavior should use `Custom` with a world-mutating closure.
pub enum Action {
    // --- Time ---
    /// Wait for a given number of frames before advancing.
    WaitFrames(u32),

    // --- Input simulation ---
    /// Simulate a key press (held until `ReleaseKey`).
    PressKey(KeyCode),
    /// Simulate a key release.
    ReleaseKey(KeyCode),
    /// Hold a key for N frames, then release.
    HoldKey { key: KeyCode, frames: u32 },
    /// Simulate mouse motion delta (pixels).
    MouseMotion { delta: Vec2 },
    /// Simulate mouse scroll delta (x, y). Positive y = scroll up.
    MouseScroll { delta: Vec2 },
    /// Simulate a mouse button press.
    PressMouseButton(MouseButton),
    /// Simulate a mouse button release.
    ReleaseMouseButton(MouseButton),

    // --- Capture ---
    /// Save a named screenshot (downscaled to ~720p).
    Screenshot(String),
    /// Begin recording frames for video capture.
    StartRecording,
    /// Stop recording and stitch frames into video via ffmpeg.
    StopRecording,
    /// Write a message to the scenario log file.
    Log(String),

    // --- Conditional ---
    /// Poll a condition each frame until it returns `true`, then advance.
    ///
    /// The label is used for logging. Aborts after `max_frames` with a warning
    /// to avoid infinite hangs.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// Action::WaitUntil {
    ///     label: "assets loaded".into(),
    ///     condition: Box::new(|world| {
    ///         let server = world.resource::<AssetServer>();
    ///         server.load_state(&handle) == LoadState::Loaded
    ///     }),
    ///     max_frames: 600,
    /// }
    /// ```
    WaitUntil {
        label: String,
        condition: Box<dyn Fn(&World) -> bool + Send + Sync>,
        max_frames: u32,
    },

    // --- Lifecycle ---
    /// Stop the scenario but keep the game running instead of exiting.
    ///
    /// Use this as the final action to hand off control to an external tool
    /// (e.g. BRP for interactive debugging). The scenario runner restores
    /// real-time progression and removes itself, leaving the game in the
    /// state the scenario set up.
    Handoff,

    // --- Game-specific (dispatched via closure) ---
    /// Arbitrary world mutation. Use for project-specific actions
    /// like teleporting the player, setting look angles, etc.
    Custom(Box<dyn FnOnce(&mut World) + Send + Sync>),
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::WaitFrames(n) => write!(f, "WaitFrames({n})"),
            Action::PressKey(k) => write!(f, "PressKey({k:?})"),
            Action::ReleaseKey(k) => write!(f, "ReleaseKey({k:?})"),
            Action::HoldKey { key, frames } => write!(f, "HoldKey({key:?}, {frames})"),
            Action::MouseMotion { delta } => write!(f, "MouseMotion({delta})"),
            Action::MouseScroll { delta } => write!(f, "MouseScroll({delta})"),
            Action::PressMouseButton(b) => write!(f, "PressMouseButton({b:?})"),
            Action::ReleaseMouseButton(b) => write!(f, "ReleaseMouseButton({b:?})"),
            Action::Screenshot(name) => write!(f, "Screenshot({name:?})"),
            Action::StartRecording => write!(f, "StartRecording"),
            Action::StopRecording => write!(f, "StopRecording"),
            Action::Handoff => write!(f, "Handoff"),
            Action::Log(msg) => write!(f, "Log({msg:?})"),
            Action::WaitUntil {
                label, max_frames, ..
            } => {
                write!(f, "WaitUntil({label:?}, max={max_frames})")
            }
            Action::Custom(_) => write!(f, "Custom(...)"),
        }
    }
}
