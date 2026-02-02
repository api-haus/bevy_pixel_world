---
name: refine-docs
description: Interactive document refinement with Q&A - go file by file, ask questions, apply changes
---

# Interactive Document Refinement

Refine documentation through an interactive Q&A session. Go through files one by one, asking questions about decisions, then applying changes based on answers.

## Scope

Target directory: `$ARGUMENTS` (defaults to `docs/`)

## Process

For each document file in the target:

1. **Read the file** - Display current content summary
2. **Identify decision points** - Find areas that need clarification or may need changes
3. **Ask questions** - Use AskUserQuestion or direct questions for:
   - Architectural decisions that need confirmation
   - Sections that may be outdated or need removal
   - Unclear responsibilities or boundaries
   - Missing information
4. **Wait for answers** - Let user provide direction
5. **Apply changes** - Edit the file based on user's answers
6. **Confirm** - Show what changed, ask if complete for this file
7. **Next file** - Move to next document

## Question Guidelines

Ask about:
- "Should this section remain? It describes [X]"
- "This references [concept]. Is this still relevant?"
- "There's ambiguity about [responsibility]. Which component owns this?"
- "This assumes [approach]. Does the new direction change this?"

Do NOT:
- Make assumptions about what to remove
- Skip files without asking
- Batch too many questions (max 3-4 per interaction)

## Output Format

For each file:
```
## [filename]

Current: [brief summary of content]

Questions:
1. [question about decision point]
2. [question about decision point]

[Wait for answers]

Changes applied:
- [change 1]
- [change 2]

Ready for next file? (or revisit)
```

## Constraints

- One file at a time - do not jump ahead
- Always show what will change before applying
- User drives direction, assistant asks and applies
