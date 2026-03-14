import { Component, inject, signal, computed, DestroyRef, HostListener } from '@angular/core';
import { DatePipe } from '@angular/common';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { forkJoin, timer, switchMap, of, map } from 'rxjs';
import { catchError } from 'rxjs/operators';
import { TasksApiService, SpTask } from '../../core/services/tasks-api.service';
import { DiraigentApiService, DgProject } from '../../core/services/diraigent-api.service';
import { taskStateColor, taskTransitions } from '../../shared/ui-constants';

interface ProjectTasks {
  project: DgProject;
  tasks: SpTask[];
}

interface OpenTaskRow {
  task: SpTask;
  projectName: string;
}

const ACTIVE_STATES = new Set(['backlog', 'ready', 'working', 'implement', 'review', 'merge', 'human_review']);
const IN_PROGRESS_STATES = new Set(['working', 'implement', 'review', 'merge', 'human_review']);
const isActive = (s: string) => ACTIVE_STATES.has(s) || s.startsWith('wait:');
const isInProgress = (s: string) => IN_PROGRESS_STATES.has(s) || s.startsWith('wait:');

@Component({
  selector: 'app-dashboard',
  standalone: true,
  imports: [TranslocoModule, DatePipe],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <h1 class="text-2xl font-semibold text-text-primary mb-3 sm:mb-6">{{ t('dashboard.title') }}</h1>

      @if (loading()) {
        <p class="text-text-secondary">{{ t('common.loading') }}</p>
      } @else if (error()) {
        <p class="text-error">{{ t('common.error') }}</p>
      } @else {
        <!-- Stats row -->
        <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-4 mb-4">
          <div class="bg-surface border border-border rounded-lg p-4">
            <div class="text-3xl font-bold text-ctp-peach">{{ '$' + totalCostUsd().toFixed(2) }}</div>
            <div class="text-sm text-text-secondary mt-1">{{ t('dashboard.stats.totalCost') }}</div>
            <div class="text-xs text-text-muted mt-1">{{ t('dashboard.stats.totalCostNote') }}</div>
          </div>
          <div class="bg-surface border border-border rounded-lg p-4">
            <div class="text-3xl font-bold text-text-primary">{{ stats().active }}</div>
            <div class="text-sm text-text-secondary mt-1">{{ t('dashboard.stats.active') }}</div>
          </div>
          <div class="bg-surface border border-border rounded-lg p-4">
            <div class="text-3xl font-bold text-ctp-blue">{{ stats().ready }}</div>
            <div class="text-sm text-text-secondary mt-1">{{ t('dashboard.stats.ready') }}</div>
          </div>
          <div class="bg-surface border border-border rounded-lg p-4">
            <div class="text-3xl font-bold text-ctp-yellow">{{ stats().inProgress }}</div>
            <div class="text-sm text-text-secondary mt-1">{{ t('dashboard.stats.inProgress') }}</div>
          </div>
          <div class="bg-surface border border-border rounded-lg p-4">
            <div class="text-3xl font-bold text-ctp-green">{{ stats().doneToday }}</div>
            <div class="text-sm text-text-secondary mt-1">{{ t('dashboard.stats.doneToday') }}</div>
          </div>
          <div class="bg-surface border border-border rounded-lg p-4">
            <div class="text-3xl font-bold text-ctp-red">{{ stats().cancelledToday }}</div>
            <div class="text-sm text-text-secondary mt-1">{{ t('dashboard.stats.cancelledToday') }}</div>
          </div>
        </div>

        <!-- Token usage row -->
        <div class="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-8">
          <div class="bg-surface border border-border rounded-lg p-4">
            <div class="text-3xl font-bold text-ctp-lavender">{{ formatTokens(tokenStats().today.total) }}</div>
            <div class="text-sm text-text-secondary mt-1">{{ t('dashboard.stats.tokensToday') }}</div>
            <div class="text-xs text-text-muted mt-1">{{ formatTokens(tokenStats().today.input) }} in / {{ formatTokens(tokenStats().today.output) }} out</div>
          </div>
          <div class="bg-surface border border-border rounded-lg p-4">
            <div class="text-3xl font-bold text-ctp-lavender">{{ formatTokens(tokenStats().week.total) }}</div>
            <div class="text-sm text-text-secondary mt-1">{{ t('dashboard.stats.tokensWeek') }}</div>
            <div class="text-xs text-text-muted mt-1">{{ formatTokens(tokenStats().week.input) }} in / {{ formatTokens(tokenStats().week.output) }} out</div>
          </div>
          <div class="bg-surface border border-border rounded-lg p-4">
            <div class="text-3xl font-bold text-ctp-lavender">{{ formatTokens(tokenStats().total.total) }}</div>
            <div class="text-sm text-text-secondary mt-1">{{ t('dashboard.stats.tokensTotal') }}</div>
            <div class="text-xs text-text-muted mt-1">{{ formatTokens(tokenStats().total.input) }} in / {{ formatTokens(tokenStats().total.output) }} out</div>
          </div>
        </div>

        <!-- Per-project breakdown -->
        <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3 mb-8">
          @for (pt of allProjectTasks(); track pt.project.id) {
            @if (projectActive(pt) > 0) {
              <div class="bg-surface border border-border rounded-lg p-3">
                <div class="text-xs text-text-muted truncate mb-1">{{ pt.project.name }}</div>
                <div class="flex items-center gap-2">
                  <span class="text-xl font-bold text-text-primary">{{ projectActive(pt) }}</span>
                  <span class="text-xs text-text-secondary">{{ t('dashboard.stats.active') }}</span>
                </div>
                @if (projectReady(pt) > 0) {
                  <div class="text-xs text-ctp-blue mt-0.5">{{ projectReady(pt) }} {{ t('dashboard.stats.ready') }}</div>
                }
                @if (projectInProgress(pt) > 0) {
                  <div class="text-xs text-ctp-yellow mt-0.5">{{ projectInProgress(pt) }} {{ t('dashboard.stats.inProgress') }}</div>
                }
              </div>
            }
          }
        </div>

        <!-- Open tasks table -->
        <h2 class="text-lg font-semibold text-text-primary mb-3">{{ t('dashboard.openTasks') }}</h2>
        <div class="bg-surface rounded-lg border border-border overflow-hidden">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b border-border text-text-secondary text-left">
                <th class="px-4 py-3 font-medium w-36">{{ t('dashboard.project') }}</th>
                <th class="px-4 py-3 font-medium">{{ t('tasks.title') }}</th>
                <th class="px-4 py-3 font-medium w-24">{{ t('tasks.kind') }}</th>
                <th class="px-4 py-3 font-medium w-28">{{ t('tasks.state') }}</th>
                <th class="px-4 py-3 font-medium w-24">{{ t('tasks.urgent') }}</th>
                <th class="px-4 py-3 font-medium w-36">{{ t('tasks.created') }}</th>
              </tr>
            </thead>
            <tbody>
              @for (row of openTasks(); track row.task.id) {
                <tr class="border-b border-border last:border-b-0 hover:bg-surface-hover transition-colors">
                  <td class="px-4 py-3 text-text-muted text-xs truncate max-w-[144px]">{{ row.projectName }}</td>
                  <td class="px-4 py-3 text-text-primary font-medium">{{ row.task.title }}</td>
                  <td class="px-4 py-3 text-text-secondary text-xs">{{ row.task.kind }}</td>
                  <td class="px-4 py-3 relative">
                    <button class="px-2 py-0.5 rounded-full text-xs font-medium cursor-pointer hover:ring-1 hover:ring-accent {{ stateColor(row.task.state) }}"
                            (click)="toggleStateMenu($event, row.task.id)">
                      {{ row.task.state }}
                    </button>
                    @if (openMenuId() === row.task.id) {
                      <div class="absolute z-50 mt-1 left-2 bg-surface border border-border rounded-lg shadow-lg py-1 min-w-[120px]">
                        @for (target of getTransitions(row.task.state); track target) {
                          <button (click)="onTransition($event, row.task, target)"
                            class="w-full text-left px-3 py-1.5 text-xs hover:bg-surface-hover transition-colors flex items-center gap-2 cursor-pointer">
                            <span class="w-2 h-2 rounded-full {{ stateColor(target) }}"></span>
                            <span class="text-text-primary">{{ target }}</span>
                          </button>
                        } @empty {
                          <span class="px-3 py-1.5 text-xs text-text-muted block">No transitions</span>
                        }
                      </div>
                    }
                  </td>
                  <td class="px-4 py-3 text-xs">
                    @if (row.task.urgent) {
                      <span class="text-ctp-red font-medium">Urgent</span>
                    }
                  </td>
                  <td class="px-4 py-3 text-text-muted text-xs whitespace-nowrap">
                    {{ row.task.created_at | date:'MMM d, y' }}
                  </td>
                </tr>
              } @empty {
                <tr>
                  <td colspan="6" class="px-4 py-8 text-center text-text-muted">{{ t('common.empty') }}</td>
                </tr>
              }
            </tbody>
          </table>
        </div>

        <p class="text-xs text-text-muted mt-3">
          {{ t('dashboard.lastUpdated') }}: {{ lastUpdated() | date:'HH:mm:ss' }}
        </p>
      }
    </div>
  `,
})
export class DashboardPage {
  private tasksApi = inject(TasksApiService);
  private diraigentApi = inject(DiraigentApiService);
  private destroyRef = inject(DestroyRef);

  allProjectTasks = signal<ProjectTasks[]>([]);
  loading = signal(true);
  error = signal(false);
  lastUpdated = signal<Date>(new Date());
  openMenuId = signal<string | null>(null);

  @HostListener('document:click')
  onDocumentClick(): void {
    this.openMenuId.set(null);
  }

  stats = computed(() => {
    const allTasks = this.allProjectTasks().flatMap(pt => pt.tasks);
    const todayStart = new Date();
    todayStart.setHours(0, 0, 0, 0);
    return {
      active: allTasks.filter(t => isActive(t.state)).length,
      ready: allTasks.filter(t => t.state === 'ready').length,
      inProgress: allTasks.filter(t => isInProgress(t.state)).length,
      doneToday: allTasks.filter(t => t.state === 'done' && new Date(t.completed_at ?? t.updated_at) >= todayStart).length,
      cancelledToday: allTasks.filter(t => t.state === 'cancelled' && new Date(t.updated_at) >= todayStart).length,
    };
  });

  totalCostUsd = computed(() =>
    this.allProjectTasks()
      .flatMap(pt => pt.tasks)
      .reduce((sum, t) => sum + (t.cost_usd ?? 0), 0),
  );

  tokenStats = computed(() => {
    const allTasks = this.allProjectTasks().flatMap(pt => pt.tasks);
    const todayStart = new Date();
    todayStart.setHours(0, 0, 0, 0);
    const weekStart = new Date(todayStart);
    weekStart.setDate(weekStart.getDate() - weekStart.getDay());

    const sumTokens = (tasks: SpTask[]) => {
      const input = tasks.reduce((s, t) => s + (t.input_tokens ?? 0), 0);
      const output = tasks.reduce((s, t) => s + (t.output_tokens ?? 0), 0);
      return { input, output, total: input + output };
    };

    const todayTasks = allTasks.filter(t => {
      if (isInProgress(t.state) && t.claimed_at && new Date(t.claimed_at) >= todayStart) return true;
      if (t.state === 'done' && t.completed_at && new Date(t.completed_at) >= todayStart) return true;
      return false;
    });

    const weekTasks = allTasks.filter(t => {
      if (isInProgress(t.state)) return true;
      if (t.state === 'done' && t.completed_at && new Date(t.completed_at) >= weekStart) return true;
      return false;
    });

    return {
      today: sumTokens(todayTasks),
      week: sumTokens(weekTasks),
      total: sumTokens(allTasks),
    };
  });

  openTasks = computed(() => {
    const rows: OpenTaskRow[] = [];
    for (const { project, tasks } of this.allProjectTasks()) {
      for (const task of tasks) {
        if (task.state !== 'done' && task.state !== 'cancelled') {
          rows.push({ task, projectName: project.name });
        }
      }
    }
    // Sort: in-progress first, then ready, then backlog; urgent tasks first within same state group
    const statePriority = (s: string): number => {
      if (isInProgress(s)) return 0;
      if (s === 'ready') return 1;
      return 2; // backlog
    };
    rows.sort((a, b) => {
      const sp = statePriority(a.task.state) - statePriority(b.task.state);
      if (sp !== 0) return sp;
      return (b.task.urgent ? 1 : 0) - (a.task.urgent ? 1 : 0);
    });
    return rows;
  });

  constructor() {
    this.startPolling();
  }

  projectActive(pt: ProjectTasks): number {
    return pt.tasks.filter(t => isActive(t.state)).length;
  }

  projectReady(pt: ProjectTasks): number {
    return pt.tasks.filter(t => t.state === 'ready').length;
  }

  projectInProgress(pt: ProjectTasks): number {
    return pt.tasks.filter(t => isInProgress(t.state)).length;
  }

  formatTokens(n: number): string {
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
    if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K';
    return n.toString();
  }

  protected readonly stateColor = taskStateColor;
  protected readonly getTransitions = taskTransitions;

  toggleStateMenu(event: Event, taskId: string): void {
    event.stopPropagation();
    this.openMenuId.set(this.openMenuId() === taskId ? null : taskId);
  }

  onTransition(event: Event, task: SpTask, target: string): void {
    event.stopPropagation();
    this.openMenuId.set(null);
    this.tasksApi.transition(task.id, target).subscribe({
      next: (updated) => {
        this.allProjectTasks.update(all =>
          all.map(pt => ({
            ...pt,
            tasks: pt.tasks.map(t => t.id === updated.id ? updated : t),
          })),
        );
      },
    });
  }

  private startPolling(): void {
    timer(0, 30_000)
      .pipe(
        switchMap(() => this.diraigentApi.getProjects()),
        switchMap(projects =>
          projects.length === 0
            ? of([] as ProjectTasks[])
            : forkJoin(
                projects.map(project => {
                  const retentionDays = (project.metadata?.['done_retention_days'] as number) ?? 1;
                  const cutoff = new Date(Date.now() - retentionDays * 24 * 60 * 60 * 1000).toISOString();
                  return this.tasksApi
                    .listForProject(project.id, { limit: 200, hide_done_before: cutoff })
                    .pipe(
                      catchError(() => of({ data: [] as SpTask[], total: 0, limit: 200, offset: 0, has_more: false })),
                      map(res => ({ project, tasks: res.data }) as ProjectTasks),
                    );
                }),
              ),
        ),
        takeUntilDestroyed(this.destroyRef),
      )
      .subscribe({
        next: projectTasks => {
          this.allProjectTasks.set(projectTasks);
          this.loading.set(false);
          this.error.set(false);
          this.lastUpdated.set(new Date());
        },
        error: () => {
          this.loading.set(false);
          this.error.set(true);
        },
      });
  }
}
