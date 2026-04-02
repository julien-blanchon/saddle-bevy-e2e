//! Screenshot and video capture utilities.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, atomic::AtomicBool};

/// Target width for downscaled screenshots and video.
/// Height is computed to preserve aspect ratio.
const TARGET_WIDTH: u32 = 1280;

/// How many consecutive frames the file count must remain stable
/// before we consider all async writes flushed.
pub const STABLE_FRAMES_THRESHOLD: u32 = 60;

/// Resource tracking the output directory and recording state for the current scenario.
#[derive(Resource)]
pub struct CaptureState {
    /// Base output directory (e.g. `e2e_output/weapon_orientation/`).
    pub output_dir: PathBuf,
    /// Whether we're currently recording frames for video.
    pub recording: bool,
    /// Frame index for recorded frames (sequential numbering).
    pub record_frame_index: u32,
    /// Log lines accumulated during the scenario.
    pub log_lines: Vec<String>,
    /// When true, we're waiting for async frame writes to complete before stitching.
    pub pending_stitch: bool,
    /// Number of frames we expect to find on disk before stitching.
    pub expected_frame_count: u32,
    /// Whether stitching has completed (safe to exit).
    pub stitch_complete: bool,
    /// Last observed file count in frames dir (for stabilization detection).
    pub last_file_count: usize,
    /// How many consecutive polls the file count has been stable.
    pub stable_count: u32,
    /// Shared flag set by the background stitch thread when done.
    pub stitch_done_flag: Option<Arc<AtomicBool>>,
}

impl CaptureState {
    pub fn new(scenario_name: &str) -> Self {
        let output_dir = PathBuf::from("e2e_output").join(scenario_name);
        // Clean previous output so stale screenshots, logs, and recordings
        // from prior runs don't confuse the agent or mix with new results.
        if output_dir.exists()
            && let Err(e) = fs::remove_dir_all(&output_dir)
        {
            warn!("[e2e] Failed to clean previous output dir: {e}");
        }
        Self {
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
        }
    }

    /// Path to the frames subdirectory (used during recording).
    pub fn frames_dir(&self) -> PathBuf {
        self.output_dir.join("frames")
    }

    /// Ensure the output and frames directories exist.
    pub fn ensure_dirs(&self) {
        fs::create_dir_all(&self.output_dir).ok();
        if self.recording {
            fs::create_dir_all(self.frames_dir()).ok();
        }
    }

    /// Append a line to the log.
    pub fn log(&mut self, msg: impl Into<String>) {
        let line = msg.into();
        info!("[e2e] {}", line);
        self.log_lines.push(line);
    }

    /// Write accumulated log to disk.
    pub fn flush_log(&self) {
        let path = self.output_dir.join("log.txt");
        let content = self.log_lines.join("\n");
        if let Err(e) = fs::write(&path, content) {
            error!("Failed to write e2e log: {e}");
        }
    }
}

/// Take a named screenshot, saving it to the scenario output directory.
/// The screenshot is saved at full resolution first, then downscaled via ffmpeg.
pub fn take_screenshot(commands: &mut Commands, capture: &CaptureState, name: &str) {
    capture.ensure_dirs();
    let raw_path = capture.output_dir.join(format!("{name}_raw.png"));
    let final_path = capture.output_dir.join(format!("{name}.png"));

    let raw_clone = raw_path.clone();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(raw_clone));

    // Schedule post-processing after the screenshot is saved.
    // We spawn a thread to avoid blocking the main loop.
    let raw_for_thread = raw_path;
    let final_for_thread = final_path.clone();
    std::thread::spawn(move || {
        // Wait briefly for the screenshot to be written
        std::thread::sleep(std::time::Duration::from_millis(500));
        downscale_image(&raw_for_thread, &final_for_thread);
    });

    info!("[e2e] Screenshot: {name} -> {}", final_path.display());
}

/// Take a recording frame (sequential numbered PNG).
pub fn take_recording_frame(commands: &mut Commands, capture: &mut CaptureState) {
    capture.ensure_dirs();
    let path = capture
        .frames_dir()
        .join(format!("frame_{:05}.png", capture.record_frame_index));
    capture.record_frame_index += 1;

    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

/// Downscale an image using ffmpeg to ~720p width while preserving aspect ratio.
pub fn downscale_image(input: &Path, output: &Path) {
    let scale_filter = format!("scale='min({TARGET_WIDTH},iw)':-2");
    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-vf")
        .arg(&scale_filter)
        .arg(output)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => {
            // Remove raw file after successful downscale
            fs::remove_file(input).ok();
            info!("[e2e] Downscaled screenshot: {}", output.display());
        }
        Ok(s) => {
            warn!(
                "[e2e] ffmpeg downscale exited with {s}, keeping raw file at {}",
                input.display()
            );
        }
        Err(e) => {
            warn!(
                "[e2e] ffmpeg not found or failed ({e}), keeping raw screenshot at {}",
                input.display()
            );
        }
    }
}

/// Stitch recorded frames into a video and post-process to 20fps ~720p no-audio.
///
/// Uses `-pattern_type glob` to handle gaps in the frame sequence (the render
/// pipeline may drop occasional screenshots under load).
pub fn stitch_video(capture: &CaptureState) {
    let frames_dir = capture.frames_dir();
    let raw_video = capture.output_dir.join("recording_raw.mp4");
    let final_video = capture.output_dir.join("recording.mp4");
    let glob_pattern = frames_dir.join("frame_*.png");

    // Step 1: Stitch frames at 30fps using glob (handles gaps + missing frame 0)
    let stitch_status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-framerate")
        .arg("30")
        .arg("-pattern_type")
        .arg("glob")
        .arg("-i")
        .arg(&glob_pattern)
        .arg("-c:v")
        .arg("libx264")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg(&raw_video)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match stitch_status {
        Ok(s) if s.success() => {
            info!("[e2e] Raw video stitched: {}", raw_video.display());
        }
        Ok(s) => {
            error!("[e2e] ffmpeg stitch failed with exit code {s}");
            return;
        }
        Err(e) => {
            error!("[e2e] ffmpeg not found: {e}");
            return;
        }
    }

    // Step 2: Post-process — 20fps, ~720p, no audio
    let vf_filter = format!("fps=20,scale='min({TARGET_WIDTH},iw)':-2");
    let pp_status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(&raw_video)
        .arg("-vf")
        .arg(&vf_filter)
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-crf")
        .arg("28")
        .arg("-an")
        .arg(&final_video)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match pp_status {
        Ok(s) if s.success() => {
            info!(
                "[e2e] Post-processed video (20fps, ~720p, no audio): {}",
                final_video.display()
            );
            // Clean up raw video
            fs::remove_file(&raw_video).ok();
        }
        Ok(s) => {
            warn!(
                "[e2e] ffmpeg post-process exited with {s}, keeping raw video at {}",
                raw_video.display()
            );
        }
        Err(e) => {
            warn!(
                "[e2e] ffmpeg post-process failed ({e}), keeping raw video at {}",
                raw_video.display()
            );
        }
    }
}

/// Count PNG files in the frames directory. Returns 0 if dir doesn't exist.
pub fn count_frame_files(capture: &CaptureState) -> usize {
    let frames_dir = capture.frames_dir();
    if !frames_dir.exists() {
        return 0;
    }
    fs::read_dir(&frames_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "png"))
                .count()
        })
        .unwrap_or(0)
}

/// Remove the frames directory. Called after the exit delay so all async
/// screenshot writes have flushed.
pub fn cleanup_frames(capture: &CaptureState) {
    let frames_dir = capture.frames_dir();
    if frames_dir.exists()
        && let Err(e) = fs::remove_dir_all(&frames_dir)
    {
        warn!("[e2e] Failed to clean up frames directory: {e}");
    }
}
