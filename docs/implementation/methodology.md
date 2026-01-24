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

### No Duplicate Entrypoints

Never write the same function, type, or entrypoint twice with different `#[cfg]` attributes. This creates:

- Duplicate code that drifts out of sync
- Maintenance burden when signatures change
- Review confusion about which version is "real"

**Wrong:**

```rust
#[cfg(feature = "physics")]
pub fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(PhysicsWorld::default());
}

#[cfg(not(feature = "physics"))]
pub fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}
```

**Right:**

```rust
pub fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    #[cfg(feature = "physics")]
    commands.spawn(PhysicsWorld::default());
}
```

### Apply `#[cfg]` to Internals

Use conditional compilation on the smallest possible scope:

- Individual struct fields
- Single statements within a function
- Specific expressions or blocks
- Import statements

Keep the outer definition unconditional. Let the internals vary.

**For types:**

```rust
pub struct GameWorld {
    entities: Vec<Entity>,
    #[cfg(feature = "physics")]
    physics: PhysicsEngine,
}
```

**For functions:**

```rust
pub fn init_systems(app: &mut App) {
    app.add_systems(Update, movement);

    #[cfg(feature = "debug")]
    app.add_systems(Update, debug_overlay);
}
```

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

---

## Summary

1. Tests verify behavior, not implementation
2. Implement what's needed now, not what might be needed
3. One working primitive beats many partial ones
4. Defer abstraction until patterns emerge
5. Visual verification is valid verification
6. Plans describe what, not how
7. Apply `#[cfg]` to internals, never duplicate entrypoints
