import { Injectable } from '@angular/core';
import { HttpParams } from '@angular/common/http';
import { Observable } from 'rxjs';
import { BaseApiService } from './base-crud-api.service';

export interface LogEntry {
  timestamp: string;
  line: string;
  labels: Record<string, string>;
}

export interface LogsResponse {
  entries: LogEntry[];
  total: number;
}

export interface LogQuery {
  query: string;
  start?: string;
  end?: string;
  limit?: number;
  direction?: 'forward' | 'backward';
}

export interface LabelsResponse {
  status: string;
  data: string[];
}

@Injectable({ providedIn: 'root' })
export class LogsApiService extends BaseApiService {
  query(q: LogQuery): Observable<LogsResponse> {
    let params = new HttpParams().set('query', q.query);
    if (q.start) params = params.set('start', q.start);
    if (q.end) params = params.set('end', q.end);
    if (q.limit) params = params.set('limit', q.limit.toString());
    if (q.direction) params = params.set('direction', q.direction);
    return this.http.get<LogsResponse>(`${this.baseUrl}/logs`, { params });
  }

  labels(): Observable<LabelsResponse> {
    return this.http.get<LabelsResponse>(`${this.baseUrl}/logs/labels`);
  }

  labelValues(name: string): Observable<LabelsResponse> {
    return this.http.get<LabelsResponse>(`${this.baseUrl}/logs/labels/${name}/values`);
  }
}
