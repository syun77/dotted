# Repository Agent Instructions

## Durable project knowledge

- During every task, identify information that would help a future agent work correctly or efficiently in this repository (for example: architecture, conventions, required commands, environment assumptions, recurring pitfalls, and non-obvious decisions).
- As soon as such information is verified, add or update a concise entry in this `AGENTS.md`. Do not wait until the end of the session.
- Record only durable, repository-specific facts. Do not add guesses, temporary debugging observations, task-specific status, credentials, secrets, personal data, or information already obvious from the code.
- Keep entries current: revise or remove stale guidance when the repository changes. Prefer short, actionable instructions and include the relevant file or command where useful.

## Resumable progress

- Maintain the current task state in `.agents/progress.md` throughout the work so another session can resume without relying on chat history.
- Create the file when starting substantive work and update it after each meaningful milestone, before risky or long-running operations, and whenever the plan, decisions, blockers, or next action changes.
- Each update must include: the task objective, current status, completed work, key decisions and reasons, files changed, verification performed and results, blockers or open questions, and the exact next steps.
- Write progress updates atomically when practical. Never store credentials, secrets, tokens, personal data, or large command outputs in the progress file.
- At the start of every task, read this `AGENTS.md` and `.agents/progress.md` if it exists. Validate the recorded state against the working tree before continuing; do not blindly repeat completed work.
- When a task is complete, mark it complete with a concise final summary and verification result. Replace obsolete task details when beginning a new task so `.agents/progress.md` reflects the active task rather than becoming an append-only log.

## Development policy

- This project is under active development and does not require backward compatibility. Prefer the best current design, including breaking changes, over compatibility layers, deprecated aliases, transitional shims, or preservation of obsolete behavior.
- When changing a design or interface, update every affected implementation, caller, test, fixture, configuration file, schema, type, documentation page, and example to the new form in the same task. Do not leave mixed old and new conventions behind.
- Remove superseded code and stale documentation instead of retaining them for compatibility. Do not add migration paths unless the user explicitly requests one.
- Keep comments and documentation accurate for the final current implementation. Remove change-history comments, temporary notes, and explanations of what the code used to do; retain only comments that clarify the current behavior or non-obvious rationale.
- Implement the solution you judge to be the strongest overall design within the requested scope, and carry necessary follow-on changes through the repository rather than applying a narrowly local patch.
- Every implementation task must include a final refactoring phase after functional work is complete. In that phase, review the entire changed area for simpler structure, clearer naming, duplication, dead code, obsolete abstractions, consistent APIs, and current comments; apply the warranted improvements, then run the relevant verification again.

## Documentation

- Japanese learning/reference material belongs in `資料/` as Markdown. Use descriptive Japanese filenames and link primary sources inline when documenting software behavior.
