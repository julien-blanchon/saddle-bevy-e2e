//! Input simulation helpers — inject keyboard and mouse events into Bevy.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

/// Simulate pressing a key by writing directly to `ButtonInput<KeyCode>`.
pub fn simulate_key_press(world: &mut World, key: KeyCode) {
    if let Some(mut input) = world.get_resource_mut::<ButtonInput<KeyCode>>() {
        input.press(key);
    }
}

/// Simulate releasing a key by writing directly to `ButtonInput<KeyCode>`.
pub fn simulate_key_release(world: &mut World, key: KeyCode) {
    if let Some(mut input) = world.get_resource_mut::<ButtonInput<KeyCode>>() {
        input.release(key);
    }
}

/// Simulate pressing a mouse button.
pub fn simulate_mouse_press(world: &mut World, button: MouseButton) {
    if let Some(mut input) = world.get_resource_mut::<ButtonInput<MouseButton>>() {
        input.press(button);
    }
}

/// Simulate releasing a mouse button.
pub fn simulate_mouse_release(world: &mut World, button: MouseButton) {
    if let Some(mut input) = world.get_resource_mut::<ButtonInput<MouseButton>>() {
        input.release(button);
    }
}

/// Simulate mouse motion by writing to the `AccumulatedMouseMotion` resource.
///
/// This is what Bevy's mouse_look system reads each frame.
pub fn simulate_mouse_motion(world: &mut World, delta: Vec2) {
    if let Some(mut accumulated) = world.get_resource_mut::<AccumulatedMouseMotion>() {
        accumulated.delta += delta;
    }
}

/// Simulate mouse scroll by writing to the `AccumulatedMouseScroll` resource.
///
/// This is what `bevy_enhanced_input` reads for `Binding::mouse_wheel()`.
pub fn simulate_mouse_scroll(world: &mut World, delta: Vec2) {
    use bevy::input::mouse::AccumulatedMouseScroll;
    if let Some(mut accumulated) = world.get_resource_mut::<AccumulatedMouseScroll>() {
        accumulated.delta += delta;
    }
}
