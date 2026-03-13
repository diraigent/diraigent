import { Injectable } from '@angular/core';
import { EMPTY, Observable } from 'rxjs';
import { map } from 'rxjs/operators';
import { PaginatedResponse } from './tasks-api.service';
import { BaseCrudApiService } from './base-crud-api.service';

export interface SpTaskSummaryForDecision {
  id: string;
  number: number;
  title: string;
  kind: string;
  state: string;
  urgent: boolean;
  created_at: string;
}

export type DecisionStatus = 'proposed' | 'accepted' | 'rejected' | 'superseded' | 'deprecated';

export interface SpDecisionAlternative {
  name: string;
  pros: string;
  cons: string;
}

export interface SpDecision {
  id: string;
  project_id: string;
  title: string;
  status: DecisionStatus;
  context: string;
  decision: string | null;
  rationale: string | null;
  // alternatives may be a structured array (from web UI) or a plain string (from agent-cli)
  alternatives: SpDecisionAlternative[] | string | null;
  consequences: string | null;
  superseded_by: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
  created_by: string;
  updated_at: string;
}

export interface SpDecisionCreate {
  title: string;
  context: string;
  decision: string;
  rationale: string;
  alternatives: SpDecisionAlternative[];
  consequences?: string;
}

export interface SpDecisionUpdate {
  title?: string;
  status?: DecisionStatus;
  context?: string;
  decision?: string;
  rationale?: string;
  alternatives?: SpDecisionAlternative[];
  consequences?: string;
  superseded_by?: string | null;
}

@Injectable({ providedIn: 'root' })
export class DecisionsApiService extends BaseCrudApiService<SpDecision, SpDecisionCreate, SpDecisionUpdate> {
  protected readonly resource = 'decisions';

  list(status?: DecisionStatus): Observable<SpDecision[]> {
    if (!this.projectId) return EMPTY;
    const params: Record<string, string> = {};
    if (status) params['status'] = status;
    return this.http.get<PaginatedResponse<SpDecision>>(
      `${this.baseUrl}/${this.projectId}/${this.resource}`, { params }
    ).pipe(map(res => res.data));
  }

  listLinkedTasks(decisionId: string): Observable<SpTaskSummaryForDecision[]> {
    return this.http.get<SpTaskSummaryForDecision[]>(
      `${this.baseUrl}/${this.resource}/${decisionId}/tasks`
    );
  }
}
