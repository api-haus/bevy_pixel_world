# Player Dig/Place Tools

Phase 5.4 implementation: tools for player interaction with pixel world.

## Overview

Allow player to dig (remove) and place pixels using tools rather than creative mode painting.

## Tasks

- [ ] Define tool state resource (current tool, selected material, radius)
- [ ] Implement dig tool (remove pixels in radius around cursor)
- [ ] Implement place tool (add selected material pixels)
- [ ] Add tool switching keybinds (1/2 or tab cycle)
- [ ] Add tool range indicator (cursor radius preview)
- [ ] Add material selection UI or hotbar
- [ ] Limit tool range from player position
- [ ] Add cooldown or resource cost (optional game mechanic)

## Controls

| Action | Input | Behavior |
|--------|-------|----------|
| Dig | Left click | Remove pixels in radius |
| Place | Right click | Add selected material |
| Switch tool | 1/2 or Tab | Cycle between tools |
| Change radius | Scroll | Adjust tool size |

## Technical Details

### Tool State
```rust
#[derive(Resource)]
pub struct PlayerTools {
    pub current: ToolType,
    pub material: u8,
    pub radius: f32,
    pub max_range: f32,
}

pub enum ToolType {
    Dig,
    Place,
}
```

### Integration Points

- Use existing brush/painting infrastructure from creative mode
- Respect terrain collision (can't dig through what you're standing on without falling)
- Consider chunk dirty tracking for tool operations

## References

- docs/implementation/plan.md (Phase 5.4)
- Creative mode painting as reference implementation
