import { Injectable } from '@angular/core';
import { EMPTY, Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';
import { environment } from '../../../environments/environment';

export interface SpScratchpadTodo {
  id: string;
  text: string;
  done: boolean;
  createdAt: string;
  taskId?: string;
}

export interface SpScratchpad {
  id: string;
  user_id: string;
  project_id: string;
  notes: string;
  todos: SpScratchpadTodo[];
  updated_at: string;
}

export interface UpsertScratchpad {
  notes: string;
  todos: SpScratchpadTodo[];
}

@Injectable({ providedIn: 'root' })
export class ScratchpadApiService extends BaseApiService {
  private get projectId(): string {
    return localStorage.getItem('diraigent-project') ?? '';
  }

  get(): Observable<SpScratchpad | null> {
    if (!this.projectId) return EMPTY;
    return this.http.get<SpScratchpad | null>(
      `${environment.apiServer}/${this.projectId}/scratchpad`
    );
  }

  upsert(data: UpsertScratchpad): Observable<SpScratchpad> {
    if (!this.projectId) return EMPTY;
    return this.http.put<SpScratchpad>(
      `${environment.apiServer}/${this.projectId}/scratchpad`,
      data
    );
  }
}
