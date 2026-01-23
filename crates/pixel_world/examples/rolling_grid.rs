//! Rolling Grid Demo - PixelWorld streaming example.
//!
//! Demonstrates the PixelWorld with automatic chunk streaming.
//! Chunks are seeded asynchronously as the camera moves through the infinite world.
//!
//! Features:
//! - Pixelated cutoff noise (hard solid/air boundary)
//! - Soil layer near surface (brown gradient)
//! - Stone layer below (gray gradient)
//! - Noise-feathered material boundaries for natural edges
//! - Async background seeding (chunks appear with brief delay)
//!
//! Controls:
//! - WASD/Arrow keys: Move camera
//! - Shift: Speed boost (5x)
//!
//! Run with: `cargo run -p pixel_world --example rolling_grid`

use bevy::{camera::ScalingMode, prelude::*};
use pixel_world::{
    create_chunk_quad, MaterialSeeder, Materials, PixelWorldBundle, PixelWorldPlugin,
    StreamingCamera, CHUNK_SIZE,
};

/// Base camera movement speed in pixels per second.
const CAMERA_SPEED: f32 = 500.0;

/// Speed multiplier when holding shift.
const SPEED_BOOST: f32 = 5.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Rolling Grid Demo - PixelWorld".to_string(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PixelWorldPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, camera_input)
        .run();
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    // Spawn camera at origin with StreamingCamera marker
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

    // Spawn the pixel world - streaming, seeding, and upload are all automatic!
    commands.spawn(PixelWorldBundle::new(seeder, mesh));
}

fn camera_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut camera: Query<&mut Transform, With<StreamingCamera>>,
    time: Res<Time>,
) {
    let mut direction = Vec2::ZERO;

    // WASD movement
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

    // Normalize and apply speed
    let direction = direction.normalize();
    let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        CAMERA_SPEED * SPEED_BOOST
    } else {
        CAMERA_SPEED
    };

    // Update camera position
    if let Ok(mut transform) = camera.single_mut() {
        transform.translation.x += direction.x * speed * time.delta_secs();
        transform.translation.y += direction.y * speed * time.delta_secs();
    }
}
