# Named Saves System

Implement consumer-controlled save management with complete world snapshots.

## Overview

The named save system provides:
- **Named save files** - Save to `"primary"`, `"backup"`, `"recovery"`, etc.
- **Complete snapshots** - Every save contains the entire world state
- **Consumer control** - Game code manages save timing (no embedded timers)
- **Copy-on-write** - Saves to new names copy from the loaded save

## Completed

- [x] `save()` method for flushing to current save file
- [x] `save_to(path)` method signature (API exists)
- [x] `PersistenceHandle` for tracking completion
- [x] `DeleteSave` IoCommand (native)

## Tasks

- [ ] Implement copy-on-write in `save_to()` (currently ignores target_path)
- [ ] Add IoDispatcher CopyTo command for copy-on-write
- [ ] Add `save_path(name)` utility to return full path
- [ ] Add `list_saves()` utility to return available save names
- [ ] Add named save API wrapper (string names instead of paths)

## File Layout
```
~/.local/share/<app_name>/saves/
├── primary.save    # Main game save
├── backup.save     # Player-created backup
└── recovery.save   # Crash recovery
```

## API Design

```rust
// Save to current loaded file (IMPLEMENTED)
persistence.save();

// Save to new file (copy-on-write from loaded) - API exists, copy-on-write TODO
persistence.save_to("/path/to/backup.save");

// Desired: Named save helpers
persistence.save_named("backup");
let saves = persistence.list_saves();
```

## References

- docs/architecture/persistence/named-saves.md
- Related: docs/todo/recovery-persistence.md
- TODO: world/control.rs:191
