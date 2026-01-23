//! ECS plugin and systems for PixelWorld.
//!
//! Provides automatic chunk streaming, seeding, and GPU upload.

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};

use super::{PixelWorld, SlotIndex};
use crate::coords::{ChunkPos, WorldPos, CHUNK_SIZE};
use crate::material::Materials;
use crate::primitives::Chunk;
use crate::render::{create_chunk_quad, create_texture, upload_surface, ChunkMaterial};

/// Marker component for the main camera that controls streaming.
#[derive(Component)]
pub struct StreamingCamera;

/// Resource holding async seeding tasks.
#[derive(Resource, Default)]
pub struct SeedingTasks {
    tasks: Vec<SeedingTask>,
}

/// An in-flight seeding task.
struct SeedingTask {
    /// Which PixelWorld entity.
    world_entity: Entity,
    /// Which slot is being seeded.
    slot_index: SlotIndex,
    /// The chunk position being seeded.
    pos: ChunkPos,
    /// The async task returning a seeded chunk.
    task: Task<Chunk>,
}

/// Maximum number of concurrent seeding tasks.
const MAX_SEEDING_TASKS: usize = 2;

/// Internal plugin for PixelWorld streaming systems.
///
/// This is automatically added by the main `PixelWorldPlugin`.
/// Do not add this plugin directly.
pub(crate) struct PixelWorldStreamingPlugin;

impl Plugin for PixelWorldStreamingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SeedingTasks>()
            .add_systems(Startup, setup_shared_resources)
            .add_systems(
                Update,
                (
                    tick_pixel_worlds,
                    dispatch_seeding,
                    poll_seeding_tasks,
                    upload_dirty_chunks,
                )
                    .chain(),
            );
    }
}

/// Shared mesh resource for chunk quads.
#[derive(Resource)]
pub struct SharedChunkMesh(pub Handle<Mesh>);

/// Sets up shared resources used by all PixelWorlds.
fn setup_shared_resources(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let mesh = meshes.add(create_chunk_quad(CHUNK_SIZE as f32, CHUNK_SIZE as f32));
    commands.insert_resource(SharedChunkMesh(mesh));
}

/// System: Updates streaming windows based on camera position.
///
/// For each PixelWorld, checks if the camera has moved to a new chunk
/// and updates the streaming window accordingly.
fn tick_pixel_worlds(
    mut commands: Commands,
    camera_query: Query<&GlobalTransform, With<StreamingCamera>>,
    mut worlds: Query<(Entity, &mut PixelWorld)>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ChunkMaterial>>,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };

    // Convert camera position to chunk position
    // Offset by half chunk so transitions occur at chunk centers
    let half_chunk = (CHUNK_SIZE / 2) as i64;
    let cam_pos = camera_transform.translation();
    let cam_x = cam_pos.x as i64 + half_chunk;
    let cam_y = cam_pos.y as i64 + half_chunk;
    let (chunk_pos, _) = WorldPos(cam_x, cam_y).to_chunk_and_local();

    for (_world_entity, mut world) in worlds.iter_mut() {
        // Check if this is initial spawn (no active chunks yet)
        let needs_initial_spawn = world.active_count() == 0;

        let delta = if needs_initial_spawn {
            // Force initial spawn by setting center and getting all visible positions
            world.initialize_at(chunk_pos)
        } else {
            world.update_center(chunk_pos)
        };

        // Despawn entities for chunks leaving the window
        for (_, entity) in delta.to_despawn {
            commands.entity(entity).despawn();
        }

        // Spawn entities for chunks entering the window
        for (pos, slot_idx) in delta.to_spawn {
            let slot = world.slot_mut(slot_idx);

            // Create or reuse texture
            let texture = if let Some(tex) = slot.texture.take() {
                tex
            } else {
                create_texture(&mut images, CHUNK_SIZE, CHUNK_SIZE)
            };

            // Create or reuse material
            let material = if let Some(mat) = slot.material.take() {
                mat
            } else {
                materials.add(ChunkMaterial {
                    texture: Some(texture.clone()),
                })
            };

            // Update material texture if reusing
            if let Some(mat) = materials.get_mut(&material) {
                mat.texture = Some(texture.clone());
            }

            // Spawn entity at chunk world position
            let world_pos = pos.to_world();
            let transform = Transform::from_xyz(
                world_pos.0 as f32 + CHUNK_SIZE as f32 / 2.0,
                world_pos.1 as f32 + CHUNK_SIZE as f32 / 2.0,
                0.0,
            );

            let entity = commands
                .spawn((
                    Mesh2d(world.mesh().clone()),
                    transform,
                    Visibility::default(),
                    MeshMaterial2d(material.clone()),
                ))
                .id();

            // Register entity and render resources in slot
            world.register_slot_entity(slot_idx, entity, texture, material);
        }
    }
}

/// System: Dispatches async seeding tasks for unseeded chunks.
fn dispatch_seeding(
    mut seeding_tasks: ResMut<SeedingTasks>,
    mut worlds: Query<(Entity, &mut PixelWorld)>,
) {
    let task_pool = AsyncComputeTaskPool::get();

    for (world_entity, world) in worlds.iter_mut() {
        // Count existing tasks for this world
        let in_flight = seeding_tasks
            .tasks
            .iter()
            .filter(|t| t.world_entity == world_entity)
            .count();

        if in_flight >= MAX_SEEDING_TASKS {
            continue;
        }

        // Find unseeded slots without in-flight tasks
        let in_flight_slots: std::collections::HashSet<_> = seeding_tasks
            .tasks
            .iter()
            .filter(|t| t.world_entity == world_entity)
            .map(|t| t.slot_index)
            .collect();

        for (pos, slot_idx) in world.active_chunks() {
            if in_flight_slots.contains(&slot_idx) {
                continue;
            }

            let slot = world.slot(slot_idx);
            if slot.seeded {
                continue;
            }

            // Spawn async seeding task
            let seeder = world.seeder().clone();
            let task = task_pool.spawn(async move {
                let mut chunk = Chunk::new(CHUNK_SIZE, CHUNK_SIZE);
                chunk.set_pos(pos);
                seeder.seed(pos, &mut chunk);
                chunk
            });

            seeding_tasks.tasks.push(SeedingTask {
                world_entity,
                slot_index: slot_idx,
                pos,
                task,
            });

            // Respect concurrency limit
            if seeding_tasks.tasks.iter().filter(|t| t.world_entity == world_entity).count()
                >= MAX_SEEDING_TASKS
            {
                break;
            }
        }
    }
}

/// System: Polls completed seeding tasks and swaps in seeded chunks.
fn poll_seeding_tasks(
    mut seeding_tasks: ResMut<SeedingTasks>,
    mut worlds: Query<&mut PixelWorld>,
) {
    let mut completed_indices = Vec::new();

    for (i, task) in seeding_tasks.tasks.iter_mut().enumerate() {
        // Poll the task - if it's ready, we get the result
        if task.task.is_finished() {
            let seeded_chunk = bevy::tasks::block_on(&mut task.task);
            // Task completed
            if let Ok(mut world) = worlds.get_mut(task.world_entity) {
                // Verify slot is still active for this position
                if let Some(current_idx) = world.get_slot_index(task.pos) {
                    if current_idx == task.slot_index {
                        let slot = world.slot_mut(task.slot_index);
                        // Swap in the seeded chunk data
                        slot.chunk.pixels = seeded_chunk.pixels;
                        slot.seeded = true;
                        slot.dirty = true;
                    }
                }
            }
            completed_indices.push(i);
        }
    }

    // Remove completed tasks (in reverse order to preserve indices)
    for i in completed_indices.into_iter().rev() {
        seeding_tasks.tasks.swap_remove(i);
    }
}

/// System: Uploads dirty chunks to GPU.
fn upload_dirty_chunks(
    mut worlds: Query<&mut PixelWorld>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ChunkMaterial>>,
    mat_registry: Option<Res<Materials>>,
) {
    let Some(mat_registry) = mat_registry else {
        return;
    };

    for mut world in worlds.iter_mut() {
        // Collect dirty+seeded slots
        let dirty_slots: Vec<_> = world
            .active_chunks()
            .filter_map(|(pos, idx)| {
                let slot = world.slot(idx);
                if slot.dirty && slot.seeded {
                    Some((pos, idx, slot.texture.clone()?, slot.material.clone()?))
                } else {
                    None
                }
            })
            .collect();

        for (pos, idx, texture_handle, material_handle) in dirty_slots {
            let slot = world.slot_mut(idx);

            // Materialize pixels to render buffer
            slot.chunk.materialize(&mat_registry);

            // Upload to GPU
            if let Some(image) = images.get_mut(&texture_handle) {
                upload_surface(slot.chunk.render_surface(), image);
            }

            // Touch material to force bind group refresh (Bevy workaround)
            let _ = materials.get_mut(&material_handle);

            // Mark clean
            slot.dirty = false;

            // Re-fetch slot to update (avoid borrow issues)
            let _ = pos; // suppress unused warning
        }
    }
}
