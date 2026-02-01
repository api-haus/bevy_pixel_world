# Island Scheduling Ideas

> **Status:** Analysis complete. Likely not worth pursuing for current window size.

Scheduling disconnected "islands" of dirty tiles in parallel, with each island running its own checkerboard phases.

## Concept

```
Current:   Phase A → B → C → D  (sequential, all tiles)

Proposed:  Island 1: A → B → C → D  ┐
           Island 2: A → B → C → D  ├─ parallel
           Island 3: A → B → C → D  ┘
```

If islands don't touch, their phase sequences can overlap.

## Detection Algorithms

| Algorithm | Complexity | Notes |
|-----------|------------|-------|
| Union-Find | O(n·α(n)) ≈ O(n) | Near-constant per-op, good for sparse |
| BFS flood | O(n + edges) | Simple, cache-friendly for small regions |

For n = dirty tile count (typically 100-1000 during activity).

## Cost-Benefit

### Detection Cost
- Scan dirty tiles + check 8 neighbors: O(n)
- Union/flood operations: O(n)
- **~20-30 ops per dirty tile**

### When It Helps
- Multiple distinct islands (≥2)
- Islands large enough that parallel execution matters
- Phase barrier cost > detection cost

## Typical Scenarios

| Scenario | Dirty Tiles | Islands | Worth It? |
|----------|-------------|---------|-----------|
| Idle world | ~10-50 | 1-3 small | No - too few tiles |
| Single explosion | ~200 | 1 | No - single region |
| Two distant players | ~400 | 2 | Maybe |
| Full chaos | ~2000+ | 1 large | No - merged region |
| Sparse activity spots | ~300 | 3-5 | Yes - best case |

## Why Probably Not Worth It

The 4×3 window (12k tiles) is small enough that:

1. **Activity connects** - Gravity chains, water flow create connected regions
2. **Detection overhead measurable** - Relative to simulation work
3. **Rayon saturates cores** - Already parallel within phases

The window where islands help is narrow:
- Too few dirty tiles → overhead dominates
- Too many → single merged island
- Sweet spot (3-5 isolated islands) uncommon in physics sim

## Better Optimization Targets

1. SIMD within tile simulation
2. Dirty rect compression (coalesce neighbors)
3. Reduce phase barrier overhead (work stealing)
4. Tile skip optimization (cooldown already exists)

## Validation Experiment

If pursuing anyway:

```rust
// Log for representative gameplay
let dirty_count = tiles_by_phase.iter().map(|v| v.len()).sum::<usize>();
let islands = count_islands(&all_dirty_tiles); // BFS
log::debug!("dirty={} islands={}", dirty_count, islands);
```

If islands rarely > 1, skip the optimization.
