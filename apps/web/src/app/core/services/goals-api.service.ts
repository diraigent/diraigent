import { Injectable } from '@angular/core';
import { HttpParams } from '@angular/common/http';
import { Observable } from 'rxjs';
import { BaseCrudApiService } from './base-crud-api.service';
import { SpTask } from './tasks-api.service';

export type GoalStatus = 'active' | 'achieved' | 'paused' | 'abandoned';
export type GoalType = 'epic' | 'feature' | 'milestone' | 'sprint' | 'initiative';

export interface SpGoal {
  id: string;
  project_id: string;
  title: string;
  description: string;
  status: GoalStatus;
  goal_type: GoalType;
  priority: number;
  parent_goal_id: string | null;
  auto_status: boolean;
  success_criteria: string;
  target_date: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
  created_by: string;
  updated_at: string;
}

export interface SpGoalProgress {
  total_tasks: number;
  done_tasks: number;
  percentage: number;
}

export interface SpGoalStats {
  goal_id: string;
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

export interface SpGoalComment {
  id: string;
  goal_id: string;
  agent_id: string | null;
  user_id: string | null;
  content: string;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface SpGoalCreate {
  title: string;
  description: string;
  success_criteria: string;
  target_date?: string | null;
  goal_type?: GoalType;
  priority?: number;
  parent_goal_id?: string | null;
  auto_status?: boolean;
}

export interface GoalTodo {
  id: number;
  text: string;
  done: boolean;
}

export interface SpGoalUpdate {
  title?: string;
  description?: string;
  status?: GoalStatus;
  success_criteria?: string;
  target_date?: string | null;
  goal_type?: GoalType;
  priority?: number;
  parent_goal_id?: string | null;
  auto_status?: boolean;
  metadata?: Record<string, unknown>;
}

@Injectable({ providedIn: 'root' })
export class GoalsApiService extends BaseCrudApiService<SpGoal, SpGoalCreate, SpGoalUpdate> {
  protected readonly resource = 'goals';

  list(status?: GoalStatus, goalType?: GoalType, topLevel?: boolean): Observable<SpGoal[]> {
    const params: Record<string, string> = {};
    if (status) params['status'] = status;
    if (goalType) params['goal_type'] = goalType;
    if (topLevel) params['top_level'] = 'true';
    return this.fetchList(params);
  }

  progress(id: string): Observable<SpGoalProgress> {
    return this.http.get<SpGoalProgress>(`${this.baseUrl}/goals/${id}/progress`);
  }

  stats(id: string): Observable<SpGoalStats> {
    return this.http.get<SpGoalStats>(`${this.baseUrl}/goals/${id}/stats`);
  }

  children(id: string): Observable<SpGoal[]> {
    return this.http.get<SpGoal[]>(`${this.baseUrl}/goals/${id}/children`);
  }

  linkTask(goalId: string, taskId: string): Observable<void> {
    return this.http.post<void>(`${this.baseUrl}/goals/${goalId}/tasks/${taskId}`, {});
  }

  unlinkTask(goalId: string, taskId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/goals/${goalId}/tasks/${taskId}`);
  }

  listTasks(goalId: string, params?: { limit?: number; offset?: number }): Observable<SpTask[]> {
    let httpParams = new HttpParams();
    if (params?.limit != null) httpParams = httpParams.set('limit', params.limit);
    if (params?.offset != null) httpParams = httpParams.set('offset', params.offset);
    return this.http.get<SpTask[]>(`${this.baseUrl}/goals/${goalId}/tasks`, { params: httpParams });
  }

  bulkLinkTasks(goalId: string, taskIds: string[]): Observable<void> {
    return this.http.post<void>(`${this.baseUrl}/goals/${goalId}/tasks/bulk`, { task_ids: taskIds });
  }

  listComments(goalId: string): Observable<SpGoalComment[]> {
    return this.http.get<SpGoalComment[]>(`${this.baseUrl}/goals/${goalId}/comments`);
  }

  createComment(goalId: string, content: string): Observable<SpGoalComment> {
    return this.http.post<SpGoalComment>(`${this.baseUrl}/goals/${goalId}/comments`, { content });
  }
}
