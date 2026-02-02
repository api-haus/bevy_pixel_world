# Case 035: Overcomplicating Simple Request

**Date:** 2026-01-25
**Session:** `04e3ed28-4c24-4d6e-b891-c5e5765e6eae`
**Component:** Physics / Pixel Displacement

## Context

Discussing how to handle pixel displacement when physics bodies move.

## What The User Said

Mentioned "presimulation" as a hypothetical approach, then asked to document before implementing.

## What Claude Understood

Claude latched onto the presimulation idea:

> "I understand. You're proposing a different approach: instead of scanning for surface voids in the wake, we pre-simulate where the displaced pixels would naturally fall/flow using the same CA logic."

## What The User Actually Wanted

*"What really I want to address first of all is not presimulation, but instead of completely erasing the pixels where body will reappear... we swap the pixels back to where the body was. This is simple."*

The user wanted simple swapping, not complex presimulation.

## Claude's Response

> "That's a much simpler and elegant approach."

Claude acknowledged the simpler approach was better once corrected.

## Why This Happens

Claude gravitates toward technically interesting solutions. When a user mentions a complex possibility hypothetically, Claude may pursue it even when the user's actual request is simpler.
