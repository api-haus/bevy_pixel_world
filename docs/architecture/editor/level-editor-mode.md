# Level Editor Mode

Runtime control over simulation and persistence for level editing.

## Overview

The level editor mode provides:

- **Non-destructive editing** - Level definitions remain separate from player state
- **Live preview** - Re-seed chunks to visualize noise profile changes
- **Mode switching** - Toggle between edit mode and playtest mode

## Storage Layer Separation

Two distinct storage layers serve different purposes:

```mermaid
flowchart TB
    subgraph Design["Design Time (Level Definition)"]
        YOL[".yol file<br/>(Yoleck level)"]
        Stamps["Stamps"]
        Noise["Noise profiles"]
        Entities["Entity placements"]
    end

    subgraph Runtime["Runtime (Player State)"]
        Save["world.save file"]
        Pixels["Pixel modifications"]
        Bodies["Body positions"]
        Progress["Game progress"]
    end

    YOL --> Stamps
    YOL --> Noise
    YOL --> Entities

    Stamps -->|" applied during seeding "| Save
    Noise -->|" generates terrain "| Save
    Bodies --> Save
    Pixels --> Save
```

| Layer | Storage | Content | Lifecycle |
|-------|---------|---------|-----------|
| **Level Definition** | `.yol` file | Stamps, noise profiles, entities | Authored at design time |
| **Player State** | `world.save` file | Pixel modifications, body positions | Runtime persistence |

This separation ensures level definitions are never corrupted by player actions.

## Operating Modes

### Edit Mode

```mermaid
stateDiagram-v2
    [*] --> EditMode: Enter editor

    state EditMode {
        Paused: Simulation Paused
        NoPersist: Persistence Disabled
        Preview: Re-seed on profile change
    }

    EditMode --> PlaytestMode: Start playtest
```

| Behavior | State |
|----------|-------|
| Simulation | Paused |
| Persistence | Disabled |
| Chunk loading | Skips persistence, goes straight to seeding |
| Player modifications | Discarded on re-seed |

### Playtest Mode

```mermaid
stateDiagram-v2
    [*] --> PlaytestMode: Start game

    state PlaytestMode {
        Running: Simulation Running
        Persist: Persistence Enabled
        Apply: Stamps applied during seeding
    }

    PlaytestMode --> EditMode: Return to editor
```

| Behavior | State |
|----------|-------|
| Simulation | Running |
| Persistence | Enabled |
| Chunk loading | Checks save file, applies stamps during seeding |
| Player modifications | Saved to `world.save` |

## Chunk Re-seeding

When noise profiles change in edit mode, all active chunks must regenerate:

```mermaid
sequenceDiagram
    participant Editor
    participant Control
    participant Chunks
    participant Seeder

    Editor->>Control: Noise profile changed
    Control->>Chunks: Clear loaded data cache
    Control->>Chunks: Transition Active → Seeding
    Chunks->>Seeder: Request new seed
    Seeder->>Chunks: Fresh terrain with new profile
```

### Re-seed Trigger Flow

1. Editor detects noise ENT change
2. `ReseedAllChunks` event dispatched
3. `LoadedChunkDataStore` cleared (no stale persistence data)
4. Active chunks transition to `Seeding` lifecycle state
5. Seeding system generates fresh content with new noise profile

## Chunk Lifecycle Integration

The editor mode integrates with the existing chunk lifecycle:

```mermaid
flowchart LR
    subgraph Normal["Normal Flow"]
        Pool --> Loading --> Seeding --> Active
    end

    subgraph EditMode["Edit Mode Flow"]
        direction LR
        Pool2[Pool] --> Seeding2[Seeding] --> Active2[Active]
        Active2 -->|" ReseedAllChunks "| Seeding2
    end
```

In edit mode:
- Loading state is **skipped** (no persistence to check)
- Active chunks can **regress** to Seeding state on re-seed request

## Control Resources

### SimulationState (Existing)

Controls cellular automata execution:

| Method | Effect |
|--------|--------|
| `pause()` | Stops simulation updates |
| `resume()` | Continues simulation updates |
| `is_running()` | Query current state |

### PersistenceControl (Extended)

Controls save file I/O:

| Method | Effect |
|--------|--------|
| `disable()` | Prevents all persistence I/O |
| `enable()` | Allows persistence I/O |
| `is_enabled()` | Query enabled state |
| `is_active()` | Returns `enabled && path.is_some()` |

### ReseedAllChunks (New)

Event to trigger global chunk regeneration:

```
Event dispatch → Clear cache → Active→Seeding transition → Fresh seeding
```

## Mode Transition Workflow

```mermaid
sequenceDiagram
    participant UI as Editor UI
    participant Sim as SimulationState
    participant Pers as PersistenceControl
    participant World as PixelWorld

    Note over UI: Enter Edit Mode
    UI->>Sim: pause()
    UI->>Pers: disable()

    Note over UI: Editing...
    UI->>World: ReseedAllChunks (on profile change)

    Note over UI: Start Playtest
    UI->>Pers: enable()
    UI->>Sim: resume()

    Note over UI: Playing...

    Note over UI: Return to Edit
    UI->>Sim: pause()
    UI->>Pers: disable()
    UI->>World: ReseedAllChunks (discard player changes)
```

## Persistence Guard Locations

Systems that require persistence guards:

| System | Guard Behavior |
|--------|----------------|
| `dispatch_chunk_loads` | Skip if `!is_enabled()` |
| `save_pixel_bodies_on_chunk_unload` | Skip if `!is_enabled()` |
| `save_pixel_bodies_on_request` | Skip if `!is_enabled()` |
| `update_streaming_windows` | Skip Loading state if `!is_enabled()` |

## Related Documentation

- [Streaming Window](../streaming/streaming-window.md) - Chunk lifecycle management
- [Chunk Persistence](../persistence/chunk-persistence.md) - Save file format
- [Chunk Seeding](../chunk-management/chunk-seeding.md) - Procedural generation
- [Architecture Overview](../README.md)
