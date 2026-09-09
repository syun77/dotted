# Active Task Progress

- Objective: Persist the project's development policy: prefer the best current design and breaking changes, fully update affected code, keep only current comments, and always finish implementation with refactoring.
- Status: Complete.
- Completed work: Added a development-policy section to `AGENTS.md` covering compatibility, repository-wide follow-through, comment hygiene, design quality, and a mandatory final refactoring phase.
- Key decisions: Interpret "breaking changes are recommended" as permission to avoid backward-compatibility code while still requiring scoped, deliberate changes and complete updates to all affected artifacts.
- Files changed: `AGENTS.md`, `.agents/progress.md`.
- Verification: Reviewed the resulting instructions for consistency with the existing durable-knowledge and resumable-progress rules.
- Blockers or open questions: None.
- Next steps: Apply these policies to every subsequent implementation task and replace this checkpoint with the next active task.
