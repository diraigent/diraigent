# Decisions as Code

Store Architecture Decision Records (ADRs) as YAML files in this directory. They are automatically synced to the Diraigent API when a task pipeline starts.

## YAML Schema

Each `.yaml` file in `.diraigent/decisions/` represents one decision:

```yaml
title: "Use PostgreSQL for persistence"
status: accepted  # proposed | accepted | rejected | superseded | deprecated
context: "We need a relational database for our data"
decision: "Use PostgreSQL 15+"
rationale: "Strong JSON support, mature ecosystem"
alternatives:
  - name: "MySQL"
    pros: "Widely known"
    cons: "Weaker JSON support"
  - name: "SQLite"
    pros: "Zero config"
    cons: "Not suitable for concurrent access"
consequences: "Need to manage PG backups and connection pooling"
tags: ["architecture", "database"]
```

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Human-readable decision title (must not be empty) |
| `context` | string | Problem or situation that led to the decision (must not be empty) |

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `status` | string | `proposed` | One of: `proposed`, `accepted`, `rejected`, `superseded`, `deprecated` |
| `decision` | string | - | The decision that was made |
| `rationale` | string | - | Why this decision was chosen |
| `alternatives` | array | `[]` | List of alternatives considered, each with `name`, `pros`, `cons` |
| `consequences` | string | - | Expected consequences of the decision |
| `tags` | array | `[]` | Tags for categorization |

## File References

For long-form content, you can reference separate markdown files instead of inlining text. File paths are resolved relative to the YAML file's directory.

```yaml
title: "Adopt microservices architecture"
status: accepted
context_file: context/microservices-context.md
decision_file: context/microservices-decision.md
rationale_file: context/microservices-rationale.md
consequences_file: context/microservices-consequences.md
tags: ["architecture"]
```

### Supported File References

| Field | Overrides |
|-------|-----------|
| `context_file` | `context` |
| `decision_file` | `decision` |
| `rationale_file` | `rationale` |
| `consequences_file` | `consequences` |

When both an inline field and its `*_file` counterpart are present, the file content takes precedence. If the referenced file cannot be read, a warning is logged and the inline content is used as a fallback.

## Sync Behavior

When a task pipeline starts, Orchestra discovers YAML files in this directory and syncs them to the project's decisions in the API:

- **New files**: A new decision is created in the API
- **Changed files**: The existing API decision is updated
- **Unchanged files**: Skipped (no API call)
- **Orphaned API decisions**: Warned about but not auto-deleted

Repo-sourced decisions are identified by the tags `source:repo` and `repo_file:<filename>`. These tags are managed automatically and should not be manually edited in the API.

## File Naming

Use descriptive kebab-case filenames:

```
.diraigent/decisions/
  001-use-postgresql.yaml
  002-adopt-rust-backend.yaml
  003-catppuccin-theme.yaml
```

The filename stem (e.g. `001-use-postgresql`) is used internally for matching but does not affect the decision title.
