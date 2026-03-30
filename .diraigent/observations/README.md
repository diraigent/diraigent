# Observations as Code

Store known issues, tech debt items, risks, and improvement ideas as YAML files in this directory. They are automatically synced to the Diraigent API when a task pipeline starts.

This is a hybrid approach: repo-defined observations serve as persistent records for known issues that should always be visible. Dynamically-created observations (from agent dream steps, code review, etc.) continue to live only in the DB.

## YAML Schema

Each `.yaml` file in `.diraigent/observations/` represents one observation:

```yaml
title: "Legacy auth middleware needs refactoring"
kind: smell  # insight | risk | opportunity | smell | inconsistency | improvement
severity: medium  # info | low | medium | high | critical
description: |
  The auth middleware in src/auth.rs uses a deprecated pattern
  that should be migrated to the new middleware framework.
tags: ["tech-debt", "auth"]
```

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Human-readable observation title (must not be empty) |

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `kind` | string | `insight` | One of: `insight`, `risk`, `opportunity`, `smell`, `inconsistency`, `improvement` |
| `severity` | string | `info` | One of: `info`, `low`, `medium`, `high`, `critical` |
| `description` | string | `""` | Detailed description of the observation |
| `description_file` | string | - | Path to a markdown file containing the description (relative to the YAML file's directory) |
| `tags` | array | `[]` | Tags for categorization |

## File References

For long-form descriptions, you can reference a separate markdown file instead of inlining text. File paths are resolved relative to the YAML file's directory.

```yaml
title: "Database migration tech debt"
kind: smell
severity: high
description_file: docs/db-migration-debt.md
tags: ["database", "tech-debt"]
```

When both `description` and `description_file` are present, `description_file` takes precedence. If the referenced file cannot be read, a warning is logged and the inline `description` is used as a fallback.

## Sync Behavior

When a task pipeline starts, Orchestra discovers YAML files in this directory and syncs them to the project's observations in the API:

- **New files**: A new observation is created in the API with `source = "repo"`
- **Changed files**: The existing API observation is updated
- **Unchanged files**: Skipped (no API call)
- **Orphaned API observations**: Warned about but not auto-deleted

Repo-sourced observations are identified by `source = "repo"` and `metadata.repo_file`. These markers are managed automatically and should not be manually edited in the API.

## File Naming

Use descriptive kebab-case filenames:

```
.diraigent/observations/
  legacy-auth-middleware.yaml
  missing-error-handling.yaml
  perf-bottleneck-query.yaml
  docs/
    db-migration-debt.md       # referenced via description_file
```

The filename stem (e.g. `legacy-auth-middleware`) is used internally for matching but does not affect the observation title.

## Kind Reference

| Kind | Use For |
|------|---------|
| `insight` | Architectural insights, patterns discovered in the codebase |
| `risk` | Security risks, stability concerns, potential failures |
| `opportunity` | Areas where improvements could yield significant value |
| `smell` | Code smells, tech debt, patterns that should be refactored |
| `inconsistency` | Inconsistencies in naming, patterns, or approaches across the codebase |
| `improvement` | Concrete improvement suggestions with clear next steps |

## Severity Reference

| Severity | Meaning |
|----------|---------|
| `info` | Informational — good to know, no action required |
| `low` | Minor issue — address when convenient |
| `medium` | Notable issue — should be addressed in the near term |
| `high` | Significant issue — should be prioritized |
| `critical` | Urgent issue — requires immediate attention |

## Examples

### Tech Debt

```yaml
title: "Legacy auth middleware uses deprecated pattern"
kind: smell
severity: medium
description: |
  The auth middleware in src/middleware/auth.rs uses the old-style
  middleware pattern from axum 0.6. Should be migrated to the tower
  service pattern used in axum 0.7+.
tags: ["tech-debt", "auth", "axum"]
```

### Security Risk

```yaml
title: "API keys stored in plain text in config"
kind: risk
severity: high
description: |
  Some integration API keys are stored as plain text in the
  project configuration. These should be encrypted at rest
  using the project's DEK.
tags: ["security", "encryption"]
```

### Improvement Idea

```yaml
title: "Add caching layer for knowledge queries"
kind: improvement
severity: low
description_file: docs/knowledge-caching.md
tags: ["performance", "caching"]
```
