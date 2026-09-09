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

## Dot tool development

- The product is an Aseprite-like raster pixel-art editor focused on deliberate practice. It is not a vector-to-pixel converter and must not assume a 1-bit target. Before designing or implementing it, read `.agents/dot-tool-development.md` and keep that guide synchronized with product decisions.
- Treat the layer/frame pixel data and palette as the editable source of truth. Every drawing operation must produce deterministic integer-grid pixel changes without antialiasing or subpixel state.
- Make the core practice loop first-class: choose a size and palette constraint, observe a reference, build a silhouette, add limited values/colors, inspect at 1x, export, and record a short reflection. Features should reduce friction in this loop rather than merely imitate a general-purpose painting app.
- Optimize feedback in this order: readability at 1x, silhouette, value contrast, deliberate pixel clusters and contour rhythm, animation readability, color, then detail. Surface accidental isolated pixels, jagged contours, excess colors, and noisy dithering without silently rewriting the artwork.
- Support enlarged editing and a simultaneous 1x preview. Verify artwork on the intended background and animations at actual playback speed; zoomed canvas appearance alone is not acceptance.
- Keep document content (canvas, layers, frames, palette, tags, and persisted practice metadata) separate from transient UI state (zoom, pan, active selection, dialogs). Define Undo/Redo transaction boundaries for complete user gestures.
- Preserve exact pixels on import/export. PNG is the baseline static interchange format; animation and sprite-sheet exports must preserve frame order, timing, transparency, tags, and integer nearest-neighbor scaling where enlargement is requested.
- Treat “copy → hide the reference → redraw from memory on a blank canvas → make a variation → compare → reflect” as the product's defining workflow. Save each stage as a separate artifact; never overwrite the earlier stage or initialize memory drawing from copied pixels.
- Treat hand-drawn 16×16 → 8×8 → 32×32 reinterpretation as a primary post-MVP exercise. Do not present automatic downscaling or pixel-difference scores as the answer; diagnostics identify review candidates and must not silently edit or rank artwork.
- Use `資料/ドット絵スキル向上ツール仕様.md` as the product specification for scope, flows, data model, MVP acceptance criteria, and phased delivery.
- Use statistics and achievements as curriculum guidance, not skill scores: show observable practice behavior and change, explain why the next exercise is suggested, and reward trying sound methods such as memory drawing, limited palettes, manual size reinterpretation, cleanup comparison, and key-pose-first animation.
- For color practice, prioritize relationships among palette roles (value steps, saturation changes, and hue direction) over raw RGB matching. For silhouette practice, include brief actual-size recognition, pose clarity, and distinction between similar subjects while keeping judgments decomposed rather than producing an overall art score.
- Use `資料/Dotted Training Studioプログラム設計.md` for the Phase 1 architecture. Keep dependencies directed from the desktop UI into UI-independent `training`, `pixel_core`, and `project_io` crates; never let domain crates depend on egui, OS APIs, SQLite, or dialogs.
- The MVP persists `.dotted` as a ZIP container with JSON metadata and per-cel RGBA PNGs, embeds normalized references, and uses a rebuildable SQLite history index. Normalize fully transparent pixels to RGBA `(0,0,0,0)` and preserve exact non-transparent RGBA values.
- All pixel mutation must pass through edit commands. One pointer gesture is one Undo transaction, diagnostics are read-only projections, and the Copy-to-Memory transition must construct an independent transparent grid without accepting copied pixels as input.

- Phase 1 uses one session and one embedded reference per Project, with at most Copy and Memory artworks. Each artwork owns its palette and a single-layer, single-frame RasterDocument; tags and finalized-artwork branching are deferred. Keep spec AC-01–AC-12 synchronized with design section 10.5.
- Keep ProjectRevision (monotonic persisted-content changes), render_epoch (including in-progress edits and rollback), and save-job OpenGeneration separate. Recovery guarantees only the last successfully written snapshot; recovery saves do not clear explicit-save dirty state.
- The Phase 1 Cargo workspace members are `apps/desktop`, `crates/pixel_core`, `crates/training`, and `crates/project_io`; preserve the dependency direction documented in `資料/Dotted Training Studioプログラム設計.md`.
- Run `cargo fmt --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` from the repository root; CI runs the same checks on macOS and Windows.
