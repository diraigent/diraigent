# Diraigent repository guidance

## Scope and autonomy

Implement the user's requested behavior through validation. Inspect the relevant code and choose an appropriate plan; do not require a plan, file list, or test command before starting an ordinary task. Task file lists are guidance unless the request explicitly makes them exclusive. Honor explicit exclusions and keep unrelated changes out of the diff.

Use the editing tools available to your agent. Prefer focused patches, read enough surrounding code to preserve behavior, and review both committed and uncommitted changes before finishing. Preserve other contributors' work; do not reset or restore files merely because they differ from your expected baseline.

Ask for clarification when required behavior is ambiguous or conflicts with an explicit restriction. Existing task authorization covers routine implementation choices. Report real blockers with evidence; do not invent file-by-file approval requirements.

## Repository and Git

- GitHub (`origin`) is the current remote. Do not introduce a private or retired Git host into public configuration.
- Use a feature branch, validate the change, then merge to `main`. If pull requests are unavailable, a reviewed local merge is acceptable.
- Push, publish or deploy when authorized by the user. An Orchestra task worker follows the worker Git rules in `apps/orchestra/CLAUDE.md`; the runtime owns its merge and push operations.
- Keep credentials, local deployment manifests, private addresses and signing overrides out of commits. Inspect staged changes for accidental exposure.
- Never edit a database migration after it has been committed. Add a new migration for a schema correction; preserve existing migration checksums.

## Validation

Choose checks that cover the changed behavior. Prefer focused regression tests for bugs and boundary cases over tests that repeat the implementation. Do not rerun a failing test unchanged as a fixed ritual; diagnose the failure first. Report pre-existing failures separately and keep unrelated formatting changes out of the patch.

- Rust: run `cargo fmt` and Clippy for affected packages, plus relevant tests.
- Web: use the scripts in `apps/web/package.json`; build for production and use focused browser tests when interaction changes.
- iOS: build the Diraigent scheme and run relevant tests on an available simulator. Keep signing and local configuration private.

Read `apps/api/CLAUDE.md` for API architecture when working there. Read `apps/orchestra/CLAUDE.md` for Orchestra task execution. Those filenames remain for compatibility; their guidance applies to all supported agents.

## Architecture boundaries

The API owns authentication, authorization and shared project/task data. Orchestra runs tools, coordinates direct task execution and manages task worktrees. The web and iOS clients use the authenticated API. Repository instructions and skills guide the selected AI agent. Tasks use backlog, ready, working, human_review, done and cancelled states. Provider-specific flags and capabilities belong in provider adapters, not shared task instructions.
