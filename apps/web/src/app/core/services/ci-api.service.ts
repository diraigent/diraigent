import { Injectable } from '@angular/core';
import { HttpParams } from '@angular/common/http';
import { Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';

export type CiRunStatus = 'success' | 'failure' | 'running' | 'pending' | 'skipped' | 'cancelled';

export interface CiRun {
  id: string;
  project_id: string;
  external_id: number;
  provider: string;
  workflow_name: string;
  status: string;
  branch: string | null;
  commit_sha: string | null;
  triggered_by: string | null;
  started_at: string | null;
  finished_at: string | null;
  created_at: string;
}

export interface CiJob {
  id: string;
  ci_run_id: string;
  name: string;
  status: string;
  runner: string | null;
  started_at: string | null;
  finished_at: string | null;
}

export interface CiStep {
  id: string;
  ci_job_id: string;
  name: string;
  status: string;
  exit_code: number | null;
  started_at: string | null;
  finished_at: string | null;
}

export interface CiRunWithJobs extends CiRun {
  jobs: CiJob[];
}

export interface CiJobWithSteps extends CiJob {
  steps: CiStep[];
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  limit: number;
  offset: number;
  has_more: boolean;
}

export interface CiRunFilters {
  branch?: string;
  status?: string;
  workflow_name?: string;
  provider?: string;
  page?: number;
  per_page?: number;
}

export interface ForgejoIntegrationResponse {
  id: string;
  project_id: string;
  base_url: string;
  webhook_url: string;
  webhook_secret: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateForgejoIntegration {
  base_url: string;
  token?: string;
}

export interface SyncForgejoResponse {
  synced: number;
  errors: number;
}

export interface GitHubIntegrationResponse {
  id: string;
  project_id: string;
  base_url: string;
  webhook_url: string;
  webhook_secret: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateGitHubIntegration {
  base_url?: string;
  token?: string;
}

export interface SyncGitHubResponse {
  synced: number;
  errors: number;
}

@Injectable({ providedIn: 'root' })
export class CiApiService extends BaseApiService {
  registerForgejo(projectId: string, req: CreateForgejoIntegration): Observable<ForgejoIntegrationResponse> {
    return this.http.post<ForgejoIntegrationResponse>(
      `${this.baseUrl}/${projectId}/integrations/forgejo`,
      req,
    );
  }

  syncForgejo(projectId: string): Observable<SyncForgejoResponse> {
    return this.http.post<SyncForgejoResponse>(
      `${this.baseUrl}/${projectId}/forgejo/sync`,
      {},
    );
  }

  registerGitHub(projectId: string, req: CreateGitHubIntegration): Observable<GitHubIntegrationResponse> {
    return this.http.post<GitHubIntegrationResponse>(
      `${this.baseUrl}/${projectId}/integrations/github`,
      req,
    );
  }

  syncGitHub(projectId: string): Observable<SyncGitHubResponse> {
    return this.http.post<SyncGitHubResponse>(
      `${this.baseUrl}/${projectId}/github/sync`,
      {},
    );
  }

  listRuns(projectId: string, filters?: CiRunFilters): Observable<PaginatedResponse<CiRun>> {
    let params = new HttpParams();
    if (filters?.branch) params = params.set('branch', filters.branch);
    if (filters?.status) params = params.set('status', filters.status);
    if (filters?.workflow_name) params = params.set('workflow_name', filters.workflow_name);
    if (filters?.provider) params = params.set('provider', filters.provider);
    if (filters?.page) params = params.set('page', filters.page.toString());
    if (filters?.per_page) params = params.set('per_page', filters.per_page.toString());

    return this.http.get<PaginatedResponse<CiRun>>(`${this.baseUrl}/${projectId}/ci/runs`, { params });
  }

  getRun(projectId: string, runId: string): Observable<CiRunWithJobs> {
    return this.http.get<CiRunWithJobs>(`${this.baseUrl}/${projectId}/ci/runs/${runId}`);
  }

  getJob(projectId: string, runId: string, jobId: string): Observable<CiJobWithSteps> {
    return this.http.get<CiJobWithSteps>(`${this.baseUrl}/${projectId}/ci/runs/${runId}/jobs/${jobId}`);
  }
}
