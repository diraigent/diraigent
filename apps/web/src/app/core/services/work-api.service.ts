import { Injectable, inject } from '@angular/core';
import { HttpParams } from '@angular/common/http';
import { EMPTY, Observable } from 'rxjs';
import { BaseCrudApiService } from './base-crud-api.service';
import { AuthService } from './auth.service';
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
  private auth = inject(AuthService);

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

  /**
   * Stream plan tasks via SSE. Returns an Observable that emits:
   * - PlanSseStatus events (for progress display)
   * - PlanSseDone event (final result with tasks)
   * - PlanSseError event (on failure)
   *
   * The Observable completes after the done/error event.
   */
  planTasksStream(workId: string): Observable<PlanSseEvent> {
    if (!this.projectId) return EMPTY as Observable<PlanSseEvent>;
    const url = `${this.baseUrl}/${this.projectId}/work/${workId}/plan`;

    return new Observable<PlanSseEvent>(subscriber => {
      const abortController = new AbortController();

      const run = async () => {
        const headers: Record<string, string> = { 'Content-Type': 'application/json' };
        const token = this.auth.getAccessToken();
        if (token) headers['Authorization'] = `Bearer ${token}`;

        const resp = await fetch(url, {
          method: 'POST',
          headers,
          body: '{}',
          signal: abortController.signal,
        });

        if (!resp.ok) {
          const body = await resp.text();
          throw new Error(`HTTP ${resp.status}: ${body}`);
        }

        const reader = resp.body?.getReader();
        if (!reader) throw new Error('No response body');

        const decoder = new TextDecoder();
        let buffer = '';

        const processEvent = (part: string) => {
          const lines = part.split('\n');
          const eventLine = lines.find(l => l.startsWith('event: '));
          const dataLines = lines.filter(l => l.startsWith('data: '));
          if (dataLines.length === 0) return;

          const eventType = eventLine?.slice(7) ?? '';
          const rawData = dataLines.map(l => l.slice(6)).join('\n');

          let data: Record<string, unknown>;
          try { data = JSON.parse(rawData); } catch { return; }

          switch (eventType) {
            case 'status':
              subscriber.next({ type: 'status', message: data['message'] as string });
              break;
            case 'done':
              subscriber.next({
                type: 'done',
                tasks: data['tasks'] as PlannedTask[],
                success_criteria: data['success_criteria'] as string[] | undefined,
              });
              subscriber.complete();
              break;
            case 'error':
              subscriber.error(new Error(data['message'] as string));
              break;
          }
        };

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });
          const parts = buffer.split('\n\n');
          buffer = parts.pop() ?? '';
          for (const part of parts) {
            if (part.trim()) processEvent(part);
          }
        }
        if (buffer.trim()) processEvent(buffer);
      };

      run().catch(err => {
        if (err instanceof DOMException && err.name === 'AbortError') return;
        subscriber.error(err);
      });

      return () => abortController.abort();
    });
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
  depends_on?: number[];
}

export interface PlanWorkResponse {
  tasks: PlannedTask[];
  success_criteria?: string[];
}

export type PlanSseEvent =
  | { type: 'status'; message: string }
  | { type: 'done'; tasks: PlannedTask[]; success_criteria?: string[] }
  | { type: 'error'; message: string };
