import { Injectable } from '@angular/core';
import { HttpParams } from '@angular/common/http';
import { EMPTY, Observable } from 'rxjs';
import { BaseCrudApiService } from './base-crud-api.service';
import { SpTask } from './tasks-api.service';

export type WorkStatus = 'active' | 'achieved' | 'paused' | 'abandoned';
export type WorkType = 'epic' | 'feature' | 'milestone' | 'sprint' | 'initiative';

export interface SpWork {
  id: string;
  project_id: string;
  title: string;
  description: string;
  status: WorkStatus;
  work_type: WorkType;
  priority: number;
  parent_work_id: string | null;
  auto_status: boolean;
  success_criteria: string;
  metadata: Record<string, unknown>;
  created_at: string;
  created_by: string;
  updated_at: string;
}

export interface SpWorkProgress {
  total_tasks: number;
  done_tasks: number;
  percentage: number;
}

export interface SpWorkStats {
  work_id: string;
  backlog_count: number;
  ready_count: number;
  working_count: number;
  done_count: number;
  cancelled_count: number;
  total_count: number;
  kind_breakdown: Record<string, number>;
  total_cost_usd: number;
  total_input_tokens: number;
  total_output_tokens: number;
  blocked_count: number;
  avg_completion_hours: number | null;
  oldest_open_task_date: string | null;
}

export interface SpWorkComment {
  id: string;
  work_id: string;
  agent_id: string | null;
  user_id: string | null;
  content: string;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface SpWorkCreate {
  title: string;
  description: string;
  success_criteria: string;
  work_type?: WorkType;
  priority?: number;
  parent_work_id?: string | null;
  auto_status?: boolean;
}

export interface WorkTodo {
  id: number;
  text: string;
  done: boolean;
}

export interface SpWorkUpdate {
  title?: string;
  description?: string;
  status?: WorkStatus;
  success_criteria?: string;
  work_type?: WorkType;
  priority?: number;
  parent_work_id?: string | null;
  auto_status?: boolean;
  metadata?: Record<string, unknown>;
}

@Injectable({ providedIn: 'root' })
export class WorkApiService extends BaseCrudApiService<SpWork, SpWorkCreate, SpWorkUpdate> {
  protected readonly resource = 'work';

  list(status?: WorkStatus, workType?: WorkType, topLevel?: boolean): Observable<SpWork[]> {
    const params: Record<string, string> = {};
    if (status) params['status'] = status;
    if (workType) params['work_type'] = workType;
    if (topLevel) params['top_level'] = 'true';
    return this.fetchList(params);
  }

  progress(id: string): Observable<SpWorkProgress> {
    return this.http.get<SpWorkProgress>(`${this.baseUrl}/work/${id}/progress`);
  }

  stats(id: string): Observable<SpWorkStats> {
    return this.http.get<SpWorkStats>(`${this.baseUrl}/work/${id}/stats`);
  }

  children(id: string): Observable<SpWork[]> {
    return this.http.get<SpWork[]>(`${this.baseUrl}/work/${id}/children`);
  }

  linkTask(workId: string, taskId: string): Observable<void> {
    return this.http.post<void>(`${this.baseUrl}/work/${workId}/tasks/${taskId}`, {});
  }

  unlinkTask(workId: string, taskId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/work/${workId}/tasks/${taskId}`);
  }

  listTasks(workId: string, params?: { limit?: number; offset?: number }): Observable<SpTask[]> {
    let httpParams = new HttpParams();
    if (params?.limit != null) httpParams = httpParams.set('limit', params.limit);
    if (params?.offset != null) httpParams = httpParams.set('offset', params.offset);
    return this.http.get<SpTask[]>(`${this.baseUrl}/work/${workId}/tasks`, { params: httpParams });
  }

  bulkLinkTasks(workId: string, taskIds: string[]): Observable<void> {
    return this.http.post<void>(`${this.baseUrl}/work/${workId}/tasks/bulk`, { task_ids: taskIds });
  }

  listComments(workId: string): Observable<SpWorkComment[]> {
    return this.http.get<SpWorkComment[]>(`${this.baseUrl}/work/${workId}/comments`);
  }

  createComment(workId: string, content: string): Observable<SpWorkComment> {
    return this.http.post<SpWorkComment>(`${this.baseUrl}/work/${workId}/comments`, { content });
  }

  planTasks(workId: string): Observable<PlannedTask[]> {
    return this.http.post<PlannedTask[]>(`${this.baseUrl}/work/${workId}/plan`, {});
  }

  reorder(workIds: string[]): Observable<SpWork[]> {
    if (!this.projectId) return EMPTY as Observable<SpWork[]>;
    return this.http.post<SpWork[]>(`${this.baseUrl}/${this.projectId}/work/reorder`, { work_ids: workIds });
  }
}

export interface PlannedTask {
  title: string;
  kind: string;
  spec: string;
  acceptance_criteria: string[];
}
