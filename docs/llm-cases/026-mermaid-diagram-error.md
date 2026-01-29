# Case 026: Mermaid Diagram Error

**Date:** 2026-01-24
**Session:** `42de6284-8be7-4ef6-85af-660c0b492b6f`
**Component:** Documentation

## Context

Creating scheduling documentation with Mermaid diagrams showing simulation phases.

## What Claude Did

Created a Gantt chart with separate `section` markers for each phase (A, B, C, D).

## User Response

*"Simulation Tick (4 Phases) - the phases appear to run in parallel to each other which is wrong - phases go one after another"*

## The Problem

Mermaid renders sections as parallel swim lanes. Claude's diagram visually showed concurrent phases when they actually run sequentially.

The documentation was technically "written" but communicated the wrong information.

## Why This Happens

Claude knows Mermaid syntax but may not verify how diagrams render. The code was syntactically valid but semantically wrong.

## Takeaway

Documentation diagrams should be reviewed for visual accuracy, not just syntax correctness. What renders isn't always what was intended.
