import { Component, inject, signal } from '@angular/core';
import { AgentsApiService } from '../../../core/services/agents-api.service';
import { AgentStatusSseService } from '../../../core/services/agent-status-sse.service';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { catchError, of, switchMap, interval, startWith } from 'rxjs';

@Component({
  selector: 'app-agent-indicator',
  standalone: true,
  template: `
    <span
      class="inline-flex items-center gap-1 text-xs"
      [title]="available() ? 'Agent available' : 'No agent available'"
    >
      <svg
        class="w-4 h-4"
        [class.text-ctp-green]="available()"
        [class.text-ctp-red]="!available()"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
        xmlns="http://www.w3.org/2000/svg"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="1.5"
          d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"
        />
      </svg>
    </span>
  `,
})
export class AgentIndicatorComponent {
  private agents = inject(AgentsApiService);
  private agentSse = inject(AgentStatusSseService);

  available = signal(false);

  /** Track per-agent status so a single event update is sufficient. */
  private agentStatuses = new Map<string, string>();

  constructor() {
    // Initial load: fetch all agents once to populate local state.
    interval(300_000).pipe(
      startWith(0),
      switchMap(() =>
        this.agents.getAgents().pipe(
          catchError(() => of([])),
        ),
      ),
      takeUntilDestroyed(),
    ).subscribe(list => {
      this.agentStatuses.clear();
      for (const a of list) {
        this.agentStatuses.set(a.id, a.status);
      }
      this.recalculate();
    });

    // Real-time updates via SSE — no polling needed for status changes.
    this.agentSse.events.pipe(
      takeUntilDestroyed(),
    ).subscribe(event => {
      this.agentStatuses.set(event.agent_id, event.status);
      this.recalculate();
    });
  }

  private recalculate(): void {
    const hasActive = [...this.agentStatuses.values()].some(
      s => s === 'idle' || s === 'working',
    );
    this.available.set(hasActive);
  }
}
