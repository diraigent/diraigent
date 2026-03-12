import { Injectable } from '@angular/core';
import { EMPTY, Observable, map } from 'rxjs';
import { PaginatedResponse } from './tasks-api.service';
import { BaseApiService } from './base-crud-api.service';

export interface SpAuditEntry {
  id: string;
  project_id: string;
  entity_type: string;
  entity_id: string;
  action: 'created' | 'updated' | 'deleted';
  actor_agent_id: string | null;
  actor_user_id: string | null;
  actor_name: string | null;
  summary: string;
  before_state: Record<string, unknown> | null;
  after_state: Record<string, unknown> | null;
  metadata: Record<string, unknown>;
  created_at: string;
}

@Injectable({ providedIn: 'root' })
export class AuditApiService extends BaseApiService {
  private get projectId(): string {
    return localStorage.getItem('diraigent-project') ?? '';
  }

  list(entityType?: string): Observable<SpAuditEntry[]> {
    if (!this.projectId) return EMPTY;
    const params: Record<string, string> = {};
    if (entityType) params['entity_type'] = entityType;
    return this.http.get<PaginatedResponse<SpAuditEntry>>(`${this.baseUrl}/${this.projectId}/audit`, { params })
      .pipe(map(res => res.data));
  }

  entityHistory(entityType: string, entityId: string): Observable<SpAuditEntry[]> {
    return this.http.get<SpAuditEntry[]>(`${this.baseUrl}/audit/${entityType}/${entityId}`);
  }
}
