//! Simulation Demo - Falling sand and water physics.
//!
//! Demonstrates cellular automata simulation with sand and water.
//!
//! Controls:
//! - LMB: Paint with sand
//! - RMB: Paint with water
//! - MMB: Erase (paint with air)
//! - 1: Select sand brush
//! - 2: Select water brush
//! - 3: Select stone brush (creates static walls)
//! - Scroll wheel: Adjust brush radius
//! - WASD/Arrow keys: Move camera
//! - Shift: Speed boost (5x)
//!
//! Run with: `cargo run -p pixel_world --example simulation`

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
const MIN_RADIUS: u32 = 2;
const MAX_RADIUS: u32 = 100;
const DEFAULT_RADIUS: u32 = 15;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum BrushMaterial {
    #[default]
    Sand,
    Water,
    Stone,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Simulation Demo - Falling Sand & Water".to_string(),
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
    material: BrushMaterial,
}

impl Default for BrushState {
    fn default() -> Self {
        Self {
            radius: DEFAULT_RADIUS,
            painting: false,
            erasing: false,
            world_pos: None,
            material: BrushMaterial::Sand,
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

    // Create material seeder for terrain generation
    let seeder = MaterialSeeder::new(42);

    // Spawn the pixel world
    commands.spawn(PixelWorldBundle::new(seeder, mesh));
}

fn input_system(
    mut brush: ResMut<BrushState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut scroll_events: MessageReader<MouseWheel>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<StreamingCamera>>,
) {
    brush.painting = mouse_buttons.pressed(MouseButton::Left);
    brush.erasing = mouse_buttons.pressed(MouseButton::Middle) || mouse_buttons.pressed(MouseButton::Right) && keys.pressed(KeyCode::ShiftLeft);

    // Material selection
    if keys.just_pressed(KeyCode::Digit1) {
        brush.material = BrushMaterial::Sand;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        brush.material = BrushMaterial::Water;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        brush.material = BrushMaterial::Stone;
    }

    // RMB without shift paints with alternate material
    if mouse_buttons.pressed(MouseButton::Right) && !keys.pressed(KeyCode::ShiftLeft) {
        brush.painting = true;
        // RMB paints water when sand is selected, sand when water is selected
    }

    // Handle scroll wheel for radius
    for event in scroll_events.read() {
        let delta = match event.unit {
            MouseScrollUnit::Line => event.y as i32 * 3,
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
    mouse_buttons: Res<ButtonInput<MouseButton>>,
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

    // Determine material to paint
    let (material, color) = if brush.erasing {
        (material_ids::AIR, ColorIndex(0))
    } else {
        // LMB uses selected material, RMB uses alternate
        let use_alt = mouse_buttons.pressed(MouseButton::Right);
        match brush.material {
            BrushMaterial::Sand => {
                if use_alt {
                    (material_ids::WATER, ColorIndex(128))
                } else {
                    (material_ids::SAND, ColorIndex(128))
                }
            }
            BrushMaterial::Water => {
                if use_alt {
                    (material_ids::SAND, ColorIndex(128))
                } else {
                    (material_ids::WATER, ColorIndex(128))
                }
            }
            BrushMaterial::Stone => (material_ids::STONE, ColorIndex(128)),
        }
    };
    let brush_pixel = Pixel::new(material, color);

    let radius = brush.radius;
    let radius_i64 = radius as i64;
    let radius_sq = (radius_i64 * radius_i64) as f32;

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
fn paint_system(
    brush: Res<BrushState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut worlds: Query<&mut PixelWorld>,
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

    // Determine material to paint
    let (material, color) = if brush.erasing {
        (material_ids::AIR, ColorIndex(0))
    } else {
        // LMB uses selected material, RMB uses alternate
        let use_alt = mouse_buttons.pressed(MouseButton::Right);
        match brush.material {
            BrushMaterial::Sand => {
                if use_alt {
                    (material_ids::WATER, ColorIndex(128))
                } else {
                    (material_ids::SAND, ColorIndex(128))
                }
            }
            BrushMaterial::Water => {
                if use_alt {
                    (material_ids::SAND, ColorIndex(128))
                } else {
                    (material_ids::WATER, ColorIndex(128))
                }
            }
            BrushMaterial::Stone => (material_ids::STONE, ColorIndex(128)),
        }
    };
    let brush_pixel = Pixel::new(material, color);

    let radius = brush.radius;
    let radius_i64 = radius as i64;
    let radius_sq = (radius_i64 * radius_i64) as f32;

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
