# CLAUDE.md

@AGENTS.md

## Rust Code Quality

- Always run `cargo fmt` and `cargo clippy` after writing or modifying Rust code

## Claude Code Conventions

- Small/confident changes: commit directly to main (skip PRs, local build is the gate)
- Bigger/riskier features: branch + PR as before
- User pushes manually — don't attempt `git push`
- Use `GITHUB_TOKEN= gh ...` prefix for all gh commands (stale env var overrides keyring auth)
- Forgejo is primary remote (HTTPS push). GitHub is a mirror (SSH push).
- Close issues via Forgejo API or `gh issue close N --comment "..."` (GitHub)
