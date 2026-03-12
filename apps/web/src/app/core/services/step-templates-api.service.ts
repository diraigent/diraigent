import { Injectable } from '@angular/core';
import { EMPTY, Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';

// ── Interfaces ──

export interface SpStepTemplate {
  id: string;
  /** null means this is a global template (immutable). */
  tenant_id: string | null;
  name: string;
  description: string | null;
  model: string | null;
  budget: number | null;
  allowed_tools: string | null;
  context_level: string | null;
  on_complete: string | null;
  retriable: boolean | null;
  max_cycles: number | null;
  timeout_minutes: number | null;
  mcp_servers: Record<string, unknown> | null;
  agents: Record<string, unknown> | null;
  agent: string | null;
  settings: Record<string, unknown> | null;
  env: Record<string, string> | null;
  vars: Record<string, string> | null;
  tags: string[];
  metadata: Record<string, unknown>;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface SpCreateStepTemplate {
  name: string;
  description?: string;
  model?: string;
  budget?: number;
  allowed_tools?: string;
  context_level?: string;
  on_complete?: string;
  retriable?: boolean;
  max_cycles?: number;
  timeout_minutes?: number;
  mcp_servers?: Record<string, unknown>;
  agents?: Record<string, unknown>;
  agent?: string;
  settings?: Record<string, unknown>;
  env?: Record<string, string>;
  vars?: Record<string, string>;
  tags?: string[];
  metadata?: Record<string, unknown>;
}

export interface SpUpdateStepTemplate {
  name?: string;
  description?: string;
  model?: string;
  budget?: number;
  allowed_tools?: string;
  context_level?: string;
  on_complete?: string;
  retriable?: boolean;
  max_cycles?: number;
  timeout_minutes?: number;
  mcp_servers?: Record<string, unknown>;
  agents?: Record<string, unknown>;
  agent?: string;
  settings?: Record<string, unknown>;
  env?: Record<string, string>;
  vars?: Record<string, string>;
  tags?: string[];
  metadata?: Record<string, unknown>;
}

// ── Service ──

@Injectable({ providedIn: 'root' })
export class StepTemplatesApiService extends BaseApiService {
  private get projectId(): string {
    return localStorage.getItem('diraigent-project') ?? '';
  }

  list(): Observable<SpStepTemplate[]> {
    if (!this.projectId) return EMPTY as Observable<SpStepTemplate[]>;
    return this.http.get<SpStepTemplate[]>(`${this.baseUrl}/${this.projectId}/step-templates`);
  }

  get(id: string): Observable<SpStepTemplate> {
    if (!this.projectId) return EMPTY as Observable<SpStepTemplate>;
    return this.http.get<SpStepTemplate>(`${this.baseUrl}/${this.projectId}/step-templates/${id}`);
  }

  create(data: SpCreateStepTemplate): Observable<SpStepTemplate> {
    if (!this.projectId) return EMPTY as Observable<SpStepTemplate>;
    return this.http.post<SpStepTemplate>(`${this.baseUrl}/${this.projectId}/step-templates`, data);
  }

  update(id: string, data: SpUpdateStepTemplate): Observable<SpStepTemplate> {
    if (!this.projectId) return EMPTY as Observable<SpStepTemplate>;
    return this.http.put<SpStepTemplate>(`${this.baseUrl}/${this.projectId}/step-templates/${id}`, data);
  }

  delete(id: string): Observable<void> {
    if (!this.projectId) return EMPTY as Observable<void>;
    return this.http.delete<void>(`${this.baseUrl}/${this.projectId}/step-templates/${id}`);
  }

  fork(id: string): Observable<SpStepTemplate> {
    if (!this.projectId) return EMPTY as Observable<SpStepTemplate>;
    return this.http.post<SpStepTemplate>(`${this.baseUrl}/${this.projectId}/step-templates/${id}/fork`, {});
  }
}
