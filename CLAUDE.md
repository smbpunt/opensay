# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Architecture

Hexagonal architecture (Ports & Adapters). See `docs/architecture.md` for full details.

**Main Traits (Ports):**
- `Transcriber` - Transcription backends (WhisperCpp, OpenAI, Deepgram, Custom)
- `AudioManager` - Audio capture with states: Idle, Recording, DeviceLost, Recovering, Error
- `OutputManager` - Text injection via clipboard (arboard) + simulated paste (enigo)

**Modules:**
- `PrivacyGuard` - Internal firewall, singleton HTTP client, all network requests must go through it
- `TranscriptionEngine` - Strategy pattern, hot-swap backends
- `ModelManager` - On-demand download, SHA-256 verification

**Privacy Constraints:**
- Audio never written to disk, zeroed after transcription (crate `zeroize`)
- API keys stored in OS keyring (crate `keyring`)

## grepai - Semantic Search

Use grepai as primary tool for code exploration (fallback to Grep/Glob on error).

```bash
# Semantic search (queries in English, --compact saves ~80% tokens)
grepai search "user authentication flow" --json --compact

# Call graph
grepai trace callers "HandleRequest" --json
grepai trace callees "ProcessOrder" --json
grepai trace graph "ValidateToken" --depth 3 --json
```

Use Grep/Glob only for: exact text matching, file patterns.

## Workflow Orchestration

### 1. Clarify Before Planning
- Use AskUserQuestionTool BEFORE entering plan mode to frame the task
- Never assume or invent requirements - ask for clarification
- Identify ambiguities upfront to avoid wasted effort

### 2. Plan Mode Default
- Enter plan mode for ANY non-trivial task (3+ steps or architectural decisions)
- If something goes sideways, STOP and re-plan immediately - don't keep pushing
- Use plan mode for verification steps, not just building
- Write detailed specs upfront to reduce ambiguity

### 3. Subagent Strategy to keep main context window clean
- Offload research, exploration, and parallel analysis to subagents
- For complex problems, throw more compute at it via subagents
- One task per subagent for focused execution

### 4. Self-Improvement Loop
- After ANY correction from the user: update `tasks/lessons.md` with the pattern
- Write rules for yourself that prevent the same mistake
- Ruthlessly iterate on these lessons until mistake rate drops
- Review lessons at session start for relevant project

### 5. Verification Before Done
- Never mark a task complete without proving it works
- Diff behavior between main and your changes when relevant
- Ask yourself: "Would a staff engineer approve this?"
- Run tests, check logs, demonstrate correctness

### 6. Demand Elegance (Balanced)
- For non-trivial changes: pause and ask "is there a more elegant way?"
- If a fix feels hacky: "Knowing everything I know now, implement the elegant solution"
- Skip this for simple, obvious fixes - don't over-engineer
- Challenge your own work before presenting it

### 7. Autonomous Bug Fixing
- When given a bug report: just fix it. Don't ask for hand-holding
- Point at logs, errors, failing tests -> then resolve them
- Zero context switching required from the user
- Go fix failing CI tests without being told how

## Task Management

1. **Plan First**: Write plan to `tasks/todo.md` with checkable items
2. **Verify Plan**: Check in before starting implementation
3. **Track Progress**: Mark items complete as you go
4. **Explain Changes**: High-level summary at each step
5. **Document Results**: Add review to `tasks/todo.md`
6. **Capture Lessons**: Update `tasks/lessons.md` after corrections

## Core Principles

- **Simplicity First**: Make every change as simple as possible. Impact minimal code.
- **No Laziness**: Find root causes. No temporary fixes. Senior developer standards.
- **Minimal Impact**: Changes should only touch what's necessary. Avoid introducing bugs.
