# Player Integration Tasks

From implementation plan phases 5.3-5.5.

## Phase 5.3: Player-World Collision

- [ ] Generate collision mesh from chunk pixel data (marching squares)
- [ ] Update collision mesh when chunks change (dirty tracking)
- [ ] Integrate with player movement controller
- [ ] Douglas-Peucker simplification (1.0 pixel tolerance)

## Phase 5.4: Player Tools

- [ ] Dig tool: remove pixels in radius around cursor
- [ ] Place tool: add selected material pixels
- [ ] Tool switching UI or keybinds
- [ ] Tool range indicator

## Phase 5.5: Camera & Spawn

- [ ] Camera follows player in play mode
- [ ] Free-cam toggle for creative mode
