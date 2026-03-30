# Knowledge as Code

Store architecture docs, conventions, and patterns as YAML files in this directory. They are automatically synced to the Diraigent API when a task pipeline starts.

## YAML Schema

Each `.yaml` file in `.diraigent/knowledge/` represents one knowledge entry:

```yaml
title: "API Authentication Pattern"
category: pattern  # architecture | convention | pattern | anti_pattern | setup | general | reference
content: |
  All API endpoints require Bearer token auth.
  Use the Authorization header with a valid JWT.
tags: ["api", "security"]
```

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Human-readable knowledge title (must not be empty) |
| `content` | string | The knowledge content (must not be empty; provide inline or via `content_file`) |

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `category` | string | `general` | One of: `architecture`, `convention`, `pattern`, `anti_pattern`, `setup`, `general`, `reference` |
| `content_file` | string | - | Path to a markdown file containing the content (relative to the YAML file's directory) |
| `tags` | array | `[]` | Tags for categorization |

## File References

For long-form content, you can reference a separate markdown file instead of inlining text. File paths are resolved relative to the YAML file's directory.

```yaml
title: "API Authentication Pattern"
category: pattern
content_file: docs/api-auth.md
tags: ["api", "security"]
```

When both `content` and `content_file` are present, `content_file` takes precedence. If the referenced file cannot be read, a warning is logged and the inline `content` is used as a fallback.

## Sync Behavior

When a task pipeline starts, Orchestra discovers YAML files in this directory and syncs them to the project's knowledge entries in the API:

- **New files**: A new knowledge entry is created in the API
- **Changed files**: The existing API knowledge entry is updated
- **Unchanged files**: Skipped (no API call)
- **Orphaned API entries**: Warned about but not auto-deleted

Repo-sourced knowledge entries are identified by `metadata.source = "repo"` and `metadata.repo_file`. The `source:repo` tag is also added automatically. These markers are managed automatically and should not be manually edited in the API.

## File Naming

Use descriptive kebab-case filenames:

```
.diraigent/knowledge/
  api-auth-pattern.yaml
  coding-conventions.yaml
  deployment-setup.yaml
  docs/
    api-auth.md           # referenced via content_file
```

The filename stem (e.g. `api-auth-pattern`) is used internally for matching but does not affect the knowledge title.

## Examples

### Inline Content

```yaml
title: "Git Branch Strategy"
category: convention
content: |
  - Main branch: `dev` for development, `main` for releases
  - Feature branches: `feature/<description>`
  - Agent branches: `agent/task-<id>` (managed by Orchestra)
tags: ["git", "workflow"]
```

### Content from File

```yaml
title: "Architecture Overview"
category: architecture
content_file: architecture-overview.md
tags: ["architecture", "overview"]
```
