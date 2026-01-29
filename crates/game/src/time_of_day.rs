use bevy::prelude::*;

use crate::config::ConfigLoaded;

/// Current time of day in the simulation.
#[derive(Resource, Debug, Clone)]
pub struct TimeOfDay {
  /// Current hour (0.0 - 24.0)
  pub hour: f32,
  /// Whether time progression is paused
  pub paused: bool,
}

pub struct TimeOfDayPlugin;

impl Plugin for TimeOfDayPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_systems(Startup, init_time_of_day)
      .add_systems(FixedUpdate, advance_time);
  }
}

fn init_time_of_day(mut commands: Commands, config: Res<ConfigLoaded>) {
  commands.insert_resource(TimeOfDay {
    hour: config.day_cycle.start_hour,
    paused: false,
  });
}

fn advance_time(mut time_of_day: ResMut<TimeOfDay>, config: Res<ConfigLoaded>, time: Res<Time>) {
  if time_of_day.paused {
    return;
  }

  let hours_per_second = 1.0 / config.day_cycle.seconds_per_hour;
  time_of_day.hour += time.delta_secs() * hours_per_second;

  // Wrap around at 24 hours
  if time_of_day.hour >= 24.0 {
    time_of_day.hour -= 24.0;
  }
}
