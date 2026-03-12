import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';

export interface DgPackage {
  id: string;
  slug: string;
  name: string;
  description: string | null;
  is_builtin: boolean;
  allowed_task_kinds: string[];
  allowed_knowledge_categories: string[];
  allowed_observation_kinds: string[];
  allowed_event_kinds: string[];
  allowed_integration_kinds: string[];
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

@Injectable({ providedIn: 'root' })
export class PackagesApiService extends BaseApiService {
  list(): Observable<DgPackage[]> {
    return this.http.get<DgPackage[]>(`${this.baseUrl}/packages`);
  }

  getById(id: string): Observable<DgPackage> {
    return this.http.get<DgPackage>(`${this.baseUrl}/packages/${id}`);
  }
}
