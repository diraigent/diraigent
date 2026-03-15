import { Injectable } from '@angular/core';
import { EMPTY, Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';

export interface ProviderConfig {
  id: string;
  tenant_id: string;
  project_id: string | null;
  provider: string;
  api_key: string | null;
  base_url: string | null;
  default_model: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateProviderConfig {
  provider: string;
  api_key?: string;
  base_url?: string;
  default_model?: string;
}

export interface UpdateProviderConfig {
  provider?: string;
  api_key?: string;
  base_url?: string;
  default_model?: string;
}

export interface ResolvedProviderConfig {
  provider: string;
  api_key: string | null;
  base_url: string | null;
  default_model: string | null;
  api_key_source: string | null;
}

@Injectable({ providedIn: 'root' })
export class ProviderConfigsApiService extends BaseApiService {
  private get projectId(): string {
    return localStorage.getItem('diraigent-project') ?? '';
  }

  listProject(): Observable<ProviderConfig[]> {
    if (!this.projectId) return EMPTY as Observable<ProviderConfig[]>;
    return this.http.get<ProviderConfig[]>(`${this.baseUrl}/${this.projectId}/providers`);
  }

  listGlobal(): Observable<ProviderConfig[]> {
    return this.http.get<ProviderConfig[]>(`${this.baseUrl}/providers`);
  }

  get(id: string): Observable<ProviderConfig> {
    return this.http.get<ProviderConfig>(`${this.baseUrl}/providers/${id}`);
  }

  createProject(data: CreateProviderConfig): Observable<ProviderConfig> {
    if (!this.projectId) return EMPTY as Observable<ProviderConfig>;
    return this.http.post<ProviderConfig>(`${this.baseUrl}/${this.projectId}/providers`, data);
  }

  createGlobal(data: CreateProviderConfig): Observable<ProviderConfig> {
    return this.http.post<ProviderConfig>(`${this.baseUrl}/providers`, data);
  }

  update(id: string, data: UpdateProviderConfig): Observable<ProviderConfig> {
    return this.http.put<ProviderConfig>(`${this.baseUrl}/providers/${id}`, data);
  }

  delete(id: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/providers/${id}`);
  }

  resolve(provider: string): Observable<ResolvedProviderConfig> {
    if (!this.projectId) return EMPTY as Observable<ResolvedProviderConfig>;
    return this.http.get<ResolvedProviderConfig>(`${this.baseUrl}/${this.projectId}/providers/resolve/${provider}`);
  }
}
