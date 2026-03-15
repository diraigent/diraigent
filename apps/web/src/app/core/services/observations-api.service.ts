import { Injectable } from '@angular/core';
import { EMPTY, Observable } from 'rxjs';
import { map } from 'rxjs/operators';
import { PaginatedResponse } from './tasks-api.service';
import { BaseCrudApiService } from './base-crud-api.service';

export type ObservationKind = 'insight' | 'risk' | 'opportunity' | 'smell' | 'inconsistency' | 'improvement';
export type ObservationSeverity = 'critical' | 'high' | 'medium' | 'low' | 'info';
export type ObservationStatus = 'open' | 'acknowledged' | 'acted_on' | 'dismissed';

export interface SpObservation {
  id: string;
  project_id: string;
  agent_id: string | null;
  kind: ObservationKind;
  title: string;
  description: string | null;
  severity: ObservationSeverity;
  status: ObservationStatus;
  evidence: Record<string, unknown>;
  source: string | null;
  source_task_id: string | null;
  metadata: Record<string, unknown>;
  resolved_task_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface SpObservationCreate {
  kind: ObservationKind;
  title: string;
  description?: string;
  severity: ObservationSeverity;
  evidence?: Record<string, unknown>;
}

export interface SpObservationUpdate {
  title?: string;
  description?: string;
  severity?: ObservationSeverity;
  status?: ObservationStatus;
}

@Injectable({ providedIn: 'root' })
export class ObservationsApiService extends BaseCrudApiService<SpObservation, SpObservationCreate, SpObservationUpdate> {
  protected readonly resource = 'observations';

  list(status?: ObservationStatus, kind?: ObservationKind): Observable<SpObservation[]> {
    if (!this.projectId) return EMPTY;
    const params: Record<string, string> = {};
    if (status) params['status'] = status;
    if (kind) params['kind'] = kind;
    return this.http.get<PaginatedResponse<SpObservation>>(
      `${this.baseUrl}/${this.projectId}/${this.resource}`, { params }
    ).pipe(map(res => res.data));
  }

  dismiss(id: string): Observable<SpObservation> {
    return this.http.post<SpObservation>(`${this.baseUrl}/observations/${id}/dismiss`, {});
  }

  promote(id: string): Observable<{ observation: SpObservation; work: { id: string }; task: { id: string } }> {
    return this.http.post<{ observation: SpObservation; work: { id: string }; task: { id: string } }>(`${this.baseUrl}/observations/${id}/promote`, {});
  }

  cleanup(): Observable<CleanupObservationsResult> {
    if (!this.projectId) return EMPTY as Observable<CleanupObservationsResult>;
    return this.http.post<CleanupObservationsResult>(
      `${this.baseUrl}/${this.projectId}/${this.resource}/cleanup`, {}
    );
  }
}

export interface CleanupObservationsResult {
  deleted_dismissed: number;
  deleted_acknowledged: number;
  deleted_acted_on: number;
  deleted_resolved: number;
  deleted_duplicates: number;
  total_deleted: number;
}
