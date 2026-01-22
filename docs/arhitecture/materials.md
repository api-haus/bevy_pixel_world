# Materials

Material definitions and interaction system.

## Overview

Materials define how pixels behave in the simulation. Each pixel's Material field (u8) indexes into a material registry containing up to 256 material definitions.

Material ID 0 is reserved for **void** (empty space).

## Material Properties

### Identity & Rendering

| Property | Type | Description |
|----------|------|-------------|
| `name` | string | Display name for debugging and UI |
| `palette_range` | (u8, u8) | Start and end indices in the color palette for visual variation |

### Physical State & Movement

| Property | Type | Description |
|----------|------|-------------|
| `state` | enum | `solid`, `powder`, `liquid`, `gas` - determines movement rules |
| `density` | u8 | Relative weight; denser materials sink below lighter ones |
| `dispersion` | u8 | How far liquids/powders spread horizontally per tick |

**State behaviors:**
- `solid` - Static, does not move, supports neighbors
- `powder` - Falls, piles up, slides off slopes
- `liquid` - Falls, flows horizontally to fill containers
- `gas` - Rises, disperses in all directions

**Gas handling - dual approach:**

Gases can be simulated as either particles or cellular automata pixels, depending on the desired behavior:

| Approach | Use When | Examples |
|----------|----------|----------|
| **Particles** | Fast vertical movement, visual effects, transient | Steam plumes, smoke trails, vapor |
| **CA with `gas` state** | Persistent clouds, material interactions needed | Fog, poison gas, settled smoke |

Choose particles for rising/dispersing gases that should naturally leave the simulation bounds. Choose CA `gas` state for dense clouds that need to interact with other materials or persist over time.

See [Particles](particles.md) for detailed gas handling rationale.

### Durability

| Property | Type | Description |
|----------|------|-------------|
| `damage_threshold` | u8 | Damage value at which pixel is destroyed/transforms. `0` = indestructible |
| `destruction_product` | MaterialId | What this becomes when destroyed (wood → ash, stone → rubite). Void = disappears |

### Decay

| Property | Type | Description |
|----------|------|-------------|
| `decay_chance` | f32 | Probability (0.0-1.0) of transformation per decay pass. `0.0` = never decays |
| `decay_product` | MaterialId | What this becomes when decay triggers. Void = disappears (evaporation) |

**Decay examples:**
- Water with `decay_chance: 0.01`, `decay_product: void` → slow evaporation
- Leaves with `decay_chance: 0.005`, `decay_product: mulch` → gradual decomposition
- Corpse with `decay_chance: 0.02`, `decay_product: bone` → faster decay
- Stone with `decay_chance: 0.0` → never decays

See [Simulation](simulation.md) for how decay passes are scheduled.

### Thermal

| Property | Type | Description |
|----------|------|-------------|
| `ignition_threshold` | u8 | Heat level required to ignite. `0` = non-flammable. Lower = catches fire easier. Implies `flammable` tag when > 0 |
| `melting_threshold` | u8 | Heat level at which material melts/transforms. `0` = cannot melt |
| `melting_product` | MaterialId | What this becomes when melted (stone → lava, ice → water) |
| `base_temperature` | u8 | Heat this material emits to the heat layer (lava = 255, ice = 0) |

**Thermal examples:**
- Wood: `ignition_threshold: 40` (catches fire easily), no melting
- Stone: `ignition_threshold: 0` (cannot burn), `melting_threshold: 240`, `melting_product: lava`
- Metal: `ignition_threshold: 0` (cannot burn), `melting_threshold: 220`, `melting_product: molten_metal`
- Ice: `ignition_threshold: 0`, `melting_threshold: 30`, `melting_product: water`
- Lava: `base_temperature: 255` (emits maximum heat)

**Note:** Non-flammable materials (stone, metal) don't ignite but still conduct heat and glow visually (orange → red → white) before melting. Rendering uses heat layer temperature to tint these materials.

See [Simulation](simulation.md) for heat layer propagation and effects.

### Particle Behavior

| Property | Type | Description |
|----------|------|-------------|
| `particle_gravity` | f32 | Gravity multiplier when this material is a particle. `1.0` = normal, `<0` = rises |

**Particle gravity examples:**
- Stone/Metal: `particle_gravity: 1.0` (falls normally)
- Water: `particle_gravity: 0.8` (slightly slower fall)
- Steam: `particle_gravity: -0.3` (rises)
- Smoke: `particle_gravity: -0.2` (rises slowly)
- Dust: `particle_gravity: 0.6` (falls slowly)

See [Particles](particles.md) for the full particle system documentation.

## Material Tags

Tags categorize materials for interaction targeting. A material can have multiple tags.

### Composition Tags

| Tag | Description | Examples |
|-----|-------------|----------|
| `stone` | Stone-like minerals | granite, basalt, marble |
| `crystal` | Crystal structures | quartz, mana_crystal, diamond |
| `metal` | Metallic materials | iron, copper, gold, enchanted_steel |
| `organic` | Living or once-living matter | wood, flesh, leaves |
| `granular` | Made from smaller particles | sand, soil, gravel |

### Property Tags

| Tag | Description |
|-----|-------------|
| `flammable` | Can catch fire (implied by `ignition_threshold > 0`) |
| `conductive` | Transmits electricity |
| `magical` | Interacts with mana-based systems |

**Note:** State tags (`solid`, `liquid`, `powder`, `gas`) are not needed - use the `state` property instead. Tags are for cross-cutting categories that don't map to physical state.

## Material Interactions

Interactions define what happens when materials contact each other. They use a tag-based system with specific material overrides.

### Interaction Definition

```
material_name:
  tags: [tag1, tag2, ...]
  interactions:
    - target: <tag or material_id>
      effect: <effect_type>
      rate: <optional, default 1>
    - target: specific_material   # override for edge cases
      effect: none
```

### Interaction Types

| Effect | Description |
|--------|-------------|
| `diffuse` | Spread into target material, mixing or diluting |
| `corrode` | Deal damage to target each tick |
| `ignite` | Set target on fire if `ignition_threshold > 0` |
| `transform` | Change target into another material |
| `displace` | Swap positions (automatic from density, but can force) |
| `none` | Explicitly no interaction (for overrides) |

### Interaction Resolution

When pixel A contacts pixel B:

1. Check A's interactions for B's specific material ID (override)
2. If no override, check A's interactions for any of B's tags
3. Apply first matching effect
4. Reciprocally check B's interactions with A

### Example: Concentrated Mana

```
concentrated_mana:
  tags: [liquid, magical]
  state: liquid
  density: 30
  dispersion: 4
  damage_threshold: 0  # indestructible

  interactions:
    - target: liquid
      effect: diffuse       # spreads into water and other liquids
      rate: 2
    - target: metal
      effect: corrode       # dissolves metals
      rate: 3
    - target: enchanted_steel
      effect: none          # override: immune to mana corrosion
```

### Example: Fire Spread

```
# Burning flag propagation (not a material interaction, but similar pattern):
# A burning pixel checks neighbors each tick:
burning_propagation:
  - target: flammable             # materials with ignition_threshold > 0
    effect: ignite
    chance: 0.3                   # 30% chance per tick per neighbor
  - target: wet                   # wet flag on pixel
    effect: none                  # wet pixels cannot ignite
```

### Example: Water

```
water:
  tags: [liquid]
  state: liquid
  density: 50
  dispersion: 5
  damage_threshold: 0         # indestructible by damage
  decay_chance: 0.01          # slow evaporation over time
  decay_product: void

  interactions:
    - target: powder
      effect: transform    # powder becomes wet variant or mud
    - target: organic
      effect: none         # just makes it wet (flag), no material change
```

## Material Registry

At runtime, materials are stored in a registry indexed by Material ID:

```
MaterialRegistry:
  materials: [Material; 256]
  tag_index: HashMap<Tag, Vec<MaterialId>>  # for fast tag lookups
```

The tag index accelerates interaction checks - instead of iterating all tags on a material, look up which materials have a given tag.

## Related Documentation

- [Pixel Format](pixel-format.md) - How material ID is stored per pixel
- [Simulation](simulation.md) - Passes, interaction processing, decay scheduling, heat layer
- [Particles](particles.md) - Free-form particle system, `particle_gravity` usage
- [Chunk Seeding](chunk-seeding.md) - How materials are placed during generation
- [Architecture Overview](README.md)
