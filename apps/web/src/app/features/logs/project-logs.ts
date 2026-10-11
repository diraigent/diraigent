import { Component, effect, inject, signal, untracked } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { ProjectContext } from '../../core/services/project-context.service';
import { environment } from '../../../environments/environment';

interface ExecutionLog { id: string; step_name: string; content?: string }
interface LogPage { data: ExecutionLog[]; has_more: boolean }

@Component({
  selector: 'app-project-logs',
  standalone: true,
  template: `
    <section class="p-3 sm:p-6">
      <h1 class="text-2xl font-semibold mb-4">Logs</h1>
      <p class="text-sm text-text-secondary mb-4">Execution logs for the selected project.</p>
      @if (error()) { <p role="alert">{{ error() }}</p> }
      @if (loading()) { <p role="status">Loading logs…</p> }
      @for (log of logs(); track log.id) {
        <button class="block p-2 text-accent" (click)="open(log)">{{ log.step_name }}</button>
      }
      @if (!loading() && !logs().length && !error()) { <p>No execution logs yet.</p> }
      @if (hasMore() || error()) {
        <button class="p-2 text-accent" [disabled]="loading()" (click)="load()">
          {{ error() ? 'Retry' : 'Load More' }}
        </button>
      }
      @if (selected(); as log) {
        <h2 class="font-semibold mt-4">{{ log.step_name }}</h2>
        <pre class="whitespace-pre-wrap break-words p-3 bg-surface rounded">{{ log.content }}</pre>
      }
    </section>
  `,
})
export class ProjectLogs {
  private http = inject(HttpClient);
  private ctx = inject(ProjectContext);
  readonly logs = signal<ExecutionLog[]>([]);
  readonly selected = signal<ExecutionLog | null>(null);
  readonly loading = signal(false);
  readonly hasMore = signal(false);
  readonly error = signal<string | null>(null);
  private offset = 0;
  private generation = 0;

  constructor() {
    effect(() => {
      this.ctx.projectId();
      untracked(() => {
        this.generation++;
        this.logs.set([]);
        this.selected.set(null);
        this.offset = 0;
        this.loading.set(false);
        this.error.set(null);
        this.hasMore.set(false);
        this.load();
      });
    });
  }

  load(): void {
    const project = this.ctx.projectId();
    const generation = this.generation;
    if (!project || this.loading()) return;
    this.loading.set(true);
    this.error.set(null);
    this.http.get<LogPage>(`${environment.apiServer}/${project}/task-logs`, {
      params: { offset: this.offset, limit: 50 },
    }).subscribe({
      next: page => {
        if (generation !== this.generation) return;
        this.logs.update(logs => [...logs, ...page.data]);
        this.offset += page.data.length;
        this.hasMore.set(page.has_more);
        this.loading.set(false);
      },
      error: () => {
        if (generation !== this.generation) return;
        this.error.set('Could not load execution logs.');
        this.loading.set(false);
      },
    });
  }

  open(log: ExecutionLog): void {
    const project = this.ctx.projectId();
    const generation = this.generation;
    this.selected.set(log);
    this.http.get<ExecutionLog>(`${environment.apiServer}/task-logs/${log.id}`).subscribe({
      next: detail => {
        if (generation === this.generation && this.selected()?.id === log.id) this.selected.set(detail);
      },
      error: () => {
        if (generation === this.generation && this.selected()?.id === log.id) this.error.set('Could not load this execution log.');
      },
    });
  }
}
