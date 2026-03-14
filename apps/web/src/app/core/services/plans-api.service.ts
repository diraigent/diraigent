import { Injectable } from '@angular/core';
import { HttpParams } from '@angular/common/http';
import { Observable } from 'rxjs';
import { BaseCrudApiService } from './base-crud-api.service';
import { PaginatedResponse, SpTask } from './tasks-api.service';

export type PlanStatus = 'active' | 'completed' | 'cancelled';

export interface SpPlan {
  id: string;
  project_id: string;
  title: string;
  description: string | null;
  status: PlanStatus;
  metadata: Record<string, unknown>;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface SpPlanCreate {
  title: string;
  description?: string | null;
  metadata?: Record<string, unknown>;
}

export interface SpPlanUpdate {
  title?: string;
  description?: string | null;
  status?: PlanStatus;
  metadata?: Record<string, unknown>;
}

export interface SpPlanProgress {
  plan_id: string;
  total_tasks: number;
  done_tasks: number;
  cancelled_tasks: number;
  working_tasks: number;
}

@Injectable({ providedIn: 'root' })
export class PlansApiService extends BaseCrudApiService<SpPlan, SpPlanCreate, SpPlanUpdate> {
  protected readonly resource = 'plans';

  list(status?: PlanStatus): Observable<PaginatedResponse<SpPlan>> {
    let params = new HttpParams();
    if (status) params = params.set('status', status);
    if (!this.projectId) return new Observable<PaginatedResponse<SpPlan>>();
    return this.http.get<PaginatedResponse<SpPlan>>(
      `${this.baseUrl}/${this.projectId}/${this.resource}`,
      { params },
    );
  }

  progress(planId: string): Observable<SpPlanProgress> {
    return this.http.get<SpPlanProgress>(`${this.baseUrl}/plans/${planId}/progress`);
  }

  listTasks(planId: string, params?: { limit?: number; offset?: number }): Observable<PaginatedResponse<SpTask>> {
    let httpParams = new HttpParams();
    if (params?.limit != null) httpParams = httpParams.set('limit', params.limit);
    if (params?.offset != null) httpParams = httpParams.set('offset', params.offset);
    return this.http.get<PaginatedResponse<SpTask>>(
      `${this.baseUrl}/plans/${planId}/tasks`,
      { params: httpParams },
    );
  }

  addTask(planId: string, taskId: string): Observable<SpTask> {
    return this.http.post<SpTask>(`${this.baseUrl}/plans/${planId}/tasks`, { task_id: taskId });
  }

  removeTask(planId: string, taskId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/plans/${planId}/tasks/${taskId}`);
  }

  reorderTasks(planId: string, taskIds: string[]): Observable<SpTask[]> {
    return this.http.post<SpTask[]>(`${this.baseUrl}/plans/${planId}/tasks/reorder`, { task_ids: taskIds });
  }
}
