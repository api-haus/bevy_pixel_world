# Level Editor: Simulation & Persistence Control

Implementation plan for runtime control over simulation and persistence.

## Overview

Add runtime control to bevy_pixel_world for level editor mode:

- Enable/disable persistence at runtime
- Re-seed all chunks on demand
- Integrate with existing simulation pause/resume

See [Level Editor Mode Architecture](../arhitecture/editor/level-editor-mode.md) for design rationale.

## Phase 1: Persistence Enable/Disable

### Data Structures

**File:** `crates/bevy_pixel_world/src/world/control.rs`

Add `enabled` field to `PersistenceControl`:

```rust
pub struct PersistenceControl {
    enabled: bool,  // NEW - defaults to true
    current_path: Option<PathBuf>,
    next_request_id: u64,
    pending_requests: Vec<PersistenceRequestInner>,
}
```

### API

| Method | Description |
|--------|-------------|
| `disable()` | Sets `enabled = false` |
| `enable()` | Sets `enabled = true` |
| `is_enabled()` | Returns `enabled` |
| `is_active()` | Returns `enabled && current_path.is_some()` |

### System Guards

**File:** `crates/bevy_pixel_world/src/world/persistence_systems.rs`

Add early-return guards to:

| System | Guard |
|--------|-------|
| `dispatch_chunk_loads` | `if !persistence.is_enabled() { return; }` |
| `save_pixel_bodies_on_chunk_unload` | `if !persistence.is_enabled() { return; }` |
| `save_pixel_bodies_on_request` | `if !persistence.is_enabled() { return; }` |

### Loading State Skip

**File:** `crates/bevy_pixel_world/src/world/streaming/window.rs`

In `update_streaming_windows`, when spawning new chunks:

```rust
// Current: always go to Loading if persistence available
if has_persistence {
    slot.lifecycle = ChunkLifecycle::Loading;
}

// New: skip Loading if persistence disabled
if has_persistence && persistence_control.map_or(true, |p| p.is_enabled()) {
    slot.lifecycle = ChunkLifecycle::Loading;
}
// else: stays at InPool, will be picked up by dispatch_seeding
```

## Phase 2: Chunk Re-seeding API

### Event Definition

**File:** `crates/bevy_pixel_world/src/world/control.rs`

```rust
#[derive(Event)]
pub struct ReseedAllChunks;
```

### Re-seed Handler

**File:** `crates/bevy_pixel_world/src/world/streaming/seeding.rs`

New system to handle re-seed requests:

```rust
pub(crate) fn handle_reseed_request(
    mut events: EventReader<ReseedAllChunks>,
    mut worlds: Query<&mut PixelWorld>,
    mut loaded_data: ResMut<LoadedChunkDataStore>,
) {
    for _ in events.read() {
        // Clear any cached persistence data
        loaded_data.store.clear();
        loaded_data.bodies.clear();

        // Transition Active chunks back to Seeding
        for mut world in &mut worlds {
            for slot in world.slots_mut() {
                if slot.lifecycle == ChunkLifecycle::Active {
                    slot.lifecycle = ChunkLifecycle::Seeding;
                    slot.from_persistence = false;
                }
            }
        }
    }
}
```

### System Registration

**File:** `crates/bevy_pixel_world/src/world/plugin.rs`

Add to streaming plugin:

```rust
app.add_event::<ReseedAllChunks>();
app.add_systems(
    PreUpdate,
    handle_reseed_request.before(dispatch_seeding),
);
```

## Phase 3: Public Exports

**File:** `crates/bevy_pixel_world/src/lib.rs`

Add to existing export block:

```rust
pub use world::control::{
    // ... existing exports ...
    ReseedAllChunks,  // NEW
};
```

## Phase 4: Editor Integration

**File:** `crates/game/src/editor/noise.rs`

Integrate control into noise panel:

```rust
fn noise_panel_system(
    // ... existing params ...
    mut reseed_events: EventWriter<ReseedAllChunks>,
    mut persistence: ResMut<PersistenceControl>,
    mut simulation: ResMut<SimulationState>,
) {
    // On noise profile change
    if profile.dirty {
        reseed_events.write(ReseedAllChunks);
        profile.dirty = false;
    }
}
```

Editor mode toggle (future UI):

```rust
// Enter edit mode
simulation.pause();
persistence.disable();
reseed_events.write(ReseedAllChunks);

// Exit edit mode (start playtest)
persistence.enable();
simulation.resume();
```

## File Summary

| File | Changes |
|------|---------|
| `world/control.rs` | Add `enabled` field, `disable()`/`enable()` methods, `ReseedAllChunks` event |
| `world/persistence_systems.rs` | Add `is_enabled()` guards to save/load systems |
| `world/streaming/window.rs` | Skip `Loading` state when persistence disabled |
| `world/streaming/seeding.rs` | Add `handle_reseed_request` system |
| `world/plugin.rs` | Register event and system |
| `lib.rs` | Export `ReseedAllChunks` |
| `game/src/editor/noise.rs` | Trigger re-seed on profile change |

## Verification

1. `cargo build -p bevy_pixel_world` - Compile library changes
2. `cargo build -p game` - Compile editor integration
3. `just dev` - Run with editor
4. Test sequence:
   - Open noise panel
   - Change ENT value
   - Verify chunks regenerate with new noise
   - Exit edit mode, make changes, verify persistence works
