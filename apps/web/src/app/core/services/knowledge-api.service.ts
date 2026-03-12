import { Injectable } from '@angular/core';
import { EMPTY, Observable } from 'rxjs';
import { map } from 'rxjs';
import { BaseCrudApiService } from './base-crud-api.service';

export type KnowledgeCategory = 'architecture' | 'convention' | 'pattern' | 'anti_pattern' | 'setup' | 'general';

export interface SpKnowledge {
  id: string;
  project_id: string;
  title: string;
  category: KnowledgeCategory;
  content: string;
  tags: string[];
  metadata: Record<string, unknown>;
  created_at: string;
  created_by: string;
  updated_at: string;
}

export interface SpKnowledgeCreate {
  title: string;
  category: KnowledgeCategory;
  content: string;
  tags: string[];
}

export interface SpKnowledgeUpdate {
  title?: string;
  category?: KnowledgeCategory;
  content?: string;
  tags?: string[];
}

@Injectable({ providedIn: 'root' })
export class KnowledgeApiService extends BaseCrudApiService<SpKnowledge, SpKnowledgeCreate, SpKnowledgeUpdate> {
  protected readonly resource = 'knowledge';

  list(category?: KnowledgeCategory, tag?: string): Observable<SpKnowledge[]> {
    if (!this.projectId) return EMPTY;
    const params: Record<string, string> = {};
    if (category) params['category'] = category;
    if (tag) params['tag'] = tag;
    return this.http.get<{ data: SpKnowledge[] }>(
      `${this.baseUrl}/${this.projectId}/${this.resource}`, { params }
    ).pipe(map(res => res.data));
  }
}
