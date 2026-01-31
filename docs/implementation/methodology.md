# Implementation Methodology

Principles guiding development of this codebase.

---

## Testing

### No Trivial Tests

Don't test that the language works. Tests that verify getters return what setters set, or that constructors initialize
fields, assert nothing beyond "Rust compiles." These tests:

- Catch zero bugs
- Create maintenance burden
- Give false confidence through line coverage

### Test Behavior, Not Implementation

A test should verify observable behavior that matters to the system. If a test wouldn't catch a real bug that would
affect users or downstream code, don't write it.

**Valuable tests:**

- Integration tests verifying components work together
- End-to-end tests verifying user-visible behavior
- Property tests for mathematical invariants (e.g., coordinate roundtrips)

**Not valuable:**

- "Does this struct hold this value?"
- "Does this function return what I just passed in?"

### Visual Verification

For graphical systems, a runnable example that displays output is often more valuable than automated tests. A human
glancing at a UV gradient immediately knows if it's correct. An automated test checking pixel values is brittle and
verbose.

Examples serve as both verification and documentation.

### Test Location

Keep tests in `tests/` directories, not inline `#[cfg(test)]` modules. This:

- Keeps source files focused on implementation
- Makes tests easier to find and navigate
- Separates concerns cleanly

For unit tests that need private access, use `#[path]` to reference external test files:

```rust
#[cfg(test)]
#[path = "tests/mymodule.rs"]
mod tests;
```

---

## API Design

### Write What You Need

Implement only what the current task requires. Not what might be useful. Not what would make a complete API. Not what
other libraries provide.

When building a module:

1. Identify the exact operation needed by calling code
2. Implement that operation
3. Stop

### Resist Completeness

A module that draws rectangles doesn't need to draw circles, lines, polygons, and Bézier curves. If calling code needs
rectangles, write rectangle drawing. When calling code needs circles, add circles then.

The cost of unused code:

- Reading and understanding code that does nothing
- Maintaining code that does nothing
- Testing code that does nothing
- Potential bugs in code that does nothing

### One Primitive, Fully Working

A single well-implemented primitive is better than many half-implemented ones. If a system needs one operation to work
correctly, make that operation work correctly. Don't spread effort across operations that aren't needed yet.

### Don't Predict the Future

Code written for hypothetical future requirements:

- Often misses actual future requirements
- Creates abstractions around the wrong boundaries
- Adds complexity for no present benefit

When the future arrives, you'll understand the actual requirements. Code written then will be better than code written
now based on guesses.

### Explicit Over Flexible

An API that does one thing explicitly is easier to understand than an API that does many things through configuration.
If there are two use cases, consider two functions rather than one function with a mode parameter.

Flexibility has a cost. Pay it only when needed.

---

## Code Organization

### Minimal Surface Area

A module should expose only what callers need. Internal helpers, intermediate data structures, and implementation
details stay private. A small public API is easier to understand, use correctly, and maintain.

### Defer Abstraction

Don't create abstractions before seeing the pattern repeat. Three concrete implementations reveal what abstraction, if
any, is appropriate. Premature abstraction locks in the wrong boundaries.

When in doubt, write concrete code. Extract abstractions later when the shape becomes clear.

---

## Conditional Compilation

Apply `#[cfg]` at the exact point of divergence. No higher, no lower.

**Wrong** — duplicated functions:

```rust
#[cfg(feature = "physics")]
pub fn setup(commands: Commands) { spawn_camera(); spawn_physics(); }

#[cfg(not(feature = "physics"))]
pub fn setup(commands: Commands) { spawn_camera(); }
```

**Wrong** — two code paths hiding inside one function:

```rust
fn setup(commands: Commands) {
    #[cfg(feature = "physics")]
    { spawn_camera(); spawn_physics(); }

    #[cfg(not(feature = "physics"))]
    { spawn_camera(); }
}
```

**Right** — cfg at the divergence:

```rust
fn setup(commands: Commands) {
    spawn_camera();
    #[cfg(feature = "physics")]
    spawn_physics();
}
```

This applies to struct fields, function parameters, statements, and expressions.

If a function only exists for one feature, gate both definition and call site:

```rust
#[cfg(feature = "rendering")]
fn upload_textures(images: ResMut<Assets<Image>>) { /* ... */ }

#[cfg(feature = "rendering")]
app.add_systems(Update, upload_textures);
```

---

## Cross-Platform Implementation

When writing code that targets multiple platforms (native + WASM), keep platform differences isolated to trait implementations.

### Uniform Data Structures

**Wrong** — different types per platform:

```rust
struct Foo {
    #[cfg(not(target_family = "wasm"))]
    backend: Arc<dyn Backend>,
    #[cfg(target_family = "wasm")]
    backend: Option<Arc<dyn Backend>>,
}
```

**Right** — same type everywhere:

```rust
struct Foo {
    backend: Option<Arc<dyn Backend>>,
}
```

The struct definition should be identical on all platforms. Use `Option` if a field may be absent on some platforms.

### Platform Logic in Trait Implementations Only

**Wrong** — conditional logic in business code:

```rust
fn is_ready(&self) -> bool {
    #[cfg(target_family = "wasm")]
    { self.name.is_some() }
    #[cfg(not(target_family = "wasm"))]
    { self.file.is_some() }
}
```

**Right** — same logic everywhere:

```rust
fn is_ready(&self) -> bool {
    self.name.is_some()
}
```

The only places `#[cfg(target_family)]` should appear:

1. Module declarations: `#[cfg(not(target_family = "wasm"))] mod native;`
2. Trait implementations inside those modules

Business logic should be platform-agnostic.

### The Abstraction Boundary

```
┌─────────────────────────────────────────┐
│  Business Logic (platform-agnostic)     │
│  - No #[cfg(target_family)] here        │
└─────────────────────────────────────────┘
                    │
                    ▼ uses trait
┌─────────────────────────────────────────┐
│  Abstraction Layer                      │
│  - Resource with platform-specific impl │
│  - Shared command/result types          │
└─────────────────────────────────────────┘
                    │
        ┌───────────┴───────────┐
        ▼                       ▼
┌───────────────┐       ┌───────────────┐
│ native.rs     │       │ wasm.rs       │
│ #[cfg] here   │       │ #[cfg] here   │
└───────────────┘       └───────────────┘
```

`#[cfg]` appears at the leaves, never at the trunk.

### Naming

**Wrong** — implementation detail or platform in name:

```rust
fn for_io_dispatcher(name: String) -> Self
fn wasm_with_dispatcher(name: String) -> Self
```

**Right** — describes behavior:

```rust
fn with_name_only(name: String) -> Self
```

API names describe behavior, not implementation details or platform constraints.

### No Platform-Specific Comments on Shared Code

**Wrong:**

```rust
/// The loaded file. None on WASM (handle is in worker).
file: Option<Arc<File>>,
```

**Right:**

```rust
/// The loaded file.
file: Option<Arc<File>>,
```

If a field needs a platform-specific explanation, the abstraction is leaking. Fix the abstraction.

### One Name Per Concept

**Wrong:**

```rust
save: Option<Arc<WorldSave>>,  // the object
current_save: Option<String>,  // ...the name? the current one?
```

**Right:**

```rust
world_save: Option<Arc<WorldSave>>,  // clearly the object
save_name: Option<String>,           // clearly just the name
```

Distinct concepts need distinct, self-explanatory names.

---

## Documentation

### Plans Stay High-Level

Implementation plans describe *what* to build, not *how* to build it. Code belongs in the codebase, not in planning
documents.

**Include:**

- Data structure definitions (structs, enums)
- API signatures (method names and purposes)
- System diagrams (mermaid for complex flows)
- Acceptance criteria

**Exclude:**

- Implementation code (function bodies, algorithms)
- Example usage code
- Test code

### Diagrams Over Prose

For systems with state machines, data flow, or sequencing, a mermaid diagram communicates more clearly than paragraphs
of text. Use:

- `stateDiagram-v2` for lifecycles
- `flowchart` for data/control flow
- `block-beta` for spatial layouts

### Rust Pseudocode

Use Rust syntax for pseudocode in documentation. Syntax highlighting still works even when the code isn't valid Rust.
This communicates intent through familiar idioms without implementation noise.

**Why Rust pseudocode:**

- Readers already know Rust syntax
- Highlighting works in editors and rendered markdown
- Generics, traits, and types express constraints naturally
- Looks like real code, reads like specification

**Techniques:**

**1. Elide implementations with `...`**

```rust
trait Layer {
    type Element: Copy + Default;
    const SAMPLE_RATE: u32;
    fn upload(&self, gpu: &mut GpuContext) { ... }
}
```

The `{ ... }` says "there's an implementation, it's not the point." Valid Rust would require a body or `;`.

**2. Use array syntax for conceptual storage**

```rust
BrickLayer<const GRID: usize = 16> {
    id: [BrickId; CHUNK_SIZE²],      // ² isn't valid, but intent is clear
    damage: [u8; GRID²],
}
```

Superscript `²` isn't Rust, but readers understand "squared" faster than `CHUNK_SIZE * CHUNK_SIZE`.

**3. Inline comments as specification**

```rust
struct Chunk {
    material: [u8; N],     // always present
    color: [u8; N],        // opt-in, Default Bundle
    flags: [u8; N],        // opt-in, Default Bundle
}
```

Comments describe invariants, not implementation details.

**4. Show data flow with assignment chains**

```rust
// Hit detection flow
brick_id = brick_id_layer[pixel_pos];
damage = damage_layer[brick_id];
damage += hit_strength;
if damage >= threshold { destroy_brick(brick_id); }
```

Not valid Rust (no types, no bounds checking), but the algorithm is immediately clear.

**5. Trait bounds as contracts**

```rust
fn register_layer<L: Layer + Send + Sync + 'static>(config: LayerConfig) { ... }
```

The bounds communicate requirements without showing implementation.

**6. Const generics for configuration**

```rust
BrickLayer<const GRID: usize = 16>    // default value shown
HeatLayer<const SAMPLE_RATE: u32 = 4>
```

Default values in const generics show recommended usage.

**7. Type derivation as prose**

```rust
// BrickId type derived from GRID:
// GRID² ≤ 256 → u8
// GRID² > 256 → u16
type BrickId = /* derived */;
```

When exact typing is complex, describe the rule instead of faking it.

**8. Enum variants for state machines**

```rust
enum ChunkState {
    Unloaded,
    Loading { progress: f32 },
    Ready { data: ChunkData },
    Dirty { since: Tick },
}
```

Enum syntax naturally expresses states and their associated data.

**When to break which rules:**

| Technique | Rule Broken | Effect |
|-----------|-------------|--------|
| `{ ... }` | Missing function body | "Implementation exists, not shown" |
| `²` | Invalid identifier | Mathematical clarity |
| No types | Missing type annotations | Focus on algorithm |
| `/* derived */` | Incomplete type | "Type exists, rule determines it" |
| Inline comments | Style convention | Specification alongside structure |

**Don't break:**

- Syntax that won't highlight (mismatched braces, invalid keywords)
- Structure that obscures intent (clever tricks over clarity)
- Type relationships that matter for understanding

The goal: a reader familiar with Rust should understand the design without wondering "wait, is this valid?"

---

## Summary

1. Tests verify behavior, not implementation
2. Implement what's needed now, not what might be needed
3. One working primitive beats many partial ones
4. Defer abstraction until patterns emerge
5. Visual verification is valid verification
6. Plans describe what, not how
7. Apply `#[cfg]` at the exact point of divergence
8. Cross-platform: uniform structs, `#[cfg]` only in trait impls
9. Rust pseudocode: break rules strategically for clarity
