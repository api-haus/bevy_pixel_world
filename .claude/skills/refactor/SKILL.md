---
name: refactor
description: Review codebase for refactoring opportunities against architectural docs
---

# Refactoring Review

Review the codebase for refactoring opportunities that reduce cognitive load by better representing concepts outlined in architectural documentation.

## Scope

Target: `$ARGUMENTS`

If no argument provided, prompt for a crate or module path.

## Process

1. **Read architectural docs** - Review `docs/architecture/` for design concepts
2. **Read implementation plans** - Check `docs/implementation/` for context
3. **Run static analysis** - Execute `debtmap analyze <target> --threshold-complexity 10` for complexity metrics
4. **Run clippy** - Execute `cargo clippy -p <crate> -- -W clippy::cognitive_complexity` for lint warnings
5. **Analyze target code** - Examine the specified crate/module
6. **Identify opportunities** - Find misalignments between code and architecture

## Evaluation Criteria

Review against:

- **Architectural alignment** - Does code structure reflect documented concepts?
- **Cognitive load** - Are abstractions clear? Could naming be improved?
- **Complexity metrics** - Functions with cognitive complexity >15 or debtmap score >30 need attention
- **Rust best practices** - Idiomatic patterns, proper error handling, type safety
- **Module boundaries** - Do modules have clear, single responsibilities?

## Output Format

Present findings as:

```markdown
## Refactoring Opportunities: [target]

### 1. [Issue Title]
**Current:** Brief description of current state
**Proposed:** What the change would be
**Rationale:** Why this reduces cognitive load or improves alignment

### 2. [Next Issue]
...
```

## Constraints

- Do NOT implement changes - only identify and describe opportunities
- Focus on structural improvements, not feature additions
- Defer changes that require incomplete features (note in `docs/implementation/plan.md`)
- Reference architectural docs when explaining rationale

## Prerequisites

- `debtmap` CLI installed (`cargo install debtmap`)
