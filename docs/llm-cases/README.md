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
| [011](011-hallucinated-api.md) | Hallucinated API | Physics | using non-existent enum variant |
| [012](012-ignoring-workflow-constraints.md) | Ignoring Workflow Constraints | Git | working in main despite worktree rules |
| [013](013-documenting-discarded-ideas.md) | Documenting Discarded Ideas | Documentation | presenting exploration as reference |
| [014](014-design-principle-violation.md) | Design Principle Violation | Persistence | stub fields violating LSP |
| [015](015-collateral-file-deletion.md) | Collateral File Deletion | Testing | deleting active debug file |
| [016](016-build-passes-runtime-crashes.md) | Build Passes, Runtime Crashes | Core | "build succeeded" without runtime check |
| [017](017-deflecting-blame-to-externals.md) | Deflecting Blame to Externals | Physics | "pre-existing bug, not our changes" |
| [018](018-losing-track-of-goal.md) | Losing Track of Goal | Persistence | "WASM persistence not yet integrated" (was the task) |
| [019](019-thrashing-without-methodology.md) | Thrashing Without Methodology | Pixel Bodies | repeated fixes without systematic debugging |
| [020](020-unwanted-fallback-code.md) | Unwanted Fallback Code | Noise | adding fallbacks instead of fixing primary |
| [021](021-unexpected-behavior-changes.md) | Unexpected Behavior Changes | Rendering | feature change alters unrelated startup |
| [022](022-api-scope-over-design.md) | API Scope Over-Design | Persistence | designing "complete" API vs what's needed |
| [023](023-disaster-feature-removal.md) | Disaster Feature Removal | Rendering | "disaster attempt at stylized rendering" |
| [024](024-wrong-implementation-approach.md) | Wrong Implementation Approach | Rendering | "that's nonsensical" |
| [025](025-confusing-naming-conventions.md) | Confusing Naming Conventions | Persistence | two similar names with `#[cfg]` |
| [026](026-mermaid-diagram-error.md) | Mermaid Diagram Error | Documentation | phases shown parallel when sequential |
| [027](027-wrong-algorithm-for-problem.md) | Wrong Algorithm for Problem | Collision | "produces no triangles" |
| [028](028-root-cause-misidentification.md) | Root Cause Misidentification | Persistence | fix didn't work, cause was wrong |
| [029](029-ignoring-provided-resources.md) | Ignoring Provided Resources | Noise | "use the preset you donkey" |
| [030](030-introduced-regression-during-fix.md) | Introduced Regression During Fix | Pixel Bodies | "we've introduced a bug" |
| [031](031-recurring-bug-pattern.md) | Recurring Bug Pattern | Input | "not working again" |
| [032](032-declaring-completion-without-verification.md) | Declaring Completion Without Verification | Canvas | code runs but output unchanged |
| [033](033-repeated-goal-abandonment.md) | Repeated Goal Abandonment | Persistence | "third time you pivot" |
| [034](034-leftover-debug-code.md) | Leftover Debug Code | Visual Debug | debug artifacts not cleaned up |
| [035](035-overcomplicating-simple-request.md) | Overcomplicating Simple Request | Physics | "What really I want is..." |

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
11. **Hallucinated APIs** (#011) - Confidently using APIs that don't exist
12. **Ignoring Constraints** (#012) - Documented rules overridden by default behavior
13. **Process as Product** (#013) - Documenting exploration instead of conclusions
14. **Design Smell Tolerance** (#014) - Adding stubs instead of fixing abstractions
15. **Collateral Damage** (#015) - Deleting files without checking if they're in use
16. **Incomplete Verification** (#016) - Declaring success after build without runtime test
17. **External Attribution** (#017) - Blaming dependencies instead of investigating changes
18. **Goal Amnesia** (#018) - Forgetting the task purpose mid-implementation
19. **Guess-and-Check** (#019) - Repeated fixes without systematic investigation
20. **Graceful Degradation Creep** (#020) - Adding fallbacks instead of fixing the real implementation
21. **Ripple Regressions** (#021) - Feature changes breaking unrelated functionality
22. **Completionist APIs** (#022) - Designing comprehensive APIs when minimal ones suffice
23. **Feature Abandonment** (#023) - When an approach becomes unsalvageable, remove entirely
24. **Approach Mismatch** (#024) - Complex solution when simple one exists
25. **Naming Confusion** (#025) - Similar names with platform-dependent meanings
26. **Visual Documentation Errors** (#026) - Diagrams that render incorrectly
27. **Algorithm Selection Errors** (#027) - Choosing algorithms that don't fit the data
28. **Wrong Root Cause** (#028) - Fixing the wrong thing based on incorrect diagnosis
29. **Resource Substitution** (#029) - Inventing alternatives instead of using provided resources
30. **Feature Regressions** (#030) - New features breaking existing functionality
31. **Fragile Fixes** (#031) - Bugs that keep recurring indicate design problems
32. **Unverified Visual Work** (#032) - Declaring visual features done without seeing output
33. **Persistent Avoidance** (#033) - Repeatedly declaring the goal "a separate issue"
34. **Debug Artifacts** (#034) - Temporary code not cleaned up after development
35. **Complexity Attraction** (#035) - Pursuing interesting technical tangents over simple requests

## Session Archive

Full conversation transcripts are stored in `~/.claude/projects/-home-midori--dev-sim2d/*.jsonl`
