# LLM Development Cases

Documented cases from spec-driven AI development of sim2d. Each case captures a moment where Claude's decisions needed human intervention.

## Purpose

Build a corpus of concrete examples to inform better methodology for spec-driven LLM development. Not "how to prompt better"—these behaviors are fundamental to how LLMs work.

## Cases

| # | Title | Component | Signal Phrase |
|---|-------|-----------|---------------|
| [001](001-unnecessary-command-separation.md) | Unnecessary Command Separation | Persistence | "This is getting complex" |
| [002](002-displacement-fix-spiral.md) | Displacement Fix Spiral | Physics | "Let me fix" → "I see another issue" |
| [003](003-coordinate-restructure-overengineering.md) | Coordinate Restructure Over-Engineering | Core | restructure storage for interface change |
| [004](004-platform-specific-naming-leakage.md) | Platform-Specific Naming Leakage | Persistence | `#[cfg]` on struct fields, platform in names |
| [005](005-silent-library-rejection.md) | Silent Library Rejection | Rendering | "instead of X because..." |
| [006](006-tests-pass-bug-remains.md) | Tests Pass, Bug Remains | Pixel Bodies | "all tests pass" without testing bug |
| [007](007-symptom-fix-not-root-cause.md) | Symptom Fix, Not Root Cause | Rendering | fixing color instead of sizing |
| [008](008-fix-breaks-unrelated-systems.md) | Fix Breaks Unrelated Systems | Rendering | coordinate change breaks egui |
| [009](009-manual-verification-loop.md) | Manual Verification Loop | Testing | repeated demo runs instead of tests |
| [010](010-non-standard-algorithm-approach.md) | Non-Standard Algorithm Approach | Collision | "why are we connecting segments?" |

## Common Signals

Phrases that often precede problematic decisions:

- "This is getting complex" → about to add unnecessary abstraction
- "I need to add..." → about to expand scope
- "Let me also..." → scope creep
- "Let me fix" followed quickly by "I see another issue" → spiral incoming
- Proposing storage/structure changes for interface/presentation requests
- "All tests pass" → may not be testing the actual bug
- "Let me run the demo to verify" (repeatedly) → not writing automated tests
- "Instead of X because..." → about to silently pivot from user's request

## Human Intervention Patterns

Phrases that effectively redirect Claude:

- "why is X separate from Y?" → forces justification of separation
- "what practical use is there for it?" → forces concrete use case
- "that's nonsensical" → blunt rejection of over-engineering
- "Please use X" → explicit restatement after silent pivot
- Direct questions about purpose
- Showing visual evidence of failure (screenshots)
- "You did not fix the bug at all" → cuts through false verification
- "Stop running the demo, write a test" → redirects from manual to automated
- "Why are we doing X?" → questions non-standard approaches
- "That's not how [algorithm] works" → catches reinvented solutions

## Case Categories

1. **Unnecessary Separation** (#001) - Adding new operations instead of extending existing ones
2. **Fix Spirals** (#002) - Cascading "let me fix" attempts that indicate wrong approach
3. **Over-Engineering** (#003) - Restructuring internals for interface-level changes
4. **Abstraction Leakage** (#004) - Platform/implementation details surfacing in public API
5. **Silent Pivoting** (#005) - Abandoning user's requested approach without asking
6. **False Verification** (#006) - Using passing tests as proof of fix without verifying tests test the bug
7. **Symptom vs Cause** (#007) - Fixing visible symptom instead of underlying problem
8. **Cascading Breakage** (#008) - Changing foundational assumptions breaks unrelated systems
9. **Manual Over Automated** (#009) - Repeated manual verification instead of writing tests
10. **Reinventing Standard Solutions** (#010) - Novel approach creates problems standard algorithm avoids

## Session Archive

Full conversation transcripts are stored in `~/.claude/projects/-home-midori--dev-sim2d/*.jsonl`
