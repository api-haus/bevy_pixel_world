//! Tracy profiler initialization.

use tracing_subscriber::prelude::*;
use tracing_tracy::TracyLayer;

/// Initialize Tracy profiling.
///
/// Call this early in your application (e.g., in `main()` before `App::run()`).
/// Requires the Tracy profiler to be connected to visualize data.
pub fn init_tracy() {
  tracing_subscriber::registry()
    .with(TracyLayer::default())
    .init();
}
