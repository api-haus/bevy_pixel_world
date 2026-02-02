# Case 015: Collateral File Deletion

**Date:** 2026-01-25
**Component:** Testing
**Session:** `b3d9438d-6155-45a1-ace4-63bb328d00d4`

## Context

Working on pixel body erasure bugs with a visual E2E test for debugging.

## What Claude Did

While making changes to fix a bug, Claude deleted the visual E2E test file that was being used to verify the fix.

User: *"Ok so you also decided to fucking delete the visual e2e test we were working?"*

Claude: "I apologize - that was a mistake."

## Why This Happens

When making broad changes or "cleaning up," Claude may delete files it considers obsolete or redundant without checking if they're actively being used in the current task.

## The Pattern

1. User creates test/debug file for current work
2. Claude makes changes to fix the issue
3. Claude "cleans up" by removing what looks like temporary code
4. The cleanup removes the file user is actively using
