mod clock_widget;
mod sky;

use bevy::prelude::*;

pub struct Ambiance2DPlugin;

impl Plugin for Ambiance2DPlugin {
  fn build(&self, app: &mut App) {
    app.add_plugins(sky::SkyPlugin);
    app.add_plugins(clock_widget::ClockWidgetPlugin);
  }
}
