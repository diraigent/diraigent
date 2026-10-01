use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;

pub fn operate(
    root: &Path,
    operation: &str,
    name: Option<&str>,
    content: Option<Value>,
) -> Result<Value> {
    let dir = root.join(".diraigent/playbooks");
    for path in [root.join(".diraigent"), dir.clone()] {
        if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            bail!("Playbook directories must not be symlinks");
        }
    }
    if operation == "list" {
        return Ok(serde_json::to_value(
            crate::repo_playbooks::load_repo_playbooks(root)?,
        )?);
    }
    let name = name.map(str::to_owned).or_else(|| {
        content
            .as_ref()
            .and_then(|v| v["title"].as_str())
            .map(crate::repo_playbooks::slugify_title)
    });
    let name = name.unwrap_or_default();
    if name.is_empty()
        || name.len() > 80
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_".contains(&b))
    {
        bail!("Use a playbook name containing lowercase letters, numbers, hyphens or underscores");
    }
    let path = dir.join(format!("{name}.yaml"));
    if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        bail!("Playbook files must not be symlinks");
    }
    match operation {
        "get" => Ok(serde_json::to_value(
            crate::repo_playbooks::parse_playbook(&path)?,
        )?),
        "delete" => {
            std::fs::remove_file(path)?;
            Ok(json!({"deleted": true}))
        }
        "create" | "update" => {
            let mut book = if operation == "update" {
                serde_json::to_value(crate::repo_playbooks::parse_playbook(&path)?)?
            } else {
                json!({})
            };
            let body = content
                .and_then(|v| v.as_object().cloned())
                .ok_or_else(|| anyhow::anyhow!("Expected playbook object"))?;
            for (key, value) in body {
                book[&key] = value;
            }
            if book["steps"].as_array().is_some_and(|steps| {
                steps
                    .iter()
                    .any(|step| step.get("description_file").is_some())
            }) {
                bail!("Use inline descriptions when saving playbooks through the API");
            }
            book["name"] = json!(name);
            std::fs::create_dir_all(&dir)?;
            // Validate before replacing any existing file; rename keeps readers atomic.
            let mut temp = tempfile::NamedTempFile::new_in(&dir)?;
            write!(temp, "{}", serde_json::to_string_pretty(&book)?)?;
            let parsed = crate::repo_playbooks::parse_playbook(temp.path())?;
            if operation == "create" {
                temp.persist_noclobber(path)?;
            } else {
                temp.persist(path)?;
            }
            Ok(serde_json::to_value(parsed)?)
        }
        _ => bail!("Unknown playbook operation"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_list_update_and_reject_traversal() {
        let root = tempfile::tempdir().unwrap();
        let body = json!({"title":"Example", "steps":[{"name":"implement"}]});
        assert!(
            operate(
                root.path(),
                "create",
                Some("../outside"),
                Some(body.clone())
            )
            .is_err()
        );
        let created = operate(root.path(), "create", Some("example"), Some(body.clone())).unwrap();
        assert_eq!(created["name"], "example");
        assert!(operate(root.path(), "create", Some("example"), Some(body)).is_err());
        assert_eq!(
            operate(root.path(), "list", None, None)
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(
            operate(
                root.path(),
                "update",
                Some("example"),
                Some(json!({"steps":[]}))
            )
            .is_err()
        );
        assert_eq!(
            operate(root.path(), "get", Some("example"), None).unwrap()["steps"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        operate(root.path(), "delete", Some("example"), None).unwrap();
    }
}
