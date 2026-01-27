# Astra Branching Strategy

## Overview

This document defines how Claude agents coordinate work using git branches.

## Branch Naming Convention

All branches must follow this pattern:
```
claude/{feature-name}-{session-id}
```

Examples:
- `claude/setup-astra-foundation-Ig1nr` (foundation work)
- `claude/typechecker-inference-Abc12` (type checker feature)
- `claude/effects-integration-Xyz99` (effects system work)

## Current Branches

| Branch | Purpose | Status |
|--------|---------|--------|
| `claude/setup-astra-foundation-Ig1nr` | Main development branch | Active |

## Agent Workflow

### Starting New Work

1. **Create a new branch** from the current main development branch:
   ```bash
   git checkout claude/setup-astra-foundation-Ig1nr
   git pull origin claude/setup-astra-foundation-Ig1nr
   git checkout -b claude/{your-feature}-{session-id}
   ```

2. **Update IMPLEMENTATION_PLAN.md** to mark your task as "In Progress"

3. **Make changes** with frequent commits

4. **Push your branch**:
   ```bash
   git push -u origin claude/{your-feature}-{session-id}
   ```

### Merging Work

Since direct pushes to a central main branch may be restricted, use this workflow:

1. **Complete your feature** with all tests passing

2. **Update your branch** with latest changes:
   ```bash
   git fetch origin claude/setup-astra-foundation-Ig1nr
   git merge origin/claude/setup-astra-foundation-Ig1nr
   ```

3. **Resolve any conflicts** and ensure tests still pass

4. **Create a pull request** or notify the coordinating agent

5. **Coordinating agent merges** by:
   ```bash
   git checkout claude/setup-astra-foundation-Ig1nr
   git merge claude/{feature-branch}
   git push origin claude/setup-astra-foundation-Ig1nr
   ```

## Agent Assignments

Each agent type works on specific areas:

| Agent | Focus Area | Files |
|-------|-----------|-------|
| typechecker-engineer | Type inference | `src/typechecker/` |
| effects-engineer | Effect checking | `src/effects/` |
| parser-engineer | Parser extensions | `src/parser/` |
| stdlib-engineer | Standard library | `stdlib/` |
| runtime-engineer | Interpreter | `src/interpreter/` |
| docs-engineer | Documentation | `docs/`, `examples/` |
| testing-engineer | Test framework | `src/testing/`, `tests/` |

## Conflict Prevention

1. **Check IMPLEMENTATION_PLAN.md** before starting work
2. **Claim tasks** by updating the plan with your session ID
3. **Avoid overlapping files** when possible
4. **Communicate via commit messages** for interface changes

## Current Main Branch

The primary development branch is:
```
claude/setup-astra-foundation-Ig1nr
```

All feature branches should be created from and merged back to this branch.
