import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';

export type IntegrationKind =
  | 'logging'
  | 'tracing'
  | 'metrics'
  | 'git'
  | 'ci'
  | 'messaging'
  | 'monitoring'
  | 'storage'
  | 'database'
  | 'custom';

export type AuthType = 'none' | 'token' | 'basic' | 'api_key' | 'oauth2';

export interface Integration {
  id: string;
  project_id: string;
  name: string;
  kind: IntegrationKind;
  provider: string;
  base_url: string;
  auth_type: AuthType;
  enabled: boolean;
  capabilities: string[];
  config: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface CreateIntegrationRequest {
  name: string;
  kind: IntegrationKind;
  provider: string;
  base_url: string;
  auth_type: AuthType;
  credentials?: Record<string, string>;
  config?: Record<string, unknown>;
  capabilities?: string[];
}

export interface UpdateIntegrationRequest {
  name?: string;
  kind?: IntegrationKind;
  provider?: string;
  base_url?: string;
  auth_type?: AuthType;
  credentials?: Record<string, string>;
  config?: Record<string, unknown>;
  capabilities?: string[];
  enabled?: boolean;
}

export interface IntegrationAccess {
  agent_id: string;
  agent_name?: string;
  granted_at: string;
}

export interface GrantAccessRequest {
  agent_id: string;
}

@Injectable({ providedIn: 'root' })
export class IntegrationsApiService extends BaseApiService {
  list(projectId: string): Observable<Integration[]> {
    return this.http.get<Integration[]>(`${this.baseUrl}/${projectId}/integrations`);
  }

  create(projectId: string, req: CreateIntegrationRequest): Observable<Integration> {
    return this.http.post<Integration>(`${this.baseUrl}/${projectId}/integrations`, req);
  }

  get(id: string): Observable<Integration> {
    return this.http.get<Integration>(`${this.baseUrl}/integrations/${id}`);
  }

  update(id: string, req: UpdateIntegrationRequest): Observable<Integration> {
    return this.http.put<Integration>(`${this.baseUrl}/integrations/${id}`, req);
  }

  delete(id: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/integrations/${id}`);
  }

  listAccess(integrationId: string): Observable<IntegrationAccess[]> {
    return this.http.get<IntegrationAccess[]>(`${this.baseUrl}/integrations/${integrationId}/access`);
  }

  grantAccess(integrationId: string, req: GrantAccessRequest): Observable<IntegrationAccess> {
    return this.http.post<IntegrationAccess>(`${this.baseUrl}/integrations/${integrationId}/access`, req);
  }

  revokeAccess(integrationId: string, agentId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/integrations/${integrationId}/access/${agentId}`);
  }

  listAgentIntegrations(agentId: string): Observable<Integration[]> {
    return this.http.get<Integration[]>(`${this.baseUrl}/agents/${agentId}/integrations`);
  }
}
