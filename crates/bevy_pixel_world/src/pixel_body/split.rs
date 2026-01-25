//! Pixel body splitting via connected components.
//!
//! When pixels are destroyed, this system detects if the body has fragmented
//! into disconnected components and spawns separate entities for each fragment.

use std::collections::HashMap;

use bevy::prelude::*;

use super::{
  BlittedTransform, NeedsColliderRegen, Persistable, PixelBody, PixelBodyId, PixelBodyIdGenerator,
  ShapeMaskModified,
};
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use crate::collision::CollisionQueryPoint;
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use crate::culling::StreamCulled;
use crate::debug_shim::GizmosParam;
use crate::world::PixelWorld;

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

  // Union adjacent solid pixels (4-connectivity)
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
      let (mut min_x, mut min_y) = (u32::MAX, u32::MAX);
      let (mut max_x, mut max_y) = (0u32, 0u32);
      for &(x, y) in &pixels {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
      }
      ConnectedComponent {
        min_x,
        min_y,
        width: max_x - min_x + 1,
        height: max_y - min_y + 1,
        pixels,
      }
    })
    .collect();

  // Sort by pixel count descending
  components.sort_by(|a, b| b.pixels.len().cmp(&a.pixels.len()));

  components
}

/// Handles entity splitting when pixel bodies fragment.
///
/// For bodies marked with `ShapeMaskModified`:
/// - 0 components: despawn entity (fully destroyed)
/// - 1 component: remove marker (collider regen handles update)
/// - N > 1 components: despawn original, spawn N fragment entities
#[cfg(feature = "avian2d")]
#[allow(clippy::type_complexity)]
pub fn split_pixel_bodies(
  mut commands: Commands,
  mut id_generator: ResMut<PixelBodyIdGenerator>,
  mut worlds: Query<&mut PixelWorld>,
  bodies: Query<
    (
      Entity,
      &PixelBody,
      &BlittedTransform,
      &GlobalTransform,
      Option<&avian2d::prelude::LinearVelocity>,
      Option<&avian2d::prelude::AngularVelocity>,
    ),
    With<ShapeMaskModified>,
  >,
  gizmos: GizmosParam,
) {
  for (entity, body, blitted, global_transform, lin_vel, ang_vel) in bodies.iter() {
    let components = find_connected_components(&body.shape_mask, body.width(), body.height());

    match components.len() {
      0 => {
        // Clear blitted pixels before despawning
        if let Some(transform) = &blitted.transform {
          if let Ok(mut world) = worlds.single_mut() {
            super::blit::clear_single_body(&mut world, body, transform, gizmos.get());
          }
        }
        // Fully destroyed - despawn
        commands.entity(entity).despawn();
      }
      1 => {
        // Single component - just remove the marker, collider regen handles the rest
        commands
          .entity(entity)
          .remove::<ShapeMaskModified>()
          .remove::<NeedsColliderRegen>()
          .insert(NeedsColliderRegen);
      }
      _ => {
        // Multiple components - split into fragments
        let parent_rotation = global_transform.to_scale_rotation_translation().1;
        let parent_linear = lin_vel.map(|v| v.0).unwrap_or(Vec2::ZERO);
        let parent_angular = ang_vel.map(|v| v.0).unwrap_or(0.0);

        // Get the transform used for blitting (where pixels actually are)
        let Some(blit_transform) = &blitted.transform else {
          commands.entity(entity).despawn();
          continue;
        };

        // Clear blitted pixels before despawning
        let Ok(mut world) = worlds.single_mut() else {
          commands.entity(entity).despawn();
          continue;
        };
        super::blit::clear_single_body(&mut world, body, blit_transform, gizmos.get());

        // Despawn original
        commands.entity(entity).despawn();

        // Spawn each fragment and blit immediately to avoid flicker
        for component in components {
          let Some(fragment) = create_fragment(body, &component, blit_transform, &mut id_generator)
          else {
            continue;
          };

          // Generate collider for this fragment
          let Some(collider) = super::generate_collider(&fragment.body) else {
            continue;
          };

          // Compute fragment transform and blit immediately
          let frag_transform = Transform::from_translation(fragment.world_pos.extend(0.0))
            .with_rotation(parent_rotation);
          let frag_global = GlobalTransform::from(frag_transform);
          super::blit::blit_single_body(&mut world, &fragment.body, &frag_global, gizmos.get());

          commands.spawn((
            fragment.body,
            collider,
            avian2d::prelude::RigidBody::Dynamic,
            avian2d::prelude::LinearVelocity(parent_linear),
            avian2d::prelude::AngularVelocity(parent_angular),
            CollisionQueryPoint,
            StreamCulled,
            BlittedTransform {
              transform: Some(frag_global),
            },
            frag_transform,
            fragment.id,
            Persistable,
          ));
        }
      }
    }
  }
}

/// Handles entity splitting when pixel bodies fragment (rapier2d variant).
#[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
#[allow(clippy::type_complexity)]
pub fn split_pixel_bodies(
  mut commands: Commands,
  mut id_generator: ResMut<PixelBodyIdGenerator>,
  mut worlds: Query<&mut PixelWorld>,
  bodies: Query<
    (
      Entity,
      &PixelBody,
      &BlittedTransform,
      &GlobalTransform,
      Option<&bevy_rapier2d::prelude::Velocity>,
    ),
    With<ShapeMaskModified>,
  >,
  gizmos: GizmosParam,
) {
  for (entity, body, blitted, global_transform, velocity) in bodies.iter() {
    let components = find_connected_components(&body.shape_mask, body.width(), body.height());

    match components.len() {
      0 => {
        // Clear blitted pixels before despawning
        if let Some(transform) = &blitted.transform {
          if let Ok(mut world) = worlds.single_mut() {
            super::blit::clear_single_body(&mut world, body, transform, gizmos.get());
          }
        }
        commands.entity(entity).despawn();
      }
      1 => {
        commands
          .entity(entity)
          .remove::<ShapeMaskModified>()
          .remove::<NeedsColliderRegen>()
          .insert(NeedsColliderRegen);
      }
      _ => {
        let parent_rotation = global_transform.to_scale_rotation_translation().1;
        let parent_velocity = velocity
          .map(|v| (v.linvel, v.angvel))
          .unwrap_or((Vec2::ZERO, 0.0));

        let Some(blit_transform) = &blitted.transform else {
          commands.entity(entity).despawn();
          continue;
        };

        // Clear blitted pixels before despawning
        let Ok(mut world) = worlds.single_mut() else {
          commands.entity(entity).despawn();
          continue;
        };
        super::blit::clear_single_body(&mut world, body, blit_transform, gizmos.get());

        commands.entity(entity).despawn();

        // Spawn each fragment and blit immediately to avoid flicker
        for component in components {
          let Some(fragment) = create_fragment(body, &component, blit_transform, &mut id_generator)
          else {
            continue;
          };

          let Some(collider) = super::generate_collider(&fragment.body) else {
            continue;
          };

          // Compute fragment transform and blit immediately
          let frag_transform = Transform::from_translation(fragment.world_pos.extend(0.0))
            .with_rotation(parent_rotation);
          let frag_global = GlobalTransform::from(frag_transform);
          super::blit::blit_single_body(&mut world, &fragment.body, &frag_global, gizmos.get());

          commands.spawn((
            fragment.body,
            collider,
            bevy_rapier2d::prelude::RigidBody::Dynamic,
            bevy_rapier2d::prelude::Velocity {
              linvel: parent_velocity.0,
              angvel: parent_velocity.1,
            },
            CollisionQueryPoint,
            StreamCulled,
            BlittedTransform {
              transform: Some(frag_global),
            },
            frag_transform,
            fragment.id,
            Persistable,
          ));
        }
      }
    }
  }
}

/// Handles entity splitting when pixel bodies fragment (no physics variant).
#[cfg(not(any(feature = "avian2d", feature = "rapier2d")))]
pub fn split_pixel_bodies(
  mut commands: Commands,
  mut id_generator: ResMut<PixelBodyIdGenerator>,
  mut worlds: Query<&mut PixelWorld>,
  bodies: Query<(Entity, &PixelBody, &BlittedTransform, &GlobalTransform), With<ShapeMaskModified>>,
  gizmos: GizmosParam,
) {
  for (entity, body, blitted, global_transform) in bodies.iter() {
    let components = find_connected_components(&body.shape_mask, body.width(), body.height());

    match components.len() {
      0 => {
        // Clear blitted pixels before despawning
        if let Some(transform) = &blitted.transform {
          if let Ok(mut world) = worlds.single_mut() {
            super::blit::clear_single_body(&mut world, body, transform, gizmos.get());
          }
        }
        commands.entity(entity).despawn();
      }
      1 => {
        commands
          .entity(entity)
          .remove::<ShapeMaskModified>()
          .remove::<NeedsColliderRegen>()
          .insert(NeedsColliderRegen);
      }
      _ => {
        let parent_rotation = global_transform.to_scale_rotation_translation().1;

        let Some(blit_transform) = &blitted.transform else {
          commands.entity(entity).despawn();
          continue;
        };

        // Clear blitted pixels before despawning
        let Ok(mut world) = worlds.single_mut() else {
          commands.entity(entity).despawn();
          continue;
        };
        super::blit::clear_single_body(&mut world, body, blit_transform, gizmos.get());

        commands.entity(entity).despawn();

        // Spawn each fragment and blit immediately to avoid flicker
        for component in components {
          let Some(fragment) = create_fragment(body, &component, blit_transform, &mut id_generator)
          else {
            continue;
          };

          // Compute fragment transform and blit immediately
          let frag_transform = Transform::from_translation(fragment.world_pos.extend(0.0))
            .with_rotation(parent_rotation);
          let frag_global = GlobalTransform::from(frag_transform);
          super::blit::blit_single_body(&mut world, &fragment.body, &frag_global, gizmos.get());

          commands.spawn((
            fragment.body,
            BlittedTransform {
              transform: Some(frag_global),
            },
            frag_transform,
            fragment.id,
            Persistable,
          ));
        }
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
