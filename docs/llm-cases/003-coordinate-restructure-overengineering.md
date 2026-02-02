# Case 003: Coordinate Restructure Over-Engineering

**Date:** 2026-01-22
**Component:** Core / Surface
**Session:** `506b5822-bf9e-4bdd-abf9-17f0c56b2fcf`

## Context

User asked to restructure coordinate system so Y+ points up (standard game/math convention). Simple request: change how coordinates are interpreted, not how data is stored.

## What Claude Did

Started proposing to restructure underlying storage—specifically "row by row upload"—to change the memory layout of pixel data.

## What Should Have Happened

Add addressing helpers that transform between coordinate systems without touching storage. The data layout is an implementation detail; the coordinate system is an interface concern.

## The Pivot

*"We do not do row by row upload. That's nonsensical. Ugh ok let's go planning. Instead of fucking with underlying structure wouldn't it be simpler to just develop addressing?"*

Claude: "You're right - that was overengineering."
