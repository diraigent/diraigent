import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';

export interface SpAgent {
  id: string;
  name: string;
  status: 'idle' | 'working' | 'offline' | 'revoked';
  owner_id: string | null;
  capabilities: string[];
  metadata: Record<string, unknown>;
  last_seen_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface SpAgentRegistered extends SpAgent {
  api_key: string;
}

export interface CreateAgentRequest {
  name: string;
  capabilities?: string[];
  metadata?: Record<string, unknown>;
}

export interface SpAgentTask {
  id: string;
  title: string;
  state: string;
  kind: string;
  priority: number;
  created_at: string;
  updated_at: string;
}

@Injectable({ providedIn: 'root' })
export class AgentsApiService extends BaseApiService {
  getAgents(): Observable<SpAgent[]> {
    return this.http.get<SpAgent[]>(`${this.baseUrl}/agents`);
  }

  getAgent(id: string): Observable<SpAgent> {
    return this.http.get<SpAgent>(`${this.baseUrl}/agents/${id}`);
  }

  createAgent(req: CreateAgentRequest): Observable<SpAgentRegistered> {
    return this.http.post<SpAgentRegistered>(`${this.baseUrl}/agents`, req);
  }

  updateAgent(agentId: string, body: Partial<{ name: string; status: string; capabilities: string[] }>): Observable<SpAgent> {
    return this.http.put<SpAgent>(`${this.baseUrl}/agents/${agentId}`, body);
  }

  getAgentTasks(agentId: string): Observable<SpAgentTask[]> {
    return this.http.get<SpAgentTask[]>(`${this.baseUrl}/agents/${agentId}/tasks`);
  }
}
