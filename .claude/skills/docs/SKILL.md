# /docs

Edit documentation only. Do not modify source code.

## Scope

- Edit markdown files in `docs/` directory
- Create new documentation files in `docs/`
- Do NOT edit `.rs`, `.toml`, or other source files

## Style Guide

**Headings:**
- `#` for document title
- `##` for major sections
- `###` for subsections
- Max 3 levels deep

**Code Blocks:**
- Always use language tags: `rust`, `wgsl`, `mermaid`
- Fenced blocks for all code (not inline)

**Tables:**
- Pipe-delimited markdown tables
- Use for specs, parameters, comparisons

**Lists:**
- `-` for unordered items
- `1.` for sequential steps
- Indent for nesting

**Diagrams:**
- Mermaid for flowcharts, state machines, sequences
- ASCII only for memory layouts

**Terminology:**
- **Bold** for component/system names (first mention)
- `backticks` for code identifiers
- "→" for transitions in prose

**Files:**
- Kebab-case: `chunk-pooling.md`
- Index files: `README.md`

**Cross-References:**
- Relative paths: `[Title](path/to/file.md)`
- Group in "Related Documentation" section at end
