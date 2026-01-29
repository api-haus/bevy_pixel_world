# Case 011: Hallucinated API

**Date:** 2026-01-24
**Component:** Physics
**Session:** `3d07d8ac-ab8a-4d7d-b042-0620e1bb0cc8`

## Context

Implementing pixel body spawning with physics. The code supports both avian2d and rapier2d physics backends.

## What Claude Did

Proposed using `RigidBody::Static`:

> "Spawn bodies with physics disabled - Use `RigidBody::Static` initially, switch to `Dynamic` once surrounding collision tiles are cached"

## The Problem

`RigidBody::Static` doesn't exist in rapier2d. The correct variant is `RigidBody::Fixed`.

Claude conflated APIs between the two physics backends:
- avian2d: `RigidBody::Static`, `RigidBody::Dynamic`
- rapier2d: `RigidBody::Fixed`, `RigidBody::Dynamic`

## Takeaway

Claude confidently uses APIs that don't exist, especially when multiple similar libraries are involved. The code may compile for one backend but fail for another. Feature-gated code paths need verification against each backend's actual API.
