import { inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { EMPTY, Observable } from 'rxjs';
import { environment } from '../../../environments/environment';

/**
 * Minimal base providing `http` and `baseUrl` to all API services.
 * Extend this for services that don't follow the standard project-scoped CRUD pattern.
 */
export abstract class BaseApiService {
  protected http = inject(HttpClient);
  protected baseUrl = environment.apiServer;
}

/**
 * Generic base class for project-scoped CRUD API services.
 *
 * URL conventions:
 *  - list/create:   {apiServer}/{projectId}/{resource}
 *  - get/update/delete: {apiServer}/{resource}/{id}
 *
 * Subclasses must declare `readonly resource` (e.g. 'work').
 * Each subclass defines its own `list()` with typed params; call `fetchList(params)` inside.
 */
export abstract class BaseCrudApiService<T, C, U> extends BaseApiService {
  protected abstract readonly resource: string;

  protected get projectId(): string {
    return localStorage.getItem('diraigent-project') ?? '';
  }

  /** Call from subclass `list()` to delegate to the standard project-scoped endpoint. */
  protected fetchList(params?: Record<string, string>): Observable<T[]> {
    if (!this.projectId) return EMPTY as Observable<T[]>;
    return this.http.get<T[]>(`${this.baseUrl}/${this.projectId}/${this.resource}`, { params });
  }

  get(id: string): Observable<T> {
    return this.http.get<T>(`${this.baseUrl}/${this.resource}/${id}`);
  }

  create(data: C): Observable<T> {
    if (!this.projectId) return EMPTY as Observable<T>;
    return this.http.post<T>(`${this.baseUrl}/${this.projectId}/${this.resource}`, data);
  }

  update(id: string, data: U): Observable<T> {
    return this.http.put<T>(`${this.baseUrl}/${this.resource}/${id}`, data);
  }

  delete(id: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/${this.resource}/${id}`);
  }
}
