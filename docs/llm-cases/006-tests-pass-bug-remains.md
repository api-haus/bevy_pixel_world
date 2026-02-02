# Case 006: Tests Pass, Bug Remains

**Date:** 2026-01-27
**Component:** Pixel Bodies / Erasure
**Session:** `44708028-a1c6-46dd-afe3-fedb5b6a5ff1`

## Context

Fixing a bug where erased pixel bodies leave behind "ghost" pixels. Claude implemented a fix and ran tests.

## What Claude Did

Repeatedly claimed success based on passing tests:

1. "The `erased_bodies_fully_removed` test passes"
2. "All three body stability tests pass"
3. "All tests pass"
4. "All E2E tests pass. The erasure fix is complete."
5. Listed passing tests as verification

Then summarized:
> **All E2E tests pass**: `erased_bodies_fully_removed`, `bodies_do_not_spontaneously_disintegrate`...
> **Visual test** shows the fix working (pixel count drops to 0)

## What Actually Happened

The test wasn't testing the actual bug. It was erasing the platform beneath bodies, causing them to fall, rather than testing direct body erasure.

User pointed out: *"What happens in the erasure test is that we simply erase the plaque beneath the objects and they all fall down. You did not fix the erasure bug at all."*

## The Pivot

User had to explicitly explain what the test was actually doing vs what it should be doing.

Claude acknowledged: "You're right. Let me trace through what's actually happening."
