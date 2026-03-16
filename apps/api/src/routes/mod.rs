mod account;
mod agents;
mod audit;
mod authentik_webhooks;
mod changed_files;
mod chat;
mod ci;
mod context;
mod decisions;
mod event_rules;
mod events;
mod files;
mod forgejo_webhooks;
mod git;
mod github_webhooks;
mod integrations;
mod knowledge;
mod locks;
mod logs;
mod members;
mod metrics;
mod observations;
mod packages;
mod playbooks;
mod projects;
mod provider_configs;
mod reports;
mod roles;
mod search;
mod settings;
mod source;
mod sse;
mod step_templates;
mod task_logs;
mod tasks;
pub(crate) mod tenants;
mod verifications;
mod webhooks;
pub(crate) mod work;
mod ws;

use std::future::Future;

use axum::{Json, Router};
use serde::Serialize;

use crate::AppState;
use crate::error::AppError;
use crate::models::PaginatedResponse;

/// Shared helper: resolves limit/offset defaults, runs list+count concurrently,
/// and wraps the result in a [`PaginatedResponse`].
pub(super) async fn paginate<T, Fut1, Fut2>(
    limit: Option<i64>,
    offset: Option<i64>,
    list_fut: Fut1,
    count_fut: Fut2,
) -> Result<Json<PaginatedResponse<T>>, AppError>
where
    T: Serialize,
    Fut1: Future<Output = Result<Vec<T>, AppError>>,
    Fut2: Future<Output = Result<i64, AppError>>,
{
    let limit = limit.unwrap_or(50).min(100);
    let offset = offset.unwrap_or(0);
    let (data, total) = tokio::try_join!(list_fut, count_fut)?;
    Ok(Json(PaginatedResponse {
        has_more: offset + limit < total,
        data,
        total,
        limit,
        offset,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(projects::routes())
        .merge(tasks::routes())
        .merge(changed_files::routes())
        .merge(agents::routes())
        .merge(work::routes())
        .merge(knowledge::routes())
        .merge(decisions::routes())
        .merge(observations::routes())
        .merge(packages::routes())
        .merge(playbooks::routes())
        .merge(event_rules::routes())
        .merge(events::routes())
        .merge(integrations::routes())
        .merge(roles::routes())
        .merge(members::routes())
        .merge(audit::routes())
        .merge(context::routes())
        .merge(webhooks::routes())
        .merge(locks::routes())
        .merge(metrics::routes())
        .merge(search::routes())
        .merge(sse::routes())
        .merge(chat::routes())
        .merge(verifications::routes())
        .merge(git::routes())
        .merge(source::routes())
        .merge(files::routes())
        .merge(logs::routes())
        .merge(tenants::routes())
        .merge(settings::routes())
        .merge(step_templates::routes())
        .merge(provider_configs::routes())
        .merge(reports::routes())
        .merge(task_logs::routes())
        .merge(forgejo_webhooks::routes())
        .merge(github_webhooks::routes())
        .merge(ci::routes())
        .merge(account::routes())
        .merge(authentik_webhooks::routes())
        .merge(ws::routes())
}
