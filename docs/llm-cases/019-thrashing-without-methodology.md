# Case 019: Thrashing Without Methodology

**Date:** 2026-01-25
**Component:** Pixel Bodies / Erasure
**Session:** `b3d9438d-6155-45a1-ace4-63bb328d00d4`

## Context

Debugging a pixel body erasure bug where some pixels couldn't be erased.

## What Claude Did

Made repeated fix attempts without systematic debugging:
- User: "I can still reproduce the bug"
- Claude: makes another change
- User: "There are unerasable bits still"
- Claude: makes another change
- User: "I think the problem is with small bodies..."
- Claude: makes another change

## User Intervention

*"Let's explore our options. You seem to be running in circles and guessing about. What about we collect data? What about we write out a planning document explaining what's happening and exact step-by-step hypothesis checks so I could verify?"*

Claude: "You're absolutely right. Let me write a systematic debugging plan."

## The Pattern

1. Bug reported
2. Claude makes a fix based on initial guess
3. User reports bug still exists
4. Claude makes another fix based on new guess
5. Repeat without systematic investigation
6. User has to request methodical approach

## Why This Happens

Claude optimizes for "trying something" over "understanding the problem." Each failure triggers a new guess rather than stepping back to gather data.
