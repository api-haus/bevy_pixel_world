# Case 001: Unnecessary Command Separation

**Date:** 2026-01-29
**Component:** Persistence / I/O Worker

## Context

Implementing async I/O for chunk persistence. Tests failing because bodies weren't loading after world reload.

## What Claude Did

Added a separate `LoadBodiesForChunk` I/O command to the worker, making chunk loading a two-step process:

1. Send `LoadChunk` command, receive chunk data
2. Send `LoadBodiesForChunk` command, receive body data

This required:
- New enum variant in `IoCommand`
- New enum variant in `IoResult`
- New `BodyLoadData` struct
- Handler implementation in native worker
- Handler implementation in WASM worker
- New state tracking for pending body loads
- Modified `poll_io_results` to handle the new result type

## What Should Have Happened

Extend the existing `LoadChunk` command to also return associated body data in the response. One command, one response, all data included.

## The Pivot

*"why is loading chunk and bodies for chunk separate commands? what practical use is there for it?"*

Claude immediately stepped back and reverted to extending the existing command.

## Transcript Excerpt

```
● I see there's no LoadBodiesForChunk command. I need to add it. This is getting
  complex. Let me add the command and result types, then implement the handler.

● Update(...io_worker/mod.rs)
  ⎿  Added 2 lines
      LoadBodiesForChunk { chunk_pos: IVec2 },

[... more additions ...]

● Now I need to add the handle_load_bodies_for_chunk function...
  ⎿  Interrupted

❯ why is loading chunk and bodies for chunk separate commands? what practical
  use is there for it? what are you doing? I'm just asking

● You're right to question this. Let me step back.

  [Claude explains the simpler solution and reverts]
```

## Notes

The phrase "This is getting complex" appeared right before adding the unnecessary command—a possible signal that the approach was wrong.
