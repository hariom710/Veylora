---
trigger: model_decision
description: Testing guidelines, targeted cargo test commands, and test noise reduction.
---
# Testing & Verification Rules

## Targeted Test Execution
- Run only the specific test case or test file related to your changes. Never run the full test suite indiscriminately.
- Examples of targeted testing:
  - Specific test function: `cargo test <test_function_name> -- --nocapture`
  - Specific integration test file: `cargo test --test <integration_file_name>`
  - Single package check: `cargo test -p veylora --lib <module_name>`
- Pipe or restrict terminal output so large passing logs do not flood the context window.

## Test Artifact & Environment Hygiene
- Always use isolated, temporary test databases (e.g., `test-vault.db`, `.tmp-*`).
- Ensure all test artifacts and temporary files are cleaned up in a `teardown` or `Drop` implementation.
- Never commit test databases, logs, or diagnostic dumps (`build_output.txt`, `*errors*.txt`) to version control.

