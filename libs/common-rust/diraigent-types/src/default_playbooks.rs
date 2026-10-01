use serde_json::Value;

/// Bundled defaults remain available when no Orchestra is connected.
pub fn default_playbooks() -> Vec<Value> {
    [
        include_str!("../resources/playbooks/standard-lifecycle.json"),
        include_str!("../resources/playbooks/standard-backlog-start.json"),
        include_str!("../resources/playbooks/dreamer.json"),
        include_str!("../resources/playbooks/researcher.json"),
    ]
    .into_iter()
    .map(|raw| {
        let mut book: Value = serde_json::from_str(raw).expect("valid bundled playbook");
        book["id"] = book["name"].clone();
        book["tenant_id"] = Value::Null;
        book
    })
    .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn defaults_have_steps_and_inherit_the_worker_provider_and_model() {
        let books = super::default_playbooks();
        assert_eq!(books.len(), 4);
        for book in books {
            assert!(book["name"].as_str().is_some());
            let steps = book["steps"].as_array().unwrap();
            assert!(!steps.is_empty());
            for step in steps {
                assert!(step["name"].as_str().is_some());
                assert!(step.get("model").is_none());
                assert!(step.get("provider").is_none());
            }
        }
    }
}
