use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::db::DiraigentDb;
use crate::models::*;
use crate::ws_protocol::WsMessage;
use crate::ws_registry::WsRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatSseEvent {
    #[serde(rename = "text")]
    Text { content: String },
    #[serde(rename = "tool_start")]
    ToolStart { tool_name: String, tool_id: String },
    #[serde(rename = "tool_end")]
    ToolEnd { tool_id: String, success: bool },
    #[serde(rename = "done")]
    Done { message: DoneMessage },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoneMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub async fn build_system_prompt(
    db: &dyn DiraigentDb,
    project_id: Uuid,
    api_base: &str,
    auth_header: &str,
) -> String {
    let project = db.get_project_by_id(project_id).await.ok();

    let tasks: Vec<Task> = db
        .list_tasks(
            project_id,
            &TaskFilters {
                limit: Some(50),
                offset: Some(0),
                ..Default::default()
            },
        )
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|t| !matches!(t.state.as_str(), "done" | "cancelled" | "backlog"))
        .take(20)
        .collect();

    let knowledge = db
        .list_knowledge(
            project_id,
            &KnowledgeFilters {
                limit: Some(10),
                offset: Some(0),
                ..Default::default()
            },
        )
        .await
        .unwrap_or_default();

    let decisions = db
        .list_decisions(
            project_id,
            &DecisionFilters {
                limit: Some(10),
                offset: Some(0),
                ..Default::default()
            },
        )
        .await
        .unwrap_or_default();

    let mut prompt = String::from(
        "You are an AI assistant for the Diraigent project management platform. \
         You help users manage their project by creating tasks, recording observations, \
         and answering questions about their project data.\n\n\
         Be concise and helpful. When creating items, confirm what you created. \
         When listing items, format them clearly.\n\n",
    );

    prompt.push_str(&format!(
        "## Diraigent API\n\
         Base URL: {api_base}/v1\n\
         Use `curl` to interact with the API. All endpoints accept/return JSON.\n\
         Include `-H 'Content-Type: application/json'` and `-H '{auth_header}'` headers.\n\n\
         ### Endpoints\n\
         - `GET /{{project_id}}/tasks?state=<state>&kind=<kind>&search=<term>&limit=<n>` — List tasks\n\
         - `POST /{{project_id}}/tasks` — Create task\n\
         - `GET /{{project_id}}/tasks/<task_id>` — Get task details\n\
         - `PATCH /{{project_id}}/tasks/<task_id>` — Update task\n\
         - `DELETE /{{project_id}}/tasks/<task_id>` — Delete task\n\
         - `POST /{{project_id}}/tasks/<task_id>/transition` — Transition\n\
         - `GET /{{project_id}}/search?q=<query>&types=<task,knowledge,decision>&limit=<n>` — Search\n\
         - `GET /{{project_id}}/knowledge?category=<cat>&limit=<n>` — List knowledge\n\
         - `POST /{{project_id}}/knowledge` — Create knowledge\n\
         - `GET /{{project_id}}/decisions?status=<status>&limit=<n>` — List decisions\n\
         - `POST /{{project_id}}/decisions` — Create decision\n\
         - `POST /{{project_id}}/observations` — Create observation\n\
         - `GET /{{project_id}}/verifications?task_id=<id>&kind=<kind>&status=<status>` — List verifications\n\
         - `POST /{{project_id}}/verifications` — Create verification\n\
         - `GET /{{project_id}}/goals` — List goals\n\
         - `GET /{{project_id}}/playbooks` — List playbooks\n\n",
    ));

    if let Some(ref p) = project {
        prompt.push_str(&format!(
            "## Current Project\n- Name: {}\n- ID: {}\n- Slug: {}\n",
            p.name, p.id, p.slug
        ));
        if let Some(ref desc) = p.description {
            prompt.push_str(&format!("- Description: {desc}\n"));
        }
        if let Some(ref path) = p.repo_path {
            prompt.push_str(&format!("- Repo path: {path}\n"));
        }
        if !p.default_branch.is_empty() {
            prompt.push_str(&format!("- Default branch: {}\n", p.default_branch));
        }
        if let Some(ref svc) = p.service_name {
            prompt.push_str(&format!("- Service name: {svc}\n"));
        }
        prompt.push('\n');
    }

    if !tasks.is_empty() {
        prompt.push_str("## Active Tasks\n");
        for t in &tasks {
            prompt.push_str(&format!(
                "- [{}] {} (state: {}, priority: {}, kind: {})\n",
                t.id, t.title, t.state, t.priority, t.kind
            ));
        }
        prompt.push('\n');
    }

    if !knowledge.is_empty() {
        prompt.push_str("## Knowledge Base\n");
        for k in &knowledge {
            prompt.push_str(&format!(
                "- [{}] {} (category: {})\n",
                k.id, k.title, k.category
            ));
        }
        prompt.push('\n');
    }

    if !decisions.is_empty() {
        prompt.push_str("## Decisions\n");
        for d in &decisions {
            prompt.push_str(&format!(
                "- [{}] {} (status: {})\n",
                d.id, d.title, d.status
            ));
        }
        prompt.push('\n');
    }

    prompt
}

pub struct ChatStreamParams {
    pub db: Arc<dyn DiraigentDb>,
    pub ws_registry: Arc<WsRegistry>,
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub messages: Vec<Message>,
    pub tx: mpsc::Sender<ChatSseEvent>,
    pub api_base: String,
    pub auth_header: String,
}

pub async fn run_chat_stream(p: ChatStreamParams) {
    let ChatStreamParams {
        db,
        ws_registry,
        project_id,
        user_id,
        messages,
        tx,
        api_base,
        auth_header,
    } = p;
    let model = std::env::var("CHAT_MODEL").unwrap_or_else(|_| "sonnet".into());
    let system_prompt = build_system_prompt(db.as_ref(), project_id, &api_base, &auth_header).await;
    let session_id = Uuid::now_v7().to_string();

    // Find the project's tenant to look up connected agents
    let tenant_id = match db.get_project_by_id(project_id).await {
        Ok(p) => p.tenant_id,
        Err(e) => {
            let _ = tx
                .send(ChatSseEvent::Error {
                    message: format!("Failed to find project: {e}"),
                })
                .await;
            return;
        }
    };

    let agent_ids = match db.list_tenant_agent_ids(tenant_id).await {
        Ok(ids) => ids,
        Err(e) => {
            let _ = tx
                .send(ChatSseEvent::Error {
                    message: format!("Failed to find agents: {e}"),
                })
                .await;
            return;
        }
    };

    let agent_id = match ws_registry.find_connected_agent(&agent_ids) {
        Some(id) => id,
        None => {
            let _ = tx
                .send(ChatSseEvent::Error {
                    message: "No orchestra agent connected".into(),
                })
                .await;
            return;
        }
    };

    // Register chat session to receive events back from orchestra
    ws_registry.register_chat_session(session_id.clone(), tx.clone());

    // Send request via WebSocket
    let ws_msg = WsMessage::ChatRequest {
        session_id: session_id.clone(),
        project_id,
        user_id,
        messages,
        system_prompt,
        model,
    };

    if !ws_registry.send_to_agent(agent_id, ws_msg) {
        let _ = tx
            .send(ChatSseEvent::Error {
                message: "Failed to send to orchestra".into(),
            })
            .await;
        ws_registry.remove_chat_session(&session_id);
        return;
    }

    // Spawn a timeout watcher so we don't hang forever.
    // 600s (10 minutes) to accommodate extended thinking / tool use chains.
    let session_clone = session_id.clone();
    let registry_clone = ws_registry.clone();
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(600)).await;
        // If session is still active after 600s, send timeout error
        if registry_clone.is_chat_active(&session_clone) {
            let _ = tx_clone
                .send(ChatSseEvent::Error {
                    message: "Chat session timed out (no response from worker)".into(),
                })
                .await;
            registry_clone.remove_chat_session(&session_clone);
        }
    });

    // Events flow directly from WS reader -> registry -> tx -> SSE stream.
    // This function returns immediately; the session ends when a terminal
    // event (Done/Error) is routed or the timeout fires.
}
