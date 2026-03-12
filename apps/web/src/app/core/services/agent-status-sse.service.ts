import { Injectable, inject, OnDestroy } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { OAuthService } from 'angular-oauth2-oidc';
import { Subject, Observable, firstValueFrom } from 'rxjs';
import { environment } from '../../../environments/environment';

export interface AgentStatusSseEvent {
  agent_id: string;
  name: string;
  status: string;
}

/**
 * Connects to the API's `/agents/stream` SSE endpoint and emits
 * {@link AgentStatusSseEvent} objects in real time whenever an agent's
 * status changes (heartbeat, status update).
 *
 * Uses the same short-lived opaque ticket pattern as {@link ReviewSseService}:
 * 1. Call `POST /agents/stream/ticket` (with `Authorization: Bearer`) to get a
 *    60-second, single-use ticket UUID.
 * 2. Open `EventSource` with `?ticket=<uuid>`.
 *
 * On error the service reconnects with an exponential back-off (max 30 s).
 */
@Injectable({ providedIn: 'root' })
export class AgentStatusSseService implements OnDestroy {
  private oauth = inject(OAuthService);
  private http = inject(HttpClient);

  private readonly events$ = new Subject<AgentStatusSseEvent>();
  private es: EventSource | null = null;
  private retryMs = 2_000;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;

  /** Observable stream of agent status-change events. */
  readonly events: Observable<AgentStatusSseEvent> = this.events$.asObservable();

  constructor() {
    this.connect();
  }

  ngOnDestroy(): void {
    this.disconnect();
  }

  private async connect(): Promise<void> {
    const base = environment.apiServer;

    let ticket: string | null = null;
    try {
      const token = this.oauth.getAccessToken();
      if (token) {
        const resp = await firstValueFrom(
          this.http.post<{ ticket: string }>(
            `${base}/agents/stream/ticket`,
            null,
            { headers: { Authorization: `Bearer ${token}` } }
          )
        );
        ticket = resp.ticket;
      }
    } catch {
      this.scheduleReconnect();
      return;
    }

    if (!ticket) {
      return;
    }

    const url = `${base}/agents/stream?ticket=${encodeURIComponent(ticket)}`;
    const es = new EventSource(url);
    this.es = es;

    es.addEventListener('agent_update', (e: MessageEvent) => {
      try {
        const event: AgentStatusSseEvent = JSON.parse(e.data);
        this.events$.next(event);
      } catch {
        // Malformed JSON — ignore
      }
      this.retryMs = 2_000;
    });

    es.addEventListener('error', () => {
      es.close();
      this.es = null;
      this.scheduleReconnect();
    });
  }

  private scheduleReconnect(): void {
    if (this.retryTimer) return;
    this.retryTimer = setTimeout(() => {
      this.retryTimer = null;
      this.retryMs = Math.min(this.retryMs * 2, 30_000);
      this.connect();
    }, this.retryMs);
  }

  private disconnect(): void {
    if (this.retryTimer) {
      clearTimeout(this.retryTimer);
      this.retryTimer = null;
    }
    if (this.es) {
      this.es.close();
      this.es = null;
    }
  }
}
