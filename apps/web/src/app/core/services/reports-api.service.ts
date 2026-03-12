import { Injectable } from '@angular/core';
import { EMPTY, Observable } from 'rxjs';
import { map } from 'rxjs/operators';
import { PaginatedResponse } from './tasks-api.service';
import { BaseCrudApiService } from './base-crud-api.service';

export type ReportStatus = 'pending' | 'in_progress' | 'completed' | 'failed';
export type ReportKind = 'security' | 'component' | 'architecture' | 'performance' | 'custom';

export interface SpReport {
  id: string;
  project_id: string;
  title: string;
  kind: string;
  prompt: string | null;
  status: ReportStatus;
  result: string | null;
  task_id: string | null;
  created_by: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface SpReportCreate {
  title: string;
  kind: string;
  prompt: string;
}

export interface SpReportUpdate {
  title?: string;
  status?: string;
  result?: string;
}

@Injectable({ providedIn: 'root' })
export class ReportsApiService extends BaseCrudApiService<SpReport, SpReportCreate, SpReportUpdate> {
  protected readonly resource = 'reports';

  list(status?: ReportStatus): Observable<SpReport[]> {
    if (!this.projectId) return EMPTY;
    const params: Record<string, string> = {};
    if (status) params['status'] = status;
    return this.http.get<PaginatedResponse<SpReport>>(
      `${this.baseUrl}/${this.projectId}/${this.resource}`, { params }
    ).pipe(map(res => res.data));
  }
}
