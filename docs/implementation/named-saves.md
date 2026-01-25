# Named Saves Implementation

Implementation guide for the named save system.

## Scope

This document covers:

- Changes to `PersistenceControl` API
- Copy-on-write save mechanics
- Removal of embedded auto-save timer

## Files to Modify

| File | Changes |
|------|---------|
| `world/control.rs` | Replace `request_save()` with `save(name)` |
| `world/plugin.rs` | Remove `tick_auto_save_timer` system |
| `lib.rs` | Update `PersistenceConfig` with `load_save` field |
| `persistence/mod.rs` | Add `WorldSave::copy_to()` method |

## API Changes

### PersistenceControl

**Remove:**
- `AutoSaveConfig` struct
- `auto_save` field
- `time_since_save` field
- `request_save()` method
- `request_chunk_save()` method

**Add:**
- `base_dir: PathBuf` field
- `current_save: Option<String>` field
- `save(&mut self, name: &str) -> PersistenceHandle`
- `save_chunks(&mut self, name: &str) -> PersistenceHandle`
- `save_path(&self, name: &str) -> PathBuf`
- `list_saves(&self) -> io::Result<Vec<String>>`
- `delete_save(&self, name: &str) -> io::Result<()>`

### PersistenceConfig

**Remove:**
- Any auto-save related fields

**Add:**
- `load_save: Option<String>` - which save to load at startup
- `base_dir: Option<PathBuf>` - explicit base directory override
- `.load(name: &str) -> Self` builder method

## Copy-on-Write Implementation

### WorldSave::copy_to

```
pub fn copy_to(&self, new_path: &Path) -> io::Result<WorldSave>

Steps:
1. Flush pending writes to self (ensure source is consistent)
2. std::fs::copy(self.path, new_path)
3. WorldSave::open(new_path)
4. Return new handle
```

### Save Flow

```
save("name"):
1. Resolve path: base_dir.join(format!("{}.save", name))
2. If name == current_save:
   - Queue dirty chunks to PersistenceTasks
   - Flush to current WorldSave
3. Else:
   - Copy current WorldSave to new path
   - Swap WorldSaveResource to new handle
   - Queue dirty chunks
   - Flush to new WorldSave
   - (Old handle closes via RAII)
```

## System Changes

### Remove: tick_auto_save_timer

Delete the system that increments `time_since_save` and triggers automatic saves.

### Modify: flush_persistence_queue

Add `target_name` to `PersistenceRequestInner` so flush knows which file to write to.

## Migration

### Breaking Changes

| Before | After |
|--------|-------|
| `persistence.request_save()` | `persistence.save("world")` |
| `persistence.request_chunk_save()` | `persistence.save_chunks("world")` |
| `AutoSaveConfig::default()` | Consumer implements own timer |

### Consumer Migration

```rust
// Before
fn auto_save(mut persistence: ResMut<PersistenceControl>) {
    // Plugin handles timing internally
}

// After
fn auto_save(
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    mut persistence: ResMut<PersistenceControl>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(5.0, TimerMode::Repeating));
    timer.tick(time.delta());
    if timer.just_finished() {
        persistence.save("world");
    }
}
```

## Testing

### Unit Tests

- `save_path()` returns correct path for name
- `list_saves()` finds all `.save` files in base_dir
- `delete_save()` removes file and returns Ok

### Integration Tests

- Save to same name: dirty chunks flushed, file updated
- Save to different name: source unchanged, new file complete
- Load specific save: correct data loaded
- Crash during copy: source intact
