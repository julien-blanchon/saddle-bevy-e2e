//! World inspection helpers — log entity counts, component lists,
//! resource values, and dump world state to JSON.
//!
//! Inspired by BRP's introspection capabilities (`world.query`,
//! `world.list_components`, `world.get_resources`) but using direct
//! `&mut World` access for zero-overhead in-process inspection.
//!
//! # Examples
//!
//! ```rust,ignore
//! use bevy_e2e::actions::inspect;
//!
//! Scenario::builder("debug_combat")
//!     .then(inspect::log_entity_count::<Player>("players"))
//!     .then(inspect::log_entity_count::<Enemy>("enemies"))
//!     .then(inspect::log_resource::<Score>("score"))
//!     .then(inspect::log_entity_components::<Player>("player components"))
//!     .then(inspect::log_world_summary("world state"))
//!     .build()
//! ```

use bevy::prelude::*;

use crate::action::Action;
use crate::capture::CaptureState;
use crate::runner::ScenarioRunner;

/// Maximum entities to log per call (prevents log spam in large worlds).
const MAX_ENTITIES_LOG: usize = 10;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Shorten a fully-qualified type name to just the final segment.
/// `"bevy_transform::components::transform::Transform"` -> `"Transform"`
fn short_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

/// Format an entity as `"Entity(4v1) (Player)"` or `"Entity(4v1)"`.
fn entity_label(world: &World, entity: Entity) -> String {
    match world.get::<Name>(entity) {
        Some(name) => format!("{entity} ({name})"),
        None => format!("{entity}"),
    }
}

/// Get current frame from ScenarioRunner, or 0 if not present.
fn current_frame(world: &World) -> u32 {
    world
        .get_resource::<ScenarioRunner>()
        .map(|r| r.total_frames)
        .unwrap_or(0)
}

/// Log a message to CaptureState and bevy's info! logger.
fn log(world: &mut World, msg: String) {
    info!("[e2e] {msg}");
    if let Some(mut capture) = world.get_resource_mut::<CaptureState>() {
        capture.log(msg);
    }
}

/// Get all component type names for an entity.
fn component_names_for(world: &World, entity: Entity) -> Vec<String> {
    let entity_ref = world.entity(entity);
    let archetype = entity_ref.archetype();
    archetype
        .components()
        .iter()
        .filter_map(|id| world.components().get_info(*id))
        .map(|info| {
            let full_name = info.name().to_string();
            short_name(&full_name).to_string()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Entity counting
// ---------------------------------------------------------------------------

/// Log how many entities have component `C`.
///
/// Output: `[frame N] [inspect] label: X entities with TypeName`
pub fn log_entity_count<C: Component>(label: &str) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let count = world.query_filtered::<(), With<C>>().iter(world).count();
        let type_name = short_name(std::any::type_name::<C>());
        let frame = current_frame(world);
        log(
            world,
            format!("[frame {frame}] [inspect] {label}: {count} entities with {type_name}"),
        );
    }))
}

// ---------------------------------------------------------------------------
// Resource inspection
// ---------------------------------------------------------------------------

/// Log a resource's `Debug` representation.
///
/// Output: `[frame N] [inspect] label: TypeName { ... }`
pub fn log_resource<R: Resource + std::fmt::Debug>(label: &str) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let frame = current_frame(world);
        let type_name = short_name(std::any::type_name::<R>());
        match world.get_resource::<R>() {
            Some(res) => {
                log(
                    world,
                    format!("[frame {frame}] [inspect] {label}: {type_name} = {res:?}"),
                );
            }
            None => {
                log(
                    world,
                    format!("[frame {frame}] [inspect] {label}: {type_name} NOT FOUND"),
                );
            }
        }
    }))
}

// ---------------------------------------------------------------------------
// Component inspection
// ---------------------------------------------------------------------------

/// Log all component type names on entities with component `C`.
///
/// Shows up to 10 entities with their full component lists.
///
/// Output:
/// ```text
/// [frame N] [inspect] label (3 entities with Player):
///   Entity(4v1) (Player): [Transform, Player, Health, Velocity]
///   Entity(7v1) (Enemy): [Transform, Enemy, Health]
/// ```
pub fn log_entity_components<C: Component>(label: &str) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let frame = current_frame(world);
        let type_name = short_name(std::any::type_name::<C>());

        let entities: Vec<Entity> = world
            .query_filtered::<Entity, With<C>>()
            .iter(world)
            .collect();
        let total = entities.len();

        let mut lines = vec![format!(
            "[frame {frame}] [inspect] {label} ({total} entities with {type_name}):"
        )];

        for (i, entity) in entities.iter().enumerate() {
            if i >= MAX_ENTITIES_LOG {
                lines.push(format!("  ... and {} more", total - MAX_ENTITIES_LOG));
                break;
            }
            let elabel = entity_label(world, *entity);
            let components = component_names_for(world, *entity);
            lines.push(format!("  {elabel}: [{}]", components.join(", ")));
        }

        let msg = lines.join("\n");
        log(world, msg);
    }))
}

/// Log the `Debug` representation of component `C` on all entities that have it.
///
/// Shows up to 10 entries.
///
/// Output:
/// ```text
/// [frame N] [inspect] label (2 entities with Health):
///   Entity(4v1) (Player): Health { current: 80, max: 100 }
///   Entity(7v1) (Enemy): Health { current: 50, max: 50 }
/// ```
pub fn log_component<C: Component + std::fmt::Debug>(label: &str) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let frame = current_frame(world);
        let type_name = short_name(std::any::type_name::<C>());

        let data: Vec<(Entity, String)> = {
            let mut query = world.query::<(Entity, &C)>();
            query
                .iter(world)
                .map(|(e, c)| (e, format!("{c:?}")))
                .collect()
        };
        let total = data.len();

        let mut lines = vec![format!(
            "[frame {frame}] [inspect] {label} ({total} entities with {type_name}):"
        )];

        for (i, (entity, debug_val)) in data.iter().enumerate() {
            if i >= MAX_ENTITIES_LOG {
                lines.push(format!("  ... and {} more", total - MAX_ENTITIES_LOG));
                break;
            }
            let elabel = entity_label(world, *entity);
            lines.push(format!("  {elabel}: {debug_val}"));
        }

        let msg = lines.join("\n");
        log(world, msg);
    }))
}

/// Log the `Debug` representation of component `C` only on entities that also
/// have marker component `M`.
///
/// Useful for inspecting a common component (e.g. `Transform`) on a specific
/// entity type (e.g. `Player`).
///
/// Output:
/// ```text
/// [frame N] [inspect] label (1 entity with Transform + Player):
///   Entity(4v1) (Player): Transform { translation: Vec3(10, 0, 5), ... }
/// ```
pub fn log_component_where<C: Component + std::fmt::Debug, M: Component>(label: &str) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let frame = current_frame(world);
        let comp_name = short_name(std::any::type_name::<C>());
        let marker_name = short_name(std::any::type_name::<M>());

        let data: Vec<(Entity, String)> = {
            let mut query = world.query_filtered::<(Entity, &C), With<M>>();
            query
                .iter(world)
                .map(|(e, c)| (e, format!("{c:?}")))
                .collect()
        };
        let total = data.len();

        let mut lines = vec![format!(
            "[frame {frame}] [inspect] {label} ({total} entities with {comp_name} + {marker_name}):"
        )];

        for (i, (entity, debug_val)) in data.iter().enumerate() {
            if i >= MAX_ENTITIES_LOG {
                lines.push(format!("  ... and {} more", total - MAX_ENTITIES_LOG));
                break;
            }
            let elabel = entity_label(world, *entity);
            lines.push(format!("  {elabel}: {debug_val}"));
        }

        let msg = lines.join("\n");
        log(world, msg);
    }))
}

// ---------------------------------------------------------------------------
// World summary
// ---------------------------------------------------------------------------

/// Log a high-level summary of the world: total entity count and named entities.
///
/// Output:
/// ```text
/// [frame N] [inspect] label: 47 entities total, 12 named
///   Named: Player, MainCamera, DirectionalLight, Enemy, Enemy, ...
/// ```
pub fn log_world_summary(label: &str) -> Action {
    let label = label.to_string();
    Action::Custom(Box::new(move |world: &mut World| {
        let frame = current_frame(world);

        let total_entities = world.query::<Entity>().iter(world).count();

        let named: Vec<String> = {
            let mut query = world.query::<(Entity, &Name)>();
            query
                .iter(world)
                .map(|(_, name)| name.as_str().to_string())
                .collect()
        };
        let named_count = named.len();

        let mut msg = format!(
            "[frame {frame}] [inspect] {label}: {total_entities} entities total, {named_count} named"
        );

        if !named.is_empty() {
            let display_names: Vec<&str> = named
                .iter()
                .take(MAX_ENTITIES_LOG)
                .map(|s| s.as_str())
                .collect();
            msg.push_str(&format!("\n  Named: {}", display_names.join(", ")));
            if named_count > MAX_ENTITIES_LOG {
                msg.push_str(&format!(
                    ", ... and {} more",
                    named_count - MAX_ENTITIES_LOG
                ));
            }
        }

        log(world, msg);
    }))
}

// ---------------------------------------------------------------------------
// JSON dump (behind "json" feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "json")]
mod json_dump {
    use super::*;
    use std::fs;

    #[derive(serde::Serialize)]
    struct WorldDump {
        frame: u32,
        label: String,
        entity_count: usize,
        entities: Vec<EntityDump>,
    }

    #[derive(serde::Serialize)]
    struct EntityDump {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        components: Vec<String>,
    }

    #[derive(serde::Serialize)]
    struct ComponentValueDump {
        frame: u32,
        label: String,
        component_type: String,
        entity_count: usize,
        entities: Vec<EntityValueDump>,
    }

    #[derive(serde::Serialize)]
    struct EntityValueDump {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Debug representation of the component value.
        value: String,
    }

    /// Write a JSON string to a file in the scenario output directory.
    fn write_json_file(world: &mut World, filename: &str, json: &str) {
        let path = world.get_resource::<CaptureState>().map(|c| {
            fs::create_dir_all(&c.output_dir).ok();
            c.output_dir.join(filename)
        });

        if let Some(path) = path
            && let Err(e) = fs::write(&path, json)
        {
            warn!("[e2e] Failed to write JSON dump to {}: {e}", path.display());
        }
    }

    /// Dump **all** entities to a JSON file with their component type names.
    ///
    /// Output file: `e2e_output/{scenario}/{label}.json`
    ///
    /// ```json
    /// {
    ///   "frame": 120,
    ///   "label": "world_state",
    ///   "entity_count": 47,
    ///   "entities": [
    ///     { "id": "4v1", "name": "Player", "components": ["Transform", "Player", "Health"] },
    ///     { "id": "7v1", "components": ["Transform", "Mesh3d", "MeshMaterial3d"] }
    ///   ]
    /// }
    /// ```
    pub fn dump_world_json(label: &str) -> Action {
        let label = label.to_string();
        Action::Custom(Box::new(move |world: &mut World| {
            let frame = current_frame(world);

            // Collect entity IDs first to release the query borrow
            let entity_ids: Vec<Entity> = world.query::<Entity>().iter(world).collect();

            let entities: Vec<EntityDump> = entity_ids
                .iter()
                .map(|&entity| {
                    let id = format!("{entity}");
                    let name = world.get::<Name>(entity).map(|n| n.as_str().to_string());
                    let components = component_names_for(world, entity);
                    EntityDump {
                        id,
                        name,
                        components,
                    }
                })
                .collect();

            let count = entities.len();
            let dump = WorldDump {
                frame,
                label: label.clone(),
                entity_count: count,
                entities,
            };

            let json = serde_json::to_string_pretty(&dump)
                .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));

            let filename = format!("{label}.json");
            write_json_file(world, &filename, &json);
            log(
                world,
                format!("[frame {frame}] [inspect] {label}: dumped {count} entities to {filename}"),
            );
        }))
    }

    /// Dump entities with component `C` to a JSON file.
    ///
    /// Output file: `e2e_output/{scenario}/{label}.json`
    pub fn dump_entities_json<C: Component>(label: &str) -> Action {
        let label = label.to_string();
        Action::Custom(Box::new(move |world: &mut World| {
            let frame = current_frame(world);
            let type_name = short_name(std::any::type_name::<C>()).to_string();

            let entity_ids: Vec<Entity> = world
                .query_filtered::<Entity, With<C>>()
                .iter(world)
                .collect();

            let entities: Vec<EntityDump> = entity_ids
                .iter()
                .map(|&entity| {
                    let id = format!("{entity}");
                    let name = world.get::<Name>(entity).map(|n| n.as_str().to_string());
                    let components = component_names_for(world, entity);
                    EntityDump {
                        id,
                        name,
                        components,
                    }
                })
                .collect();

            let count = entities.len();
            let dump = WorldDump {
                frame,
                label: label.clone(),
                entity_count: count,
                entities,
            };

            let json = serde_json::to_string_pretty(&dump)
                .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));

            let filename = format!("{label}.json");
            write_json_file(world, &filename, &json);
            log(
                world,
                format!(
                    "[frame {frame}] [inspect] {label}: dumped {count} {type_name} entities to {filename}"
                ),
            );
        }))
    }

    /// Dump the `Debug` representation of component `C` for all entities that
    /// have it, as a JSON file.
    ///
    /// Output file: `e2e_output/{scenario}/{label}.json`
    ///
    /// ```json
    /// {
    ///   "frame": 120,
    ///   "label": "health_values",
    ///   "component_type": "Health",
    ///   "entity_count": 3,
    ///   "entities": [
    ///     { "id": "4v1", "name": "Player", "value": "Health { current: 80, max: 100 }" },
    ///     { "id": "7v1", "name": "Enemy", "value": "Health { current: 50, max: 50 }" }
    ///   ]
    /// }
    /// ```
    pub fn dump_component_json<C: Component + std::fmt::Debug>(label: &str) -> Action {
        let label = label.to_string();
        Action::Custom(Box::new(move |world: &mut World| {
            let frame = current_frame(world);
            let type_name = short_name(std::any::type_name::<C>()).to_string();

            let data: Vec<(Entity, String)> = {
                let mut query = world.query::<(Entity, &C)>();
                query
                    .iter(world)
                    .map(|(e, c)| (e, format!("{c:?}")))
                    .collect()
            };

            let entities: Vec<EntityValueDump> = data
                .iter()
                .map(|(entity, debug_val)| {
                    let id = format!("{entity}");
                    let name = world.get::<Name>(*entity).map(|n| n.as_str().to_string());
                    EntityValueDump {
                        id,
                        name,
                        value: debug_val.clone(),
                    }
                })
                .collect();

            let count = entities.len();
            let dump = ComponentValueDump {
                frame,
                label: label.clone(),
                component_type: type_name.clone(),
                entity_count: count,
                entities,
            };

            let json = serde_json::to_string_pretty(&dump)
                .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));

            let filename = format!("{label}.json");
            write_json_file(world, &filename, &json);
            log(
                world,
                format!(
                    "[frame {frame}] [inspect] {label}: dumped {count} {type_name} values to {filename}"
                ),
            );
        }))
    }
}

#[cfg(feature = "json")]
pub use json_dump::{dump_component_json, dump_entities_json, dump_world_json};
