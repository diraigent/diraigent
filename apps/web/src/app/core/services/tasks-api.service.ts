import { Injectable } from '@angular/core';
import { HttpParams } from '@angular/common/http';
import { EMPTY, Observable } from 'rxjs';
import { BaseCrudApiService } from './base-crud-api.service';

/** Well-known task states. Playbooks may define additional step states dynamically. */
export type TaskState = 'backlog' | 'ready' | 'working' | 'implement' | 'review' | 'merge' | 'dream' | 'human_review' | 'done' | 'cancelled' | `wait:${string}` | (string & {});
/** Well-known task kinds. Packages may define additional kinds dynamically. */
export type TaskKind = 'feature' | 'bug' | 'chore' | 'spike' | 'refactor' | 'docs' | 'test' | 'research' | (string & {});
export type UpdateKind = 'progress' | 'blocker' | 'question' | 'artifact' | 'note';

export interface SpDecisionSummary {
  id: string;
  title: string;
  status: string;
  rationale_excerpt: string | null;
}

export interface SpTask {
  id: string;
  project_id: string;
  number: number;
  title: string;
  kind: string;
  state: string;
  urgent: boolean;
  context: Record<string, unknown>;
  assigned_agent_id: string | null;
  claimed_at: string | null;
  required_capabilities: string[];
  assigned_role_id: string | null;
  delegated_by: string | null;
  delegated_at: string | null;
  playbook_id: string | null;
  playbook_step: number | null;
  decision_id: string | null;
  created_by: string;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
  reverted_at: string | null;
  flagged: boolean;
  parent_id: string | null;
  /** Enriched field — only present on GET /tasks/:id */
  decision?: SpDecisionSummary | null;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
}

export interface SpTaskDependency {
  task_id: string;
  depends_on: string;
}

export interface SpTaskDependencyInfo {
  task_id: string;
  depends_on: string;
  title: string;
  state: string;
}

export interface SpTaskDependencies {
  depends_on: SpTaskDependencyInfo[];
  blocks: SpTaskDependencyInfo[];
}

export interface SpTaskUpdate {
  id: string;
  task_id: string;
  agent_id: string | null;
  user_id: string | null;
  kind: string;
  content: string;
  metadata: Record<string, unknown>;
  created_at: string;
}

export interface SpTaskComment {
  id: string;
  task_id: string;
  agent_id: string | null;
  user_id: string | null;
  content: string;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  limit: number;
  offset: number;
  has_more: boolean;
}

export interface TaskListFilters {
  state?: string;
  kind?: string;
  agent_id?: string;
  search?: string;
  limit?: number;
  offset?: number;
  hide_done_before?: string;
  work_id?: string;
  unlinked?: boolean;
}

export interface CreateTaskRequest {
  title: string;
  kind?: string;
  urgent?: boolean;
  context?: Record<string, unknown>;
  required_capabilities?: string[];
  playbook_id?: string;
  decision_id?: string;
  work_id?: string;
  parent_id?: string;
}

export interface UpdateTaskRequest {
  title?: string;
  kind?: string;
  urgent?: boolean;
  context?: Record<string, unknown>;
  required_capabilities?: string[];
  playbook_id?: string | null;
  playbook_step?: number | null;
  flagged?: boolean;
}

export interface CreateTaskUpdateRequest {
  kind?: string;
  content: string;
  metadata?: Record<string, unknown>;
}

export interface CreateTaskCommentRequest {
  content: string;
  metadata?: Record<string, unknown>;
}

export interface BulkResult {
  succeeded: string[];
  failed: { task_id: string; error: string }[];
}

export interface ChangedFileSummary {
  id: string;
  task_id: string;
  path: string;
  change_type: 'added' | 'modified' | 'deleted';
  created_at: string;
}

export interface ChangedFile extends ChangedFileSummary {
  diff: string | null;
}

@Injectable({ providedIn: 'root' })
export class TasksApiService extends BaseCrudApiService<SpTask, CreateTaskRequest, UpdateTaskRequest> {
  protected readonly resource = 'tasks';

  list(filters?: TaskListFilters): Observable<PaginatedResponse<SpTask>> {
    if (!this.projectId) return EMPTY;
    return this.listForProject(this.projectId, filters);
  }

  listForProject(projectId: string, filters?: TaskListFilters): Observable<PaginatedResponse<SpTask>> {
    if (!projectId) return EMPTY;
    let params = new HttpParams();
    if (filters?.state) params = params.set('state', filters.state);
    if (filters?.kind) params = params.set('kind', filters.kind);
    if (filters?.agent_id) params = params.set('agent_id', filters.agent_id);
    if (filters?.search) params = params.set('search', filters.search);
    if (filters?.limit != null) params = params.set('limit', filters.limit);
    if (filters?.offset != null) params = params.set('offset', filters.offset);
    if (filters?.hide_done_before) params = params.set('hide_done_before', filters.hide_done_before);
    if (filters?.work_id) params = params.set('work_id', filters.work_id);
    if (filters?.unlinked) params = params.set('unlinked', 'true');
    return this.http.get<PaginatedResponse<SpTask>>(`${this.baseUrl}/${projectId}/tasks`, { params });
  }

  transition(id: string, state: string): Observable<SpTask> {
    return this.http.post<SpTask>(`${this.baseUrl}/tasks/${id}/transition`, { state });
  }

  claim(id: string, agentId: string): Observable<SpTask> {
    return this.http.post<SpTask>(`${this.baseUrl}/tasks/${id}/claim`, { agent_id: agentId });
  }

  release(id: string): Observable<SpTask> {
    return this.http.post<SpTask>(`${this.baseUrl}/tasks/${id}/release`, {});
  }

  delegate(id: string, agentId: string, roleId?: string): Observable<SpTask> {
    return this.http.post<SpTask>(`${this.baseUrl}/tasks/${id}/delegate`, {
      agent_id: agentId,
      role_id: roleId ?? null,
    });
  }

  listUpdates(taskId: string): Observable<SpTaskUpdate[]> {
    return this.http.get<SpTaskUpdate[]>(`${this.baseUrl}/tasks/${taskId}/updates`);
  }

  createUpdate(taskId: string, data: CreateTaskUpdateRequest): Observable<SpTaskUpdate> {
    return this.http.post<SpTaskUpdate>(`${this.baseUrl}/tasks/${taskId}/updates`, data);
  }

  listComments(taskId: string): Observable<SpTaskComment[]> {
    return this.http.get<SpTaskComment[]>(`${this.baseUrl}/tasks/${taskId}/comments`);
  }

  createComment(taskId: string, data: CreateTaskCommentRequest): Observable<SpTaskComment> {
    return this.http.post<SpTaskComment>(`${this.baseUrl}/tasks/${taskId}/comments`, data);
  }

  listTasksWithBlockers(projectId: string): Observable<SpTask[]> {
    return this.http.get<SpTask[]>(`${this.baseUrl}/${projectId}/tasks/with-blockers`);
  }

  listBlockedIds(): Observable<string[]> {
    if (!this.projectId) return EMPTY;
    return this.http.get<string[]>(`${this.baseUrl}/${this.projectId}/tasks/blocked`);
  }

  listWorkLinkedIds(): Observable<string[]> {
    if (!this.projectId) return EMPTY;
    return this.http.get<string[]>(`${this.baseUrl}/${this.projectId}/tasks/work-linked`);
  }

  listFlaggedIds(): Observable<string[]> {
    if (!this.projectId) return EMPTY;
    return this.http.get<string[]>(`${this.baseUrl}/${this.projectId}/tasks/flagged`);
  }

  listDependencies(taskId: string): Observable<SpTaskDependencies> {
    return this.http.get<SpTaskDependencies>(`${this.baseUrl}/tasks/${taskId}/dependencies`);
  }

  addDependency(taskId: string, dependsOn: string): Observable<SpTaskDependency> {
    return this.http.post<SpTaskDependency>(`${this.baseUrl}/tasks/${taskId}/dependencies`, { depends_on: dependsOn });
  }

  removeDependency(taskId: string, depId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/tasks/${taskId}/dependencies/${depId}`);
  }

  // Bulk actions

  bulkTransition(taskIds: string[], state: string): Observable<BulkResult> {
    if (!this.projectId) return EMPTY;
    return this.http.post<BulkResult>(`${this.baseUrl}/${this.projectId}/tasks/bulk/transition`, {
      task_ids: taskIds,
      state,
    });
  }

  bulkDelegate(taskIds: string[], agentId: string, roleId?: string): Observable<BulkResult> {
    if (!this.projectId) return EMPTY;
    return this.http.post<BulkResult>(`${this.baseUrl}/${this.projectId}/tasks/bulk/delegate`, {
      task_ids: taskIds,
      agent_id: agentId,
      role_id: roleId ?? null,
    });
  }

  bulkDelete(taskIds: string[]): Observable<BulkResult> {
    if (!this.projectId) return EMPTY;
    return this.http.post<BulkResult>(`${this.baseUrl}/${this.projectId}/tasks/bulk/delete`, {
      task_ids: taskIds,
    });
  }

  // Changed files

  listChangedFiles(taskId: string): Observable<ChangedFileSummary[]> {
    return this.http.get<ChangedFileSummary[]>(`${this.baseUrl}/tasks/${taskId}/changed-files`);
  }

  getChangedFile(taskId: string, fileId: string): Observable<ChangedFile> {
    return this.http.get<ChangedFile>(`${this.baseUrl}/tasks/${taskId}/changed-files/${fileId}`);
  }

  // Hierarchy

  listChildren(taskId: string): Observable<SpTask[]> {
    return this.http.get<SpTask[]>(`${this.baseUrl}/tasks/${taskId}/children`);
  }
}
