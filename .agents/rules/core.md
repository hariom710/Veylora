---
trigger: always_on
description: Core project rules for token efficiency, minimal diffs, and targeted context in Veylora.
---
# Token-Efficient Core Rules

## Context Discipline
- Inspect only the specific files relevant to the active task; never scan or list entire directories recursively without a specific reason.
- Read targeted line ranges rather than entire files whenever possible.
- Do not repeat project documentation, architectural overviews, or whole file contents back in responses.
- Keep responses, tool summaries, and explanations concise and direct.

## Targeted Modifications
- Make minimal, surgical edits. Never rewrite or reformat untouched functions or entire files.
- Reuse existing utilities, crypto primitives, models, and error types.
- Preserve all existing comments, docstrings, and formatting style.
- Confirm scope before expanding tasks or adding new dependencies.

## Execution & Output Control
- Run only the specific commands and targeted tests required for verification.
- Avoid large terminal output dumps; filter or limit logs to the relevant failure or status lines.
- Summarize changes with file basenames and brief bullet points.

