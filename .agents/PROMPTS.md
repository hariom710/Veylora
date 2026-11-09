# Antigravity Token Optimization Prompt Kit

Use these structured prompts to interact with the AI assistant. They keep requests surgical, prevent multi-file context blowup, and avoid unnecessary token consumption.

---

## 1. Targeted Bug Fix / Error Resolution

Use when fixing a specific compiler error or bug:

```text
TASK: Fix [Error / Bug Description] in [Component/Module]
1. Target files: Inspect only [e.g., src-tauri/src/streaming/mod.rs]
2. Root cause: Diagnose without scanning other modules.
3. Surgical fix: Modify only lines directly responsible for the error.
4. Verification: Run targeted check: cargo check -p veylora (or specific test).
5. Output: Report the fix concisely with file basename and lines changed. Do not dump the entire file.
```

---

## 2. Feature Implementation (Incremental)

Use when building a new function, endpoint, or component:

```text
TASK: Implement [Feature Name]
1. Target files: [List 1-2 exact files, e.g., src-tauri/src/vault/session.rs]
2. Constraints:
   - Reuse existing types, error handlers, and crypto utilities.
   - Do not modify unrelated files.
3. Verification: Run only the corresponding unit/integration test.
4. Summary: Provide a brief 3-5 bullet point summary of changes.
```

---

## 3. Targeted Refactoring

Use when cleaning up or optimizing existing code:

```text
TASK: Refactor [Function or Struct Name] in [File Path]
1. Goal: [e.g., Simplify error mapping / reduce allocations]
2. Scope: Restrict edits strictly to [Target Function/Struct].
3. Preservation: Preserve all existing signatures, docstrings, and tests.
4. Do not reformat untouched code.
```

---

## 4. Test Verification & Debugging

Use when running tests without context pollution:

```text
TASK: Verify [Module or Feature]
1. Execute only the specific test: cargo test [test_name] -- --nocapture
2. If failed, read only the failing assertion and stack trace.
3. Do not run the full test suite.
```

---

## 5. Token-Saving Rules of Thumb

| Habit | Recommended Practice | What to Avoid |
| :--- | :--- | :--- |
| **Inspection** | Name the exact file and lines to inspect | "Search my entire repo for..." |
| **Diffs** | Request surgical inline replacements | "Regenerate the entire file with changes" |
| **Explanations** | "Explain in 2 sentences" or "Report diffs only" | "Explain every concept in detail" |
| **Testing** | Target specific test binary or function | Full re-compilation and full test suite run |
| **Continuity** | Keep related iterative steps in the same session | Starting a fresh conversation for every tiny tweak |

