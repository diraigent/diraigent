import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { environment } from '../../../environments/environment';

export function isSpectatorUrl(url: string): boolean {
  return /^\/spectate\/[^/?#]+(?:[/?#]|$)/.test(url);
}

export interface PublicProject {
  id: string;
  name: string;
  description: string | null;
  spectator: boolean;
}
export interface PublicTask {
  id: string;
  number: number;
  title: string;
  kind: string;
  state: string;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
}
export interface PublicTaskDetail extends PublicTask {
  spec: string | null;
  acceptance_criteria: string[];
}
export interface PublicWork {
  id: string;
  title: string;
  description: string | null;
  status: string;
  work_type: string;
  success_criteria: string[];
}
export interface PublicKnowledge {
  id: string;
  title: string;
  category: string;
  content: string;
  tags: string[];
}
export interface PublicDecision {
  id: string;
  title: string;
  status: string;
  context: string;
  decision: string | null;
  rationale: string | null;
}
export type SpectatorArea = 'tasks' | 'work' | 'knowledge' | 'decisions';
export type PublicItem = PublicTask | PublicWork | PublicKnowledge | PublicDecision;
export type PublicDetail = PublicTaskDetail | PublicWork | PublicKnowledge | PublicDecision;
export interface PublicPage {
  data: PublicItem[];
  total: number;
  limit: number;
  offset: number;
  has_more: boolean;
}

/** Only the anonymous publication contract. No authenticated CRUD dependencies. */
@Injectable({ providedIn: 'root' })
export class SpectatorApiService {
  private http = inject(HttpClient);
  readonly pageSize = 20;

  private projectUrl(id: string): string {
    return `${environment.apiServer}/spectator/projects/${encodeURIComponent(id)}`;
  }

  project(id: string) {
    return this.http.get<PublicProject>(this.projectUrl(id), { credentials: 'omit' });
  }

  list(projectId: string, area: SpectatorArea, offset: number) {
    return this.http.get<PublicPage>(`${this.projectUrl(projectId)}/${area}`, {
      credentials: 'omit',
      params: { limit: this.pageSize, offset: Math.max(0, Math.floor(offset)) },
    });
  }

  detail(projectId: string, area: SpectatorArea, id: string) {
    return this.http.get<PublicDetail>(`${this.projectUrl(projectId)}/${area}/${encodeURIComponent(id)}`, {
      credentials: 'omit',
    });
  }
}
