import { Injectable } from '@angular/core';
import { EMPTY, Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';

export type VerificationKind = 'test' | 'acceptance' | 'sign_off';
export type VerificationStatus = 'pass' | 'fail' | 'pending' | 'skipped';

export interface SpVerification {
  id: string;
  project_id: string;
  task_id: string | null;
  agent_id: string | null;
  user_id: string | null;
  kind: VerificationKind;
  status: VerificationStatus;
  title: string;
  detail: string | null;
  evidence: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface SpVerificationCreate {
  task_id?: string;
  kind: VerificationKind;
  status?: VerificationStatus;
  title: string;
  detail?: string;
  evidence?: Record<string, unknown>;
}

export interface SpVerificationUpdate {
  status?: VerificationStatus;
  detail?: string;
  evidence?: Record<string, unknown>;
}

export interface VerificationFilters {
  task_id?: string;
  kind?: VerificationKind;
  status?: VerificationStatus;
  limit?: number;
  offset?: number;
}

export interface PaginatedVerifications {
  data: SpVerification[];
  total: number;
  limit: number;
  offset: number;
  has_more: boolean;
}

@Injectable({ providedIn: 'root' })
export class VerificationsApiService extends BaseApiService {
  private get projectId(): string {
    return localStorage.getItem('diraigent-project') ?? '';
  }

  list(filters?: VerificationFilters): Observable<PaginatedVerifications> {
    if (!this.projectId) return EMPTY;
    const params: Record<string, string> = {};
    if (filters?.task_id) params['task_id'] = filters.task_id;
    if (filters?.kind) params['kind'] = filters.kind;
    if (filters?.status) params['status'] = filters.status;
    if (filters?.limit != null) params['limit'] = String(filters.limit);
    if (filters?.offset != null) params['offset'] = String(filters.offset);
    return this.http.get<PaginatedVerifications>(
      `${this.baseUrl}/${this.projectId}/verifications`, { params }
    );
  }

  get(id: string): Observable<SpVerification> {
    return this.http.get<SpVerification>(`${this.baseUrl}/verifications/${id}`);
  }

  create(data: SpVerificationCreate): Observable<SpVerification> {
    if (!this.projectId) return EMPTY;
    return this.http.post<SpVerification>(`${this.baseUrl}/${this.projectId}/verifications`, data);
  }

  update(id: string, data: SpVerificationUpdate): Observable<SpVerification> {
    return this.http.put<SpVerification>(`${this.baseUrl}/verifications/${id}`, data);
  }
}
