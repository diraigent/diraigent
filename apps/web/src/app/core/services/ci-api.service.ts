import { Injectable } from '@angular/core';
import { HttpParams } from '@angular/common/http';
import { Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';

export type CiRunStatus = 'success' | 'failure' | 'running' | 'pending' | 'skipped' | 'cancelled';

export interface CiRun {
  id: string;
  project_id: string;
  forgejo_run_id: number;
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
  page?: number;
  per_page?: number;
}

@Injectable({ providedIn: 'root' })
export class CiApiService extends BaseApiService {
  listRuns(projectId: string, filters?: CiRunFilters): Observable<PaginatedResponse<CiRun>> {
    let params = new HttpParams();
    if (filters?.branch) params = params.set('branch', filters.branch);
    if (filters?.status) params = params.set('status', filters.status);
    if (filters?.workflow_name) params = params.set('workflow_name', filters.workflow_name);
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
