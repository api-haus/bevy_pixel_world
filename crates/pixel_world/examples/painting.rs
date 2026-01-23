//! Brush Painting Demo - PixelWorld modification example.
//!
//! Demonstrates using the PixelWorld API for pixel modification.
//! Paint across chunk boundaries seamlessly.
//!
//! Controls:
//! - LMB: Paint with stone material
//! - RMB: Erase (paint with air)
//! - Scroll wheel: Adjust brush radius
//! - WASD/Arrow keys: Move camera
//! - Shift: Speed boost (5x)
//!
//! Run with: `cargo run -p pixel_world --example painting`

use bevy::camera::ScalingMode;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use pixel_world::{
    create_chunk_quad, material_ids, ColorIndex, MaterialSeeder, Materials, Pixel,
    PixelWorld, PixelWorldBundle, PixelWorldPlugin, StreamingCamera, WorldRect,
    CHUNK_SIZE,
};

const CAMERA_SPEED: f32 = 500.0;
const SPEED_BOOST: f32 = 5.0;
const MIN_RADIUS: u32 = 5;
const MAX_RADIUS: u32 = 200;
const DEFAULT_RADIUS: u32 = 20;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Brush Painting Demo - PixelWorld".to_string(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PixelWorldPlugin)
        .insert_resource(BrushState::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (input_system, camera_input, paint_system).chain(),
        )
        .run();
}

#[derive(Resource)]
struct BrushState {
    radius: u32,
    painting: bool,
    erasing: bool,
    world_pos: Option<(i64, i64)>,
}

impl Default for BrushState {
    fn default() -> Self {
        Self {
            radius: DEFAULT_RADIUS,
            painting: false,
            erasing: false,
            world_pos: None,
        }
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    // Spawn camera with StreamingCamera marker
    commands.spawn((
        Camera2d,
        StreamingCamera,
        Projection::Orthographic(OrthographicProjection {
            near: -1000.0,
            far: 1000.0,
            scale: 1.0,
            viewport_origin: Vec2::new(0.5, 0.5),
            scaling_mode: ScalingMode::AutoMin {
                min_width: 640.0,
                min_height: 480.0,
            },
            area: Rect::default(),
        }),
    ));

    // Create materials registry
    let mat_registry = Materials::new();
    commands.insert_resource(mat_registry);

    // Create shared mesh for chunks
    let mesh = meshes.add(create_chunk_quad(CHUNK_SIZE as f32, CHUNK_SIZE as f32));

    // Create material seeder for terrain generation (using defaults)
    let seeder = MaterialSeeder::new(42);

    // Spawn the pixel world
    commands.spawn(PixelWorldBundle::new(seeder, mesh));
}

fn input_system(
    mut brush: ResMut<BrushState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut scroll_events: MessageReader<MouseWheel>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<StreamingCamera>>,
) {
    brush.painting = mouse_buttons.pressed(MouseButton::Left);
    brush.erasing = mouse_buttons.pressed(MouseButton::Right);

    // Handle scroll wheel for radius
    for event in scroll_events.read() {
        let delta = match event.unit {
            MouseScrollUnit::Line => event.y as i32 * 5,
            MouseScrollUnit::Pixel => (event.y / 10.0) as i32,
        };
        let new_radius = (brush.radius as i32 + delta).clamp(MIN_RADIUS as i32, MAX_RADIUS as i32);
        brush.radius = new_radius as u32;
    }

    // Convert mouse position to world coordinates
    let Ok(window) = window_query.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    if let Some(cursor_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            brush.world_pos = Some((world_pos.x as i64, world_pos.y as i64));
        }
    } else {
        brush.world_pos = None;
    }
}

fn camera_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut camera: Query<&mut Transform, With<StreamingCamera>>,
    time: Res<Time>,
) {
    let mut direction = Vec2::ZERO;

    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    if direction == Vec2::ZERO {
        return;
    }

    let direction = direction.normalize();
    let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        CAMERA_SPEED * SPEED_BOOST
    } else {
        CAMERA_SPEED
    };

    if let Ok(mut transform) = camera.single_mut() {
        transform.translation.x += direction.x * speed * time.delta_secs();
        transform.translation.y += direction.y * speed * time.delta_secs();
    }
}

#[cfg(feature = "visual-debug")]
fn paint_system(
    brush: Res<BrushState>,
    mut worlds: Query<&mut PixelWorld>,
    debug_gizmos: Res<pixel_world::visual_debug::PendingDebugGizmos>,
) {
    if !brush.painting && !brush.erasing {
        return;
    }

    let Some((center_x, center_y)) = brush.world_pos else {
        return;
    };

    let Ok(mut world) = worlds.single_mut() else {
        return;
    };

    // Use STONE material for painting, AIR for erasing
    let (material, color) = if brush.painting {
        (material_ids::STONE, ColorIndex(128))
    } else {
        (material_ids::AIR, ColorIndex(0))
    };
    let brush_pixel = Pixel::new(material, color);

    let radius = brush.radius;
    let radius_i64 = radius as i64;
    let radius_sq = (radius_i64 * radius_i64) as f32;

    // Use the blit API for parallel painting
    let rect = WorldRect::centered(center_x, center_y, radius);

    world.blit(
        rect,
        |frag| {
            let dx = frag.x - center_x;
            let dy = frag.y - center_y;
            let dist_sq = (dx * dx + dy * dy) as f32;

            if dist_sq <= radius_sq {
                Some(brush_pixel)
            } else {
                None
            }
        },
        Some(&debug_gizmos),
    );
}

#[cfg(not(feature = "visual-debug"))]
fn paint_system(brush: Res<BrushState>, mut worlds: Query<&mut PixelWorld>) {
    if !brush.painting && !brush.erasing {
        return;
    }

    let Some((center_x, center_y)) = brush.world_pos else {
        return;
    };

    let Ok(mut world) = worlds.single_mut() else {
        return;
    };

    // Use STONE material for painting, AIR for erasing
    let (material, color) = if brush.painting {
        (material_ids::STONE, ColorIndex(128))
    } else {
        (material_ids::AIR, ColorIndex(0))
    };
    let brush_pixel = Pixel::new(material, color);

    let radius = brush.radius;
    let radius_i64 = radius as i64;
    let radius_sq = (radius_i64 * radius_i64) as f32;

    // Use the blit API for parallel painting
    let rect = WorldRect::centered(center_x, center_y, radius);

    world.blit(
        rect,
        |frag| {
            let dx = frag.x - center_x;
            let dy = frag.y - center_y;
            let dist_sq = (dx * dx + dy * dy) as f32;

            if dist_sq <= radius_sq {
                Some(brush_pixel)
            } else {
                None
            }
        },
        (),
    );
}
