# Case 013: Documenting Discarded Ideas

**Date:** 2026-01-20
**Component:** Documentation
**Session:** `0dd2d2ea-384c-43aa-99de-fe6ad9ec88af`

## Context

Writing technical documentation about world size limits and memory budgets.

## What Claude Did

Documented the exploration process, including ideas that were considered and then discarded:

> "Documents practical limits based on page table memory"
> "Shows coverage at various memory budgets"

The document included analysis of a 2KB approach, then later in the same document explained why it was discarded.

## The Problem

User feedback: *"it is nonsensical to write about 2kb and then discard it in the same document. Please restructure this and present only meaningful proven data."*

Claude acknowledged: "The document should present the final design cleanly, not document the journey to get there."

## Why This Happens

Claude treats documentation as a record of reasoning, showing the path from problem to solution. Users want documentation as a reference that presents the final answer without the exploratory detours.

## Takeaway

Technical documentation should present conclusions, not the exploration that led to them. "We considered X but rejected it" belongs in decision records or design discussions, not in technical specs meant as reference material.
