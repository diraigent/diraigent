import { Injectable, inject, OnDestroy } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { OAuthService } from 'angular-oauth2-oidc';
import { Subject, Observable, firstValueFrom } from 'rxjs';
import { environment } from '../../../environments/environment';

export interface ReviewSseEvent {
  /** `"entered"` when a task transitions to human_review; `"left"` when it leaves. */
  kind: 'entered' | 'left';
  project_id: string;
  task_id: string;
  title: string;
}

/**
 * Connects to the API's `/review/stream` SSE endpoint and emits
 * {@link ReviewSseEvent} objects in real time.
 *
 * The browser's `EventSource` API cannot set custom headers, so we use a
 * short-lived opaque ticket exchange:
 * 1. Call `POST /review/stream/ticket` (with `Authorization: Bearer`) to get a
 *    60-second, single-use ticket UUID.
 * 2. Open `EventSource` with `?ticket=<uuid>` — the full JWT never appears in
 *    URLs, logs, or browser history.
 *
 * On error the service reconnects with an exponential back-off (max 30 s).
 */
@Injectable({ providedIn: 'root' })
export class ReviewSseService implements OnDestroy {
  private oauth = inject(OAuthService);
  private http = inject(HttpClient);

  private readonly events$ = new Subject<ReviewSseEvent>();
  private es: EventSource | null = null;
  private retryMs = 2_000;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;

  /** Observable stream of review state-change events. */
  readonly events: Observable<ReviewSseEvent> = this.events$.asObservable();

  constructor() {
    this.connect();
  }

  ngOnDestroy(): void {
    this.disconnect();
  }

  private async connect(): Promise<void> {
    const base = environment.apiServer;

    // Obtain a short-lived ticket before opening the EventSource.
    // If we can't get a ticket (e.g. not authenticated) skip the connection.
    let ticket: string | null = null;
    try {
      const token = this.oauth.getAccessToken();
      if (token) {
        const resp = await firstValueFrom(
          this.http.post<{ ticket: string }>(
            `${base}/review/stream/ticket`,
            null,
            { headers: { Authorization: `Bearer ${token}` } }
          )
        );
        ticket = resp.ticket;
      }
    } catch {
      // Could not obtain a ticket — schedule retry.
      this.scheduleReconnect();
      return;
    }

    if (!ticket) {
      return;
    }

    const url = `${base}/review/stream?ticket=${encodeURIComponent(ticket)}`;
    const es = new EventSource(url);
    this.es = es;

    es.addEventListener('review_update', (e: MessageEvent) => {
      try {
        const event: ReviewSseEvent = JSON.parse(e.data);
        this.events$.next(event);
      } catch {
        // Malformed JSON — ignore
      }
      // Reset back-off on successful message
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
