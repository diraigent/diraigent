import { Injectable } from '@angular/core';
import { EMPTY, Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';

export interface EventObservationRule {
  id: string;
  project_id: string;
  name: string;
  enabled: boolean;
  event_kind: string | null;
  event_source: string | null;
  severity_gte: string | null;
  observation_kind: string;
  observation_severity: string;
  title_template: string;
  description_template: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateEventObservationRule {
  name: string;
  enabled?: boolean;
  event_kind?: string;
  event_source?: string;
  severity_gte?: string;
  observation_kind?: string;
  observation_severity?: string;
  title_template: string;
  description_template?: string;
}

export interface UpdateEventObservationRule {
  name?: string;
  enabled?: boolean;
  event_kind?: string;
  event_source?: string;
  severity_gte?: string;
  observation_kind?: string;
  observation_severity?: string;
  title_template?: string;
  description_template?: string;
}

@Injectable({ providedIn: 'root' })
export class EventRulesApiService extends BaseApiService {
  private get projectId(): string {
    return localStorage.getItem('diraigent-project') ?? '';
  }

  list(): Observable<EventObservationRule[]> {
    if (!this.projectId) return EMPTY as Observable<EventObservationRule[]>;
    return this.http.get<EventObservationRule[]>(`${this.baseUrl}/${this.projectId}/event-rules`);
  }

  get(id: string): Observable<EventObservationRule> {
    return this.http.get<EventObservationRule>(`${this.baseUrl}/event-rules/${id}`);
  }

  create(data: CreateEventObservationRule): Observable<EventObservationRule> {
    if (!this.projectId) return EMPTY as Observable<EventObservationRule>;
    return this.http.post<EventObservationRule>(`${this.baseUrl}/${this.projectId}/event-rules`, data);
  }

  update(id: string, data: UpdateEventObservationRule): Observable<EventObservationRule> {
    return this.http.put<EventObservationRule>(`${this.baseUrl}/event-rules/${id}`, data);
  }

  delete(id: string): Observable<void> {
    return this.http.delete<void>(`${this.baseUrl}/event-rules/${id}`);
  }

  toggle(id: string): Observable<EventObservationRule> {
    return this.http.post<EventObservationRule>(`${this.baseUrl}/event-rules/${id}/toggle`, {});
  }
}
