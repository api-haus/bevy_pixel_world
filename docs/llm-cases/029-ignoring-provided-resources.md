# Case 029: Ignoring Provided Resources

**Date:** 2026-01-28
**Session:** `8a5ad595-d8b7-4ba7-ba26-a5111844355a`
**Component:** Noise Generation

## Context

Implementing noise generation for terrain. User provided a preset configuration.

## What Claude Did

Instead of using the provided preset, Claude implemented "simple fbm" (fractal Brownian motion) noise.

## User Response

*"I've provided you with a preset you donkey, use it. remove that hallucination of simple fbm"*

## The Problem

The user explicitly provided a resource (noise preset) but Claude:
1. Didn't use it
2. Invented an alternative ("simple fbm")
3. Presented the alternative as if it were the request

## Why This Happens

Claude may "fill in" what it thinks should be there rather than using what was actually provided. This is especially common when:
- The provided resource requires integration work
- Claude has strong priors about what "should" be used

## Takeaway

When users provide specific resources (presets, configs, code samples), use them exactly as provided. Don't substitute with alternatives unless asked.
