//! Pixel body splitting via connected components.
//!
//! When pixels are destroyed, this system detects if the body has fragmented
//! into disconnected components and spawns separate entities for each fragment.

use std::collections::HashMap;

use bevy::prelude::*;

use super::{
  LastBlitTransform, NeedsColliderRegen, Persistable, PixelBody, PixelBodyId, PixelBodyIdGenerator,
  ShapeMaskModified,
};
#[cfg(physics)]
use crate::collision::CollisionQueryPoint;
use crate::collision::Stabilizing;
use crate::debug_shim::GizmosParam;
use crate::material::Materials;
use crate::persistence::PersistenceTasks;
use crate::world::PixelWorld;
#[cfg(physics)]
use crate::world::streaming::culling::StreamCulled;

/// Type alias for velocity query - varies by physics backend.
#[cfg(feature = "avian2d")]
type VelocityQuery<'w, 's> = Query<
  'w,
  's,
  (
    Option<&'static avian2d::prelude::LinearVelocity>,
    Option<&'static avian2d::prelude::AngularVelocity>,
  ),
>;

#[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
type VelocityQuery<'w, 's> = Query<'w, 's, Option<&'static bevy_rapier2d::prelude::Velocity>>;

/// Extracts linear and angular velocity for fragment spawning.
#[cfg(physics)]
fn extract_parent_velocity(entity: Entity, velocities: &VelocityQuery) -> (Vec2, f32) {
  #[cfg(feature = "avian2d")]
  {
    velocities
      .get(entity)
      .map(|(lin, ang)| {
        (
          lin.map(|v| v.0).unwrap_or(Vec2::ZERO),
          ang.map(|v| v.0).unwrap_or(0.0),
        )
      })
      .unwrap_or((Vec2::ZERO, 0.0))
  }

  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
  {
    velocities
      .get(entity)
      .ok()
      .flatten()
      .map(|v| (v.linvel, v.angvel))
      .unwrap_or((Vec2::ZERO, 0.0))
  }
}

/// A connected region of pixels within a shape mask.
pub struct ConnectedComponent {
  /// Minimum X coordinate in local pixel space.
  pub min_x: u32,
  /// Minimum Y coordinate in local pixel space.
  pub min_y: u32,
  /// Width of the tight bounding box.
  pub width: u32,
  /// Height of the tight bounding box.
  pub height: u32,
  /// Local coordinates of all pixels in this component.
  pub pixels: Vec<(u32, u32)>,
}

/// Union-Find data structure for connected component detection.
struct UnionFind {
  parent: Vec<usize>,
  rank: Vec<u8>,
}

impl UnionFind {
  fn new(size: usize) -> Self {
    Self {
      parent: (0..size).collect(),
      rank: vec![0; size],
    }
  }

  fn find(&mut self, mut x: usize) -> usize {
    // Path compression
    let mut root = x;
    while self.parent[root] != root {
      root = self.parent[root];
    }
    while self.parent[x] != root {
      let next = self.parent[x];
      self.parent[x] = root;
      x = next;
    }
    root
  }

  fn union(&mut self, x: usize, y: usize) {
    let rx = self.find(x);
    let ry = self.find(y);
    if rx == ry {
      return;
    }
    // Union by rank
    match self.rank[rx].cmp(&self.rank[ry]) {
      std::cmp::Ordering::Less => self.parent[rx] = ry,
      std::cmp::Ordering::Greater => self.parent[ry] = rx,
      std::cmp::Ordering::Equal => {
        self.parent[ry] = rx;
        self.rank[rx] += 1;
      }
    }
  }
}

/// Unions adjacent solid pixels using 4-connectivity.
fn union_adjacent_pixels(uf: &mut UnionFind, shape_mask: &[bool], w: usize, h: usize) {
  for y in 0..h {
    for x in 0..w {
      let idx = y * w + x;
      if !shape_mask[idx] {
        continue;
      }

      // Check right neighbor
      if x + 1 < w && shape_mask[idx + 1] {
        uf.union(idx, idx + 1);
      }
      // Check bottom neighbor
      if y + 1 < h && shape_mask[idx + w] {
        uf.union(idx, idx + w);
      }
    }
  }
}

/// Computes the bounding box for a list of pixel coordinates.
fn compute_bounding_box(pixels: &[(u32, u32)]) -> (u32, u32, u32, u32) {
  let (mut min_x, mut min_y) = (u32::MAX, u32::MAX);
  let (mut max_x, mut max_y) = (0u32, 0u32);
  for &(x, y) in pixels {
    min_x = min_x.min(x);
    min_y = min_y.min(y);
    max_x = max_x.max(x);
    max_y = max_y.max(y);
  }
  (min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
}

/// Finds connected components in a shape mask using 4-connectivity.
///
/// Returns components sorted by pixel count (largest first).
pub fn find_connected_components(
  shape_mask: &[bool],
  width: u32,
  height: u32,
) -> Vec<ConnectedComponent> {
  let w = width as usize;
  let h = height as usize;
  let size = w * h;

  if size == 0 {
    return Vec::new();
  }

  let mut uf = UnionFind::new(size);
  union_adjacent_pixels(&mut uf, shape_mask, w, h);

  // Group pixels by their root
  let mut groups: HashMap<usize, Vec<(u32, u32)>> = HashMap::new();
  for y in 0..h {
    for x in 0..w {
      let idx = y * w + x;
      if shape_mask[idx] {
        let root = uf.find(idx);
        groups.entry(root).or_default().push((x as u32, y as u32));
      }
    }
  }

  // Convert groups to components with bounding boxes
  let mut components: Vec<ConnectedComponent> = groups
    .into_values()
    .map(|pixels| {
      let (min_x, min_y, width, height) = compute_bounding_box(&pixels);
      ConnectedComponent {
        min_x,
        min_y,
        width,
        height,
        pixels,
      }
    })
    .collect();

  // Sort by pixel count descending
  components.sort_by(|a, b| b.pixels.len().cmp(&a.pixels.len()));

  components
}

/// Handles the case where a pixel body has no remaining pixels.
///
/// Clears blitted pixels, queues removal from persistence, and despawns the
/// entity.
fn handle_empty_body(
  commands: &mut Commands,
  persistence_tasks: &mut Option<ResMut<PersistenceTasks>>,
  world: &mut Option<Mut<PixelWorld>>,
  entity: Entity,
  body_id: &PixelBodyId,
  blitted: &LastBlitTransform,
  gizmos: crate::debug_shim::DebugGizmos<'_>,
) {
  if let Some(tasks) = persistence_tasks {
    tasks.queue_body_remove(body_id.value());
  }
  if let Some(w) = world {
    // Clear using written_positions, NOT for_each_body_pixel.
    // The shape_mask has already been set to all-false by apply_readback_changes,
    // so for_each_body_pixel would skip all pixels. But update_pixel_bodies may
    // have re-blitted pixels before shape_mask was updated, leaving ghost pixels.
    super::blit::clear_body_pixels(w, &blitted.written_positions, None, gizmos);
  }
  commands.entity(entity).despawn();
}

/// Handles the case where a pixel body has a single connected component.
///
/// Removes modification markers and triggers collider regeneration.
fn handle_single_component(commands: &mut Commands, entity: Entity) {
  commands
    .entity(entity)
    .remove::<ShapeMaskModified>()
    .remove::<NeedsColliderRegen>()
    .insert(NeedsColliderRegen);
}

/// Context for spawning fragment entities from a split pixel body.
struct FragmentSpawnContext<'a, 'w, 's> {
  commands: &'a mut Commands<'w, 's>,
  id_generator: &'a mut PixelBodyIdGenerator,
  world: &'a mut PixelWorld,
  materials: &'a Materials,
  parent_body: &'a PixelBody,
  blit_transform: &'a GlobalTransform,
  parent_rotation: Quat,
  #[cfg(physics)]
  parent_linear: Vec2,
  #[cfg(physics)]
  parent_angular: f32,
  gizmos: crate::debug_shim::DebugGizmos<'a>,
}

/// Spawns fragment entities for each connected component.
fn spawn_fragment_entities(
  ctx: FragmentSpawnContext<'_, '_, '_>,
  components: Vec<ConnectedComponent>,
) {
  for component in components {
    let Some(fragment) = create_fragment(
      ctx.parent_body,
      &component,
      ctx.blit_transform,
      ctx.id_generator,
    ) else {
      continue;
    };

    #[cfg(physics)]
    let Some(collider) = super::generate_collider(&fragment.body) else {
      continue;
    };

    let frag_transform = Transform::from_translation(fragment.world_pos.extend(0.0))
      .with_rotation(ctx.parent_rotation);
    let frag_global = GlobalTransform::from(frag_transform);

    // Blit fragment and track written positions for erasure detection
    let written_positions = super::blit::blit_single_body(
      ctx.world,
      &fragment.body,
      &frag_global,
      None, // No displacement for fragments
      ctx.materials,
      ctx.gizmos,
    );

    #[allow(unused_mut, unused_variables)]
    let mut entity_commands = ctx.commands.spawn((
      fragment.body,
      LastBlitTransform {
        transform: Some(frag_global),
        written_positions,
      },
      frag_transform,
      // Explicit GlobalTransform ensures correct position on first frame.
      // Without this, GlobalTransform defaults to identity until PostUpdate.
      frag_global,
      fragment.id,
      Persistable,
      Stabilizing::default(),
    ));

    #[cfg(feature = "avian2d")]
    entity_commands.insert((
      collider,
      avian2d::prelude::RigidBody::Dynamic,
      avian2d::prelude::LinearVelocity(ctx.parent_linear),
      avian2d::prelude::AngularVelocity(ctx.parent_angular),
      CollisionQueryPoint,
      StreamCulled,
    ));

    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
    entity_commands.insert((
      collider,
      bevy_rapier2d::prelude::RigidBody::Dynamic,
      bevy_rapier2d::prelude::Velocity {
        linvel: ctx.parent_linear,
        angvel: ctx.parent_angular,
      },
      CollisionQueryPoint,
      StreamCulled,
    ));
  }
}

/// Handles entity splitting when pixel bodies fragment.
///
/// For bodies marked with `ShapeMaskModified`:
/// - 0 components: despawn entity (fully destroyed)
/// - 1 component: remove marker (collider regen handles update)
/// - N > 1 components: despawn original, spawn N fragment entities
#[allow(clippy::type_complexity, clippy::too_many_arguments, unused_variables)]
pub fn split_pixel_bodies(
  mut commands: Commands,
  mut id_generator: ResMut<PixelBodyIdGenerator>,
  mut persistence_tasks: Option<ResMut<PersistenceTasks>>,
  mut worlds: Query<&mut PixelWorld>,
  bodies: Query<
    (
      Entity,
      &PixelBody,
      &PixelBodyId,
      &LastBlitTransform,
      &GlobalTransform,
    ),
    With<ShapeMaskModified>,
  >,
  #[cfg(physics)] velocities: VelocityQuery,
  materials: Res<Materials>,
  gizmos: GizmosParam,
) {
  let mut world = worlds.single_mut().ok();

  for (entity, body, body_id, blitted, global_transform) in bodies.iter() {
    let components = find_connected_components(&body.shape_mask, body.width(), body.height());

    match components.len() {
      0 => {
        handle_empty_body(
          &mut commands,
          &mut persistence_tasks,
          &mut world,
          entity,
          body_id,
          blitted,
          gizmos.get(),
        );
      }
      1 => {
        handle_single_component(&mut commands, entity);
      }
      _ => {
        if let Some(ref mut tasks) = persistence_tasks {
          tasks.queue_body_remove(body_id.value());
        }

        let parent_rotation = global_transform.to_scale_rotation_translation().1;

        #[cfg(physics)]
        let (parent_linear, parent_angular) = extract_parent_velocity(entity, &velocities);

        let Some(blit_transform) = &blitted.transform else {
          commands.entity(entity).despawn();
          continue;
        };

        let Some(ref mut world) = world else {
          commands.entity(entity).despawn();
          continue;
        };

        super::blit::clear_single_body_no_tracking(world, body, blit_transform, gizmos.get());
        commands.entity(entity).despawn();

        spawn_fragment_entities(
          FragmentSpawnContext {
            commands: &mut commands,
            id_generator: &mut id_generator,
            world,
            materials: &materials,
            parent_body: body,
            blit_transform,
            parent_rotation,
            #[cfg(physics)]
            parent_linear,
            #[cfg(physics)]
            parent_angular,
            gizmos: gizmos.get(),
          },
          components,
        );
      }
    }
  }
}

/// Data for a fragment to be spawned.
struct Fragment {
  body: PixelBody,
  world_pos: Vec2,
  id: PixelBodyId,
}

/// Creates a fragment pixel body from a connected component.
fn create_fragment(
  parent: &PixelBody,
  component: &ConnectedComponent,
  blit_transform: &GlobalTransform,
  id_generator: &mut PixelBodyIdGenerator,
) -> Option<Fragment> {
  if component.pixels.is_empty() {
    return None;
  }

  let width = component.width;
  let height = component.height;

  // Create new pixel body with tight bounds
  let mut fragment_body = PixelBody::new(width, height);

  // Copy pixels from parent at component positions (adjusted to fragment-local
  // coords)
  for &(px, py) in &component.pixels {
    let local_x = px - component.min_x;
    let local_y = py - component.min_y;

    if let Some(pixel) = parent.get_pixel(px, py) {
      fragment_body.set_pixel(local_x, local_y, *pixel);
    }
  }

  // Compute centroid in parent-local coords
  let centroid_x = component.min_x as f32 + (width as f32 / 2.0) + parent.origin.x as f32;
  let centroid_y = component.min_y as f32 + (height as f32 / 2.0) + parent.origin.y as f32;

  // Transform to world position
  let world_pos = blit_transform.transform_point(Vec3::new(centroid_x, centroid_y, 0.0));

  Some(Fragment {
    body: fragment_body,
    world_pos: world_pos.truncate(),
    id: id_generator.generate(),
  })
}
