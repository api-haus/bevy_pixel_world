use bevy::prelude::*;
use bevy_egui::egui;
use bevy_rapier2d::prelude::*;
use bevy_yoleck::prelude::*;
use bevy_yoleck::vpeol::prelude::*;
use serde::{Deserialize, Serialize};

/// Yoleck component for platform data.
#[derive(Clone, PartialEq, Serialize, Deserialize, Component, YoleckComponent)]
pub struct PlatformData {
  pub width: f32,
  pub height: f32,
  pub color: [f32; 3],
}

impl Default for PlatformData {
  fn default() -> Self {
    Self {
      width: 100.0,
      height: 20.0,
      color: [0.4, 0.4, 0.6],
    }
  }
}

pub fn register(app: &mut App) {
  app.add_yoleck_entity_type(
    YoleckEntityType::new("Platform")
      .with::<Vpeol2dPosition>()
      .with::<PlatformData>(),
  );
  app.add_systems(YoleckSchedule::Populate, populate_platform);
}

#[cfg(feature = "editor")]
pub fn register_edit_systems(app: &mut App) {
  app.add_yoleck_edit_system(edit_platform);
}

fn populate_platform(mut populate: YoleckPopulate<(&Vpeol2dPosition, &PlatformData)>) {
  populate.populate(|_ctx, mut cmd, (position, data)| {
    cmd.insert((
      Transform::from_translation(position.0.extend(0.0)),
      Sprite {
        color: Color::srgb(data.color[0], data.color[1], data.color[2]),
        custom_size: Some(Vec2::new(data.width, data.height)),
        ..default()
      },
      RigidBody::Fixed,
      Collider::cuboid(data.width / 2.0, data.height / 2.0),
    ));
  });
}

#[cfg(feature = "editor")]
fn edit_platform(mut ui: ResMut<YoleckUi>, mut edit: YoleckEdit<&mut PlatformData>) {
  let Ok(mut data) = edit.single_mut() else {
    return;
  };

  ui.add(egui::Slider::new(&mut data.width, 20.0..=500.0).text("Width"));
  ui.add(egui::Slider::new(&mut data.height, 10.0..=200.0).text("Height"));

  ui.horizontal(|ui| {
    ui.label("Color:");
    ui.color_edit_button_rgb(&mut data.color);
  });
}
