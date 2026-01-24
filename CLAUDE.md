# Project Guidelines

## Testing

- No trivial unit tests. Don't test that getters return what setters set.
- Integration and E2E tests only. Tests should catch real bugs.
- Visual verification via runnable examples is preferred for graphical systems.
- Keep tests in `tests/` directory, not inline `#[cfg(test)]` modules.

## API Design

- Write only what's needed for the current task. Stop when done.
- Resist completeness. Don't add operations "because a complete API would have them."
- Don't predict the future. Code for hypothetical requirements is usually wrong.
- One working primitive beats many partial ones.

## Code Organization

- Defer abstraction until patterns repeat. Three concrete cases reveal the right abstraction.
- Minimal public surface. Expose only what callers need.

## Conditional Compilation

- Never duplicate functions, types, or entrypoints for `#[cfg]` gating.
- Apply `#[cfg]` to inner fields, statements, and scopes instead.
- One function/type definition with conditional internals, not two definitions with conditional attributes.

## Documentation

- Plans describe *what* to build, not *how*.
- Data structure definitions are permitted in plans. Implementation code is not.
- Use mermaid diagrams for complex systems (state machines, data flow).

## References

See `docs/implementation/methodology.md` for full rationale.
