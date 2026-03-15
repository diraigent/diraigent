use crate::error::AppError;
use crate::models::{self, *};

// ── Shared Helpers ──

/// Validate that a string has a length in the range `[min, max]`.
/// Generates a uniform error message: `"{field} must be {min}-{max} characters"`.
fn validate_str_len(value: &str, min: usize, max: usize, field: &str) -> Result<(), AppError> {
    if value.len() < min || value.len() > max {
        return Err(AppError::Validation(format!(
            "{} must be {}-{} characters",
            field, min, max
        )));
    }
    Ok(())
}

/// Validate that `value` is a member of the static `valid` slice.
/// Error: `"Invalid {field}: {value}. Valid: {valid:?}"`.
fn validate_enum_member(value: &str, valid: &[&str], field: &str) -> Result<(), AppError> {
    if !valid.contains(&value) {
        return Err(AppError::Validation(format!(
            "Invalid {}: {}. Valid: {:?}",
            field, value, valid
        )));
    }
    Ok(())
}

/// Validate an enum field that can be overridden by the project's package.
///
/// When `pkg_allowed` is `Some`, the value must appear in the package's list and
/// the error references the project's allowed values.  When `None`, falls back to
/// the static `fallback` slice.
fn validate_dyn_enum(
    value: &str,
    pkg_allowed: Option<&[String]>,
    fallback: &[&str],
    field: &str,
) -> Result<(), AppError> {
    match pkg_allowed {
        Some(list) => {
            if !list.iter().any(|k| k == value) {
                return Err(AppError::Validation(format!(
                    "Invalid {}: {}. Valid for this project: {:?}",
                    field, value, list
                )));
            }
        }
        None => {
            if !fallback.contains(&value) {
                return Err(AppError::Validation(format!(
                    "Invalid {}: {}. Valid: {:?}",
                    field, value, fallback
                )));
            }
        }
    }
    Ok(())
}

// ── URL ──

fn validate_url(url: &str, field: &str) -> Result<(), AppError> {
    if url.is_empty() {
        return Err(AppError::Validation(format!("{} must be non-empty", field)));
    }
    if url.len() > 2048 {
        return Err(AppError::Validation(format!(
            "{} must be under 2048 characters",
            field
        )));
    }
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(AppError::Validation(format!(
            "{} must start with http:// or https://",
            field
        )));
    }
    // Must have a host after the scheme
    let after_scheme = if let Some(s) = url.strip_prefix("https://") {
        s
    } else if let Some(s) = url.strip_prefix("http://") {
        s
    } else {
        url
    };
    if after_scheme.is_empty() || after_scheme.starts_with('/') {
        return Err(AppError::Validation(format!(
            "{} must include a hostname",
            field
        )));
    }
    Ok(())
}

// ── JSON Payload Limits ──

const MAX_JSON_SIZE_BYTES: usize = 65_536; // 64 KB
const MAX_JSON_DEPTH: usize = 10;

pub fn validate_json_payload(value: &serde_json::Value, field: &str) -> Result<(), AppError> {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    if serialized.len() > MAX_JSON_SIZE_BYTES {
        return Err(AppError::Validation(format!(
            "{} exceeds maximum size of {}KB",
            field,
            MAX_JSON_SIZE_BYTES / 1024
        )));
    }
    if json_depth(value) > MAX_JSON_DEPTH {
        return Err(AppError::Validation(format!(
            "{} exceeds maximum nesting depth of {}",
            field, MAX_JSON_DEPTH
        )));
    }
    Ok(())
}

fn json_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Array(arr) => 1 + arr.iter().map(json_depth).max().unwrap_or(0),
        serde_json::Value::Object(map) => 1 + map.values().map(json_depth).max().unwrap_or(0),
        _ => 0,
    }
}

// ── Project ──

const VALID_GIT_MODES: &[&str] = &["monorepo", "standalone", "none"];

pub fn validate_create_project(req: &CreateProject) -> Result<(), AppError> {
    validate_str_len(&req.name, 1, 200, "Project name")?;
    if let Some(ref slug) = req.slug {
        validate_slug(slug)?;
    }
    if let Some(ref url) = req.repo_url {
        validate_repo_url(url)?;
    }
    if let Some(ref path) = req.repo_path {
        validate_repo_path(path, "repo_path")?;
    }
    if let Some(ref branch) = req.default_branch {
        validate_str_len(branch, 1, 200, "default_branch")?;
    }
    if let Some(ref m) = req.metadata {
        validate_json_payload(m, "metadata")?;
    }
    if let Some(ref mode) = req.git_mode {
        validate_enum_member(mode, VALID_GIT_MODES, "git_mode")?;
    }
    if let Some(ref root) = req.git_root {
        validate_repo_path(root, "git_root")?;
    }
    if let Some(ref pr) = req.project_root
        && !pr.is_empty()
    {
        validate_repo_path(pr, "project_root")?;
    }
    Ok(())
}

pub fn validate_update_project(req: &UpdateProject) -> Result<(), AppError> {
    if let Some(ref name) = req.name {
        validate_str_len(name, 1, 200, "Project name")?;
    }
    if let Some(Some(ref url)) = req.repo_url {
        validate_repo_url(url)?;
    }
    if let Some(Some(ref path)) = req.repo_path {
        validate_repo_path(path, "repo_path")?;
    }
    if let Some(ref branch) = req.default_branch {
        validate_str_len(branch, 1, 200, "default_branch")?;
    }
    if let Some(ref m) = req.metadata {
        validate_json_payload(m, "metadata")?;
    }
    if let Some(ref mode) = req.git_mode {
        validate_enum_member(mode, VALID_GIT_MODES, "git_mode")?;
    }
    if let Some(Some(ref root)) = req.git_root {
        validate_repo_path(root, "git_root")?;
    }
    if let Some(Some(ref pr)) = req.project_root
        && !pr.is_empty()
    {
        validate_repo_path(pr, "project_root")?;
    }
    Ok(())
}

fn validate_slug(slug: &str) -> Result<(), AppError> {
    validate_str_len(slug, 1, 100, "Slug")?;
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(AppError::Validation(
            "Slug must be lowercase alphanumeric with hyphens only".into(),
        ));
    }
    Ok(())
}

fn validate_repo_url(url: &str) -> Result<(), AppError> {
    validate_str_len(url, 1, 2048, "repo_url")?;
    // Accept both HTTPS and SSH git URLs
    if !url.starts_with("https://")
        && !url.starts_with("http://")
        && !url.starts_with("git@")
        && !url.starts_with("ssh://")
    {
        return Err(AppError::Validation(
            "repo_url must start with https://, http://, git@, or ssh://".into(),
        ));
    }
    Ok(())
}

fn validate_repo_path(path: &str, field: &str) -> Result<(), AppError> {
    validate_str_len(path, 1, 500, field)?;
    if path.starts_with('/') || path.ends_with('/') {
        return Err(AppError::Validation(format!(
            "{} must not start or end with '/'",
            field
        )));
    }
    Ok(())
}

// ── Task ──

/// Validate a task creation request.
/// `pkg` is the project's package (if resolved); when `None`, falls back to
/// the hardcoded allow-list so existing behaviour is preserved.
pub fn validate_create_task(req: &CreateTask, pkg: Option<&Package>) -> Result<(), AppError> {
    validate_str_len(&req.title, 1, 500, "Task title")?;
    if let Some(ref kind) = req.kind {
        validate_dyn_enum(
            kind,
            pkg.map(|p| p.allowed_task_kinds.as_slice()),
            models::TASK_KINDS,
            "task kind",
        )?;
    }
    if let Some(ref ctx) = req.context {
        validate_json_payload(ctx, "context")?;
    }
    Ok(())
}

/// Validate a task update request.
/// `pkg` is the project's package (if resolved); when `None`, falls back to
/// the hardcoded allow-list so existing behaviour is preserved.
pub fn validate_update_task(req: &UpdateTask, pkg: Option<&Package>) -> Result<(), AppError> {
    if let Some(ref title) = req.title {
        validate_str_len(title, 1, 500, "Task title")?;
    }
    if let Some(ref kind) = req.kind {
        validate_dyn_enum(
            kind,
            pkg.map(|p| p.allowed_task_kinds.as_slice()),
            models::TASK_KINDS,
            "task kind",
        )?;
    }
    if let Some(ref ctx) = req.context {
        validate_json_payload(ctx, "context")?;
    }
    Ok(())
}

// ── Agent ──

pub fn validate_create_agent(req: &CreateAgent) -> Result<(), AppError> {
    validate_str_len(&req.name, 1, 100, "Agent name")?;
    if let Some(ref caps) = req.capabilities {
        validate_capabilities(caps)?;
    }
    Ok(())
}

pub fn validate_update_agent(req: &UpdateAgent) -> Result<(), AppError> {
    if let Some(ref name) = req.name {
        validate_str_len(name, 1, 100, "Agent name")?;
    }
    if let Some(ref caps) = req.capabilities {
        validate_capabilities(caps)?;
    }
    Ok(())
}

fn validate_capabilities(caps: &[String]) -> Result<(), AppError> {
    for cap in caps {
        if cap.is_empty() {
            return Err(AppError::Validation(
                "Capability items must be non-empty".into(),
            ));
        }
    }
    Ok(())
}

// ── Role ──

pub fn validate_create_role(req: &CreateRole) -> Result<(), AppError> {
    if let Some(ref auths) = req.authorities {
        validate_authorities(auths)?;
    }
    Ok(())
}

pub fn validate_update_role(req: &UpdateRole) -> Result<(), AppError> {
    if let Some(ref auths) = req.authorities {
        validate_authorities(auths)?;
    }
    Ok(())
}

fn validate_authorities(auths: &[String]) -> Result<(), AppError> {
    for a in auths {
        validate_enum_member(a, models::AUTHORITIES, "authority")?;
    }
    Ok(())
}

// ── Priority (used by Work, not Task) ──

fn validate_priority(priority: i32) -> Result<(), AppError> {
    if !(-1000..=1000).contains(&priority) {
        return Err(AppError::Validation(
            "Priority must be between -1000 and 1000".into(),
        ));
    }
    Ok(())
}

// ── Work ──

pub fn validate_create_work(req: &CreateWork) -> Result<(), AppError> {
    validate_str_len(&req.title, 1, 500, "Work title")?;
    if let Some(ref work_type) = req.work_type {
        validate_enum_member(work_type, models::WORK_TYPES, "work type")?;
    }
    if let Some(ref intent_type) = req.intent_type {
        validate_enum_member(intent_type, models::WORK_INTENT_TYPES, "work intent type")?;
    }
    if let Some(priority) = req.priority {
        validate_priority(priority)?;
    }
    Ok(())
}

pub fn validate_update_work(req: &UpdateWork) -> Result<(), AppError> {
    if let Some(ref title) = req.title {
        validate_str_len(title, 1, 500, "Work title")?;
    }
    if let Some(ref status) = req.status {
        validate_enum_member(status, models::WORK_STATUSES, "work status")?;
    }
    if let Some(ref work_type) = req.work_type {
        validate_enum_member(work_type, models::WORK_TYPES, "work type")?;
    }
    if let Some(Some(ref intent_type)) = req.intent_type {
        validate_enum_member(intent_type, models::WORK_INTENT_TYPES, "work intent type")?;
    }
    if let Some(priority) = req.priority {
        validate_priority(priority)?;
    }
    Ok(())
}

// ── Knowledge ──

pub fn validate_create_knowledge(
    req: &CreateKnowledge,
    pkg: Option<&Package>,
) -> Result<(), AppError> {
    if req.title.is_empty() {
        return Err(AppError::Validation(
            "Knowledge title must be non-empty".into(),
        ));
    }
    if req.content.is_empty() {
        return Err(AppError::Validation(
            "Knowledge content must be non-empty".into(),
        ));
    }
    if let Some(ref cat) = req.category {
        validate_dyn_enum(
            cat,
            pkg.map(|p| p.allowed_knowledge_categories.as_slice()),
            models::KNOWLEDGE_CATEGORIES,
            "knowledge category",
        )?;
    }
    Ok(())
}

pub fn validate_update_knowledge(
    req: &UpdateKnowledge,
    pkg: Option<&Package>,
) -> Result<(), AppError> {
    if let Some(ref title) = req.title
        && title.is_empty()
    {
        return Err(AppError::Validation(
            "Knowledge title must be non-empty".into(),
        ));
    }
    if let Some(ref content) = req.content
        && content.is_empty()
    {
        return Err(AppError::Validation(
            "Knowledge content must be non-empty".into(),
        ));
    }
    if let Some(ref cat) = req.category {
        validate_dyn_enum(
            cat,
            pkg.map(|p| p.allowed_knowledge_categories.as_slice()),
            models::KNOWLEDGE_CATEGORIES,
            "knowledge category",
        )?;
    }
    Ok(())
}

// ── Decision ──

pub fn validate_create_decision(req: &CreateDecision) -> Result<(), AppError> {
    if req.title.is_empty() {
        return Err(AppError::Validation(
            "Decision title must be non-empty".into(),
        ));
    }
    if req.context.is_empty() {
        return Err(AppError::Validation(
            "Decision context must be non-empty".into(),
        ));
    }
    Ok(())
}

pub fn validate_update_decision(req: &UpdateDecision) -> Result<(), AppError> {
    if let Some(ref title) = req.title
        && title.is_empty()
    {
        return Err(AppError::Validation(
            "Decision title must be non-empty".into(),
        ));
    }
    if let Some(ref status) = req.status {
        validate_enum_member(status, models::DECISION_STATUSES, "decision status")?;
    }
    Ok(())
}

// ── Observation ──

pub fn validate_create_observation(
    req: &CreateObservation,
    pkg: Option<&Package>,
) -> Result<(), AppError> {
    if let Some(ref kind) = req.kind {
        validate_dyn_enum(
            kind,
            pkg.map(|p| p.allowed_observation_kinds.as_slice()),
            models::OBSERVATION_KINDS,
            "observation kind",
        )?;
    }
    if let Some(ref sev) = req.severity {
        // Severity is a protocol-level invariant — always validated against hardcoded list
        validate_enum_member(sev, models::OBSERVATION_SEVERITIES, "observation severity")?;
    }
    // Source is a soft enum — validate against known values but don't reject unknown ones
    // so new sources can be added without API changes.
    Ok(())
}

// ── Event Observation Rule ──

pub fn validate_create_event_observation_rule(
    req: &CreateEventObservationRule,
) -> Result<(), AppError> {
    if req.name.is_empty() {
        return Err(AppError::Validation("Rule name must be non-empty".into()));
    }
    if let Some(ref kind) = req.observation_kind {
        validate_enum_member(kind, models::OBSERVATION_KINDS, "observation kind")?;
    }
    if let Some(ref sev) = req.observation_severity {
        validate_enum_member(sev, models::OBSERVATION_SEVERITIES, "observation severity")?;
    }
    if let Some(ref ek) = req.event_kind {
        validate_enum_member(ek, models::EVENT_KINDS, "event kind")?;
    }
    if let Some(ref sev) = req.severity_gte {
        validate_enum_member(sev, models::EVENT_SEVERITIES, "severity_gte")?;
    }
    if req.title_template.is_empty() {
        return Err(AppError::Validation(
            "title_template must be non-empty".into(),
        ));
    }
    Ok(())
}

// ── Integration ──

pub fn validate_create_integration(
    req: &CreateIntegration,
    pkg: Option<&Package>,
) -> Result<(), AppError> {
    if req.name.is_empty() {
        return Err(AppError::Validation(
            "Integration name must be non-empty".into(),
        ));
    }
    if req.provider.is_empty() {
        return Err(AppError::Validation(
            "Integration provider must be non-empty".into(),
        ));
    }
    validate_dyn_enum(
        &req.kind,
        pkg.map(|p| p.allowed_integration_kinds.as_slice()),
        models::INTEGRATION_KINDS,
        "integration kind",
    )?;
    if let Some(ref at) = req.auth_type {
        // auth_type is a protocol-level invariant — always validated against hardcoded list
        validate_enum_member(at, models::AUTH_TYPES, "auth type")?;
    }
    if let Some(ref url) = req.base_url {
        validate_url(url, "Integration base_url")?;
    }
    Ok(())
}

// ── Webhook ──

pub fn validate_create_webhook(req: &CreateWebhook) -> Result<(), AppError> {
    validate_str_len(&req.name, 1, 200, "Webhook name")?;
    validate_url(&req.url, "Webhook URL")?;
    if req.events.is_empty() {
        return Err(AppError::Validation(
            "Webhook must subscribe to at least one event".into(),
        ));
    }
    Ok(())
}

pub fn validate_update_webhook(req: &UpdateWebhook) -> Result<(), AppError> {
    if let Some(ref name) = req.name {
        validate_str_len(name, 1, 200, "Webhook name")?;
    }
    if let Some(ref url) = req.url {
        validate_url(url, "Webhook URL")?;
    }
    if let Some(ref events) = req.events
        && events.is_empty()
    {
        return Err(AppError::Validation(
            "Webhook must subscribe to at least one event".into(),
        ));
    }
    Ok(())
}

// ── Event ──

pub fn validate_create_event(req: &CreateEvent, pkg: Option<&Package>) -> Result<(), AppError> {
    validate_dyn_enum(
        &req.kind,
        pkg.map(|p| p.allowed_event_kinds.as_slice()),
        models::EVENT_KINDS,
        "event kind",
    )?;
    if let Some(ref sev) = req.severity {
        // Severity is a protocol-level invariant — always validated against hardcoded list
        validate_enum_member(sev, models::EVENT_SEVERITIES, "event severity")?;
    }
    if req.source.is_empty() {
        return Err(AppError::Validation(
            "Event source must be non-empty".into(),
        ));
    }
    if req.title.is_empty() {
        return Err(AppError::Validation("Event title must be non-empty".into()));
    }
    Ok(())
}

// ── Verification ──

const VERIFICATION_KINDS: &[&str] = &["test", "acceptance", "sign_off"];
const VERIFICATION_STATUSES: &[&str] = &["pass", "fail", "pending", "skipped"];

pub fn validate_create_verification(req: &CreateVerification) -> Result<(), AppError> {
    if req.title.is_empty() {
        return Err(AppError::Validation(
            "Verification title must be non-empty".into(),
        ));
    }
    validate_enum_member(&req.kind, VERIFICATION_KINDS, "verification kind")?;
    if let Some(ref status) = req.status {
        validate_enum_member(status, VERIFICATION_STATUSES, "verification status")?;
    }
    Ok(())
}

pub fn validate_update_verification(req: &UpdateVerification) -> Result<(), AppError> {
    if let Some(ref status) = req.status {
        validate_enum_member(status, VERIFICATION_STATUSES, "verification status")?;
    }
    Ok(())
}

pub fn validate_create_package(req: &CreatePackage) -> Result<(), AppError> {
    if req.slug.is_empty() || req.slug.len() > 100 {
        return Err(AppError::Validation("slug must be 1-100 characters".into()));
    }
    if !req
        .slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(AppError::Validation(
            "slug must be lowercase alphanumeric with hyphens only".into(),
        ));
    }
    if req.name.is_empty() || req.name.len() > 200 {
        return Err(AppError::Validation("name must be 1-200 characters".into()));
    }
    Ok(())
}

// ── Report ──

pub fn validate_create_report(req: &CreateReport) -> Result<(), AppError> {
    if req.title.is_empty() {
        return Err(AppError::Validation(
            "Report title must be non-empty".into(),
        ));
    }
    validate_str_len(&req.title, 1, 500, "Report title")?;
    validate_enum_member(&req.kind, models::REPORT_KINDS, "report kind")?;
    if req.prompt.is_empty() {
        return Err(AppError::Validation(
            "Report prompt must be non-empty".into(),
        ));
    }
    if let Some(ref m) = req.metadata {
        validate_json_payload(m, "metadata")?;
    }
    Ok(())
}

pub fn validate_update_report(req: &UpdateReport) -> Result<(), AppError> {
    if let Some(ref title) = req.title
        && title.is_empty()
    {
        return Err(AppError::Validation(
            "Report title must be non-empty".into(),
        ));
    }
    if let Some(ref status) = req.status {
        validate_enum_member(status, models::REPORT_STATUSES, "report status")?;
    }
    if let Some(ref m) = req.metadata {
        validate_json_payload(m, "metadata")?;
    }
    Ok(())
}

pub fn validate_update_package(req: &UpdatePackage) -> Result<(), AppError> {
    if let Some(ref slug) = req.slug {
        if slug.is_empty() || slug.len() > 100 {
            return Err(AppError::Validation("slug must be 1-100 characters".into()));
        }
        if !slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(AppError::Validation(
                "slug must be lowercase alphanumeric with hyphens only".into(),
            ));
        }
    }
    if let Some(ref name) = req.name
        && (name.is_empty() || name.len() > 200)
    {
        return Err(AppError::Validation("name must be 1-200 characters".into()));
    }
    Ok(())
}

// ── Provider Config ──

pub fn validate_create_provider_config(req: &CreateProviderConfig) -> Result<(), AppError> {
    if req.provider.is_empty() {
        return Err(AppError::Validation(
            "Provider name must be non-empty".into(),
        ));
    }
    if req.provider.len() > 100 {
        return Err(AppError::Validation(
            "Provider name must be at most 100 characters".into(),
        ));
    }
    if let Some(ref url) = req.base_url {
        validate_url(url, "Provider base_url")?;
    }
    Ok(())
}

pub fn validate_update_provider_config(req: &UpdateProviderConfig) -> Result<(), AppError> {
    if let Some(ref url) = req.base_url {
        validate_url(url, "Provider base_url")?;
    }
    Ok(())
}
