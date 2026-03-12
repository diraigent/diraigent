import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { EMPTY, Observable } from 'rxjs';
import { environment } from '../../../environments/environment';

export interface BranchInfo {
  name: string;
  commit: string;
  is_pushed: boolean;
  ahead_remote: number;
  behind_remote: number;
  task_id_prefix: string | null;
}

export interface BranchListResponse {
  current_branch: string;
  branches: BranchInfo[];
}

export interface TaskBranchStatus {
  branch: string;
  exists: boolean;
  is_pushed: boolean;
  ahead_remote: number;
  behind_remote: number;
  last_commit: string | null;
  last_commit_message: string | null;
  behind_default: number;
  has_conflict: boolean;
}

export interface PushResponse {
  success: boolean;
  message: string;
}

export interface MainPushStatus {
  ahead: number;
  behind: number;
  last_commit: string | null;
  last_commit_message: string | null;
}

@Injectable({ providedIn: 'root' })
export class GitApiService {
  private http = inject(HttpClient);
  private baseUrl = environment.apiServer;

  private get projectId(): string {
    return localStorage.getItem('diraigent-project') ?? '';
  }

  listBranches(prefix?: string): Observable<BranchListResponse> {
    if (!this.projectId) return EMPTY;
    const params: Record<string, string> = {};
    if (prefix) params['prefix'] = prefix;
    return this.http.get<BranchListResponse>(
      `${this.baseUrl}/${this.projectId}/git/branches`,
      { params },
    );
  }

  taskBranchStatus(taskId: string): Observable<TaskBranchStatus> {
    if (!this.projectId) return EMPTY;
    return this.http.get<TaskBranchStatus>(
      `${this.baseUrl}/${this.projectId}/git/task-branch/${taskId}`,
    );
  }

  pushBranch(branch: string, remote?: string): Observable<PushResponse> {
    if (!this.projectId) return EMPTY;
    return this.http.post<PushResponse>(
      `${this.baseUrl}/${this.projectId}/git/push`,
      { branch, remote: remote ?? null },
    );
  }

  mainStatus(): Observable<MainPushStatus> {
    if (!this.projectId) return EMPTY;
    return this.http.get<MainPushStatus>(
      `${this.baseUrl}/${this.projectId}/git/main-status`,
    );
  }

  pushMain(): Observable<PushResponse> {
    if (!this.projectId) return EMPTY;
    return this.http.post<PushResponse>(
      `${this.baseUrl}/${this.projectId}/git/push-main`,
      {},
    );
  }

  resolveAndPushMain(): Observable<PushResponse> {
    if (!this.projectId) return EMPTY;
    return this.http.post<PushResponse>(
      `${this.baseUrl}/${this.projectId}/git/resolve-and-push-main`,
      {},
    );
  }

  revertTask(taskId: string): Observable<PushResponse> {
    if (!this.projectId) return EMPTY;
    return this.http.post<PushResponse>(
      `${this.baseUrl}/${this.projectId}/git/revert-task/${taskId}`,
      {},
    );
  }

  resolveTaskBranch(taskId: string): Observable<PushResponse> {
    if (!this.projectId) return EMPTY;
    return this.http.post<PushResponse>(
      `${this.baseUrl}/${this.projectId}/git/resolve-task-branch/${taskId}`,
      {},
    );
  }
}
