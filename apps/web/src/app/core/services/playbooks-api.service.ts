import { Injectable } from '@angular/core';
import { Observable, map } from 'rxjs';
import { STORAGE_KEYS } from '../../shared/ui-constants';
import { BaseApiService } from './base-crud-api.service';

export interface SpPlaybookStep {
  name: string;
  description: string;
  on_complete: string;
  step: number;
  step_template_id?: string;
  model?: string;
  budget?: number;
  allowed_tools?: string;
  context_level?: string;
  max_cycles?: number;
  retriable?: boolean;
  timeout_minutes?: number;
  mcp_servers?: Record<string, unknown>;
  agents?: Record<string, unknown>;
  agent?: string;
  settings?: Record<string, unknown>;
  env?: Record<string, string>;
  git_action?: 'none' | 'merge' | 'push';
  provider?: string;
  base_url?: string;
}

export interface SpPlaybook {
  id: string;
  /** null means this is a shared default playbook (immutable). */
  tenant_id: string | null;
  title: string;
  trigger_description: string;
  steps: SpPlaybookStep[];
  tags: string[];
  initial_state: 'ready' | 'backlog';
  metadata: Record<string, unknown>;
  created_at: string;
  created_by: string;
  updated_at: string;
}

export type GitStrategyId = 'merge_to_default' | 'branch_only' | 'branch_to_target' | 'feature_branch' | 'no_git';

export interface GitStrategyDef {
  id: GitStrategyId;
  name: string;
  description: string;
  fields?: Record<string, string>;
}

export interface SpPlaybookCreate {
  title: string;
  trigger_description: string;
  steps: SpPlaybookStep[];
  tags: string[];
  initial_state?: 'ready' | 'backlog';
  metadata?: Record<string, unknown>;
}

export interface SpPlaybookUpdate {
  title?: string;
  trigger_description?: string;
  steps?: SpPlaybookStep[];
  tags?: string[];
  initial_state?: 'ready' | 'backlog';
  metadata?: Record<string, unknown>;
}

@Injectable({ providedIn: 'root' })
export class PlaybooksApiService extends BaseApiService {
  private get playbooksUrl(): string {
    return `${this.baseUrl}/projects/${localStorage.getItem(STORAGE_KEYS.PROJECT) ?? ''}/playbooks`;
  }

  private normalize(book: SpPlaybook & { name?: string }): SpPlaybook {
    return { ...book, id: book.name ?? book.id };
  }
  list(): Observable<SpPlaybook[]> {
    return this.http.get<SpPlaybook[]>(this.playbooksUrl).pipe(map(books => books.map(book => this.normalize(book))));
  }

  get(id: string): Observable<SpPlaybook> {
    return this.http.get<SpPlaybook>(`${this.playbooksUrl}/${encodeURIComponent(id)}`).pipe(map(book => this.normalize(book)));
  }

  create(data: SpPlaybookCreate): Observable<SpPlaybook> {
    return this.http.post<SpPlaybook>(this.playbooksUrl, data).pipe(map(book => this.normalize(book)));
  }

  update(id: string, data: SpPlaybookUpdate): Observable<SpPlaybook> {
    return this.http.put<SpPlaybook>(`${this.playbooksUrl}/${encodeURIComponent(id)}`, data).pipe(map(book => this.normalize(book)));
  }

  delete(id: string): Observable<void> {
    return this.http.delete<void>(`${this.playbooksUrl}/${encodeURIComponent(id)}`);
  }

  getGitStrategies(): Observable<GitStrategyDef[]> {
    return this.http.get<GitStrategyDef[]>(`${this.baseUrl}/git-strategies`);
  }
}
