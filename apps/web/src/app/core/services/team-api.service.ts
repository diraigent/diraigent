import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';

export interface SpRole {
  id: string;
  name: string;
  description: string | null;
  authorities: string[];
  required_capabilities: string[];
  knowledge_scope: string[];
  created_at: string;
  updated_at: string;
}

export interface SpMember {
  id: string;
  agent_id: string;
  role_id: string;
  status: string;
  joined_at: string;
  updated_at: string;
  config: Record<string, unknown>;
}

export interface SpRoleCreate {
  name: string;
  description?: string;
  authorities: string[];
  required_capabilities?: string[];
  knowledge_scope?: string[];
}

export interface SpMemberCreate {
  agent_id: string;
  role_id: string;
}

@Injectable({ providedIn: 'root' })
export class TeamApiService extends BaseApiService {
  getRoles(): Observable<SpRole[]> {
    return this.http.get<SpRole[]>(`${this.baseUrl}/roles`);
  }

  createRole(role: SpRoleCreate): Observable<SpRole> {
    return this.http.post<SpRole>(`${this.baseUrl}/roles`, role);
  }

  updateRole(roleId: string, role: SpRoleCreate): Observable<SpRole> {
    return this.http.put<SpRole>(`${this.baseUrl}/roles/${roleId}`, role);
  }

  deleteRole(roleId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/roles/${roleId}`);
  }

  getMembers(): Observable<SpMember[]> {
    return this.http.get<SpMember[]>(`${this.baseUrl}/members`);
  }

  createMember(member: SpMemberCreate): Observable<SpMember> {
    return this.http.post<SpMember>(`${this.baseUrl}/members`, member);
  }

  updateMember(memberId: string, update: { role_id: string }): Observable<SpMember> {
    return this.http.put<SpMember>(`${this.baseUrl}/members/${memberId}`, update);
  }

  deleteMember(memberId: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/members/${memberId}`);
  }
}
