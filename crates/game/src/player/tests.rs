use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use super::components::*;
use super::{interpolation, movement};
use crate::core::GravityConfig;

#[test]
fn player_falls_with_gravity() {
  let mut app = App::new();

  app
    .add_plugins(MinimalPlugins)
    .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(1.0))
    .insert_resource(Time::<Fixed>::from_hz(60.0))
    .insert_resource(GravityConfig { value: 980.0 });

  app
    .add_systems(FixedFirst, interpolation::shift_positions)
    .add_systems(
      FixedUpdate,
      (
        movement::apply_locomotion_physics,
        movement::apply_velocity_to_controller,
      )
        .chain()
        .before(PhysicsSet::SyncBackend),
    )
    .add_systems(
      FixedUpdate,
      (
        movement::sync_ground_from_physics,
        interpolation::store_current_position,
      )
        .chain()
        .after(PhysicsSet::Writeback),
    );

  let spawn_pos = Vec3::new(0.0, 100.0, 0.0);
  let player = app
    .world_mut()
    .spawn((
      Player,
      Transform::from_translation(spawn_pos),
      RigidBody::KinematicPositionBased,
      Collider::capsule_y(10.0, 15.0),
      KinematicCharacterController::default(),
      CharacterVelocity::default(),
      LocomotionState::Airborne,
      PreviousPosition(spawn_pos),
      CurrentPosition(spawn_pos),
    ))
    .id();

  // First update to initialize Rapier
  app.update();

  let initial_pos = app.world().get::<Transform>(player).unwrap().translation;

  // Run many more updates to accumulate more simulated time
  // Need ~5000 updates to get ~0.5s of simulated time at 120Hz
  for _ in 0..5000 {
    app.update();
  }

  let final_pos = app.world().get::<Transform>(player).unwrap().translation;
  let final_vel = app.world().get::<CharacterVelocity>(player).unwrap().0;
  let delta = initial_pos.y - final_pos.y;

  println!("Initial Y: {}", initial_pos.y);
  println!("Final Y: {}", final_pos.y);
  println!("Delta Y: {}", delta);
  println!("Final velocity Y: {}", final_vel.y);

  // With enough time, player should fall at least 20 units
  assert!(
    delta > 20.0,
    "Player should fall at least 20 units. Only fell {} units. initial_y={}, final_y={}",
    delta,
    initial_pos.y,
    final_pos.y
  );

  // Velocity should reach a significant value (approaching terminal velocity of
  // 500)
  assert!(
    final_vel.y < -100.0,
    "Velocity should be at least -100. vel_y={}",
    final_vel.y
  );
}
