import { Component, inject, signal, computed, DestroyRef, HostListener } from '@angular/core';
import { DatePipe } from '@angular/common';
import { Router } from '@angular/router';
import { HttpClient } from '@angular/common/http';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { forkJoin, timer, switchMap, of, map } from 'rxjs';
import { catchError } from 'rxjs/operators';
import { TasksApiService, SpTask } from '../../core/services/tasks-api.service';
import { DiraigentApiService, DgProject, TokenDayCount, CostSummary } from '../../core/services/diraigent-api.service';
import { GitApiService, BranchInfo } from '../../core/services/git-api.service';
import { SpWork } from '../../core/services/work-api.service';
import { taskStateColor, taskTransitions, WORK_STATUS_COLORS } from '../../shared/ui-constants';
import { TokenUsageChartComponent, ChartProject } from './token-usage-chart';
import { environment } from '../../../environments/environment';

interface ProjectTasks {
  project: DgProject;
  tasks: SpTask[];
}

interface OpenTaskRow {
  task: SpTask;
  projectName: string;
}

interface ActiveWorkRow {
  work: SpWork;
  projectName: string;
}

interface UnmergedBranchRow {
  branch: BranchInfo;
  taskTitle: string | null;
  taskState: string | null;
  projectName: string;
}

const ACTIVE_STATES = new Set(['backlog', 'ready', 'working', 'implement', 'review', 'merge', 'human_review']);
const IN_PROGRESS_STATES = new Set(['working', 'implement', 'review', 'merge', 'human_review']);
const isActive = (s: string) => ACTIVE_STATES.has(s) || s.startsWith('wait:');
const isInProgress = (s: string) => IN_PROGRESS_STATES.has(s) || s.startsWith('wait:');

@Component({
  selector: 'app-dashboard',
  standalone: true,
  imports: [TranslocoModule, DatePipe, TokenUsageChartComponent],
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
        <div class="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-4">
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

        <!-- Token usage chart (all projects combined) -->
        @if (chartProjects().length > 0) {
          <div class="mb-4">
            <app-token-usage-chart [projects]="chartProjects()" />
          </div>
        }

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

        <!-- Unmerged branches -->
        @if (unmergedBranches().length > 0) {
          <h2 class="text-lg font-semibold text-text-primary mb-3 flex items-center gap-2">
            {{ t('dashboard.unmergedBranches') }}
            <span class="text-sm font-normal text-ctp-yellow">({{ unmergedBranches().length }})</span>
          </h2>
          <div class="bg-surface rounded-lg border border-border overflow-hidden mb-8">
            <table class="w-full text-sm">
              <thead>
                <tr class="border-b border-border text-text-secondary text-left">
                  <th class="px-4 py-3 font-medium w-36">{{ t('dashboard.project') }}</th>
                  <th class="px-4 py-3 font-medium">{{ t('dashboard.branch') }}</th>
                  <th class="px-4 py-3 font-medium">{{ t('tasks.title') }}</th>
                  <th class="px-4 py-3 font-medium w-28">{{ t('tasks.state') }}</th>
                  <th class="px-4 py-3 font-medium w-28">{{ t('dashboard.status') }}</th>
                </tr>
              </thead>
              <tbody>
                @for (row of unmergedBranches(); track row.branch.name) {
                  <tr class="border-b border-border last:border-b-0 hover:bg-surface-hover transition-colors">
                    <td class="px-4 py-3 text-text-muted text-xs truncate max-w-[144px]">{{ row.projectName }}</td>
                    <td class="px-4 py-3 font-mono text-xs text-text-primary truncate max-w-[240px]" [title]="row.branch.name">
                      {{ row.branch.name }}
                    </td>
                    <td class="px-4 py-3 text-text-primary text-xs">{{ row.taskTitle ?? '—' }}</td>
                    <td class="px-4 py-3">
                      @if (row.taskState) {
                        <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ stateColor(row.taskState) }}">
                          {{ row.taskState }}
                        </span>
                      } @else {
                        <span class="text-text-muted text-xs">—</span>
                      }
                    </td>
                    <td class="px-4 py-3">
                      @if (row.branch.is_pushed) {
                        <span class="inline-flex items-center gap-1 text-xs text-ctp-green">
                          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                            <path d="M5 13l4 4L19 7" />
                          </svg>
                          {{ t('dashboard.pushed') }}
                        </span>
                      } @else {
                        <span class="inline-flex items-center gap-1 text-xs text-ctp-yellow">
                          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                            <path d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
                          </svg>
                          {{ t('dashboard.localOnly') }}
                        </span>
                      }
                    </td>
                  </tr>
                }
              </tbody>
            </table>
          </div>
        }

        <!-- Active Work section -->
        <h2 class="text-lg font-semibold text-text-primary mb-3">{{ t('dashboard.activeWork') }}</h2>
        @if (allActiveWork().length === 0) {
          <p class="text-sm text-text-muted mb-8">{{ t('dashboard.noActiveWork') }}</p>
        } @else {
          <div class="bg-surface rounded-lg border border-border overflow-hidden mb-8">
            <table class="w-full text-sm">
              <thead>
                <tr class="border-b border-border text-text-secondary text-left">
                  <th class="px-4 py-3 font-medium w-36">{{ t('dashboard.project') }}</th>
                  <th class="px-4 py-3 font-medium">{{ t('tasks.title') }}</th>
                  <th class="px-4 py-3 font-medium w-28">{{ t('tasks.state') }}</th>
                  <th class="px-4 py-3 font-medium w-24">{{ t('goals.fieldType') }}</th>
                </tr>
              </thead>
              <tbody>
                @for (row of allActiveWork(); track row.work.id) {
                  <tr class="border-b border-border last:border-b-0 hover:bg-surface-hover transition-colors cursor-pointer"
                      (click)="navigateToWork(row.work.id)">
                    <td class="px-4 py-3 text-text-muted text-xs truncate max-w-[144px]">{{ row.projectName }}</td>
                    <td class="px-4 py-3 text-text-primary font-medium">{{ row.work.title }}</td>
                    <td class="px-4 py-3">
                      <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ workStatusColor(row.work.status) }}">
                        {{ row.work.status }}
                      </span>
                    </td>
                    <td class="px-4 py-3 text-text-secondary text-xs">{{ row.work.work_type }}</td>
                  </tr>
                }
              </tbody>
            </table>
          </div>
        }

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
  private gitApi = inject(GitApiService);
  private http = inject(HttpClient);
  private router = inject(Router);
  private destroyRef = inject(DestroyRef);

  allProjectTasks = signal<ProjectTasks[]>([]);
  allActiveWork = signal<ActiveWorkRow[]>([]);
  unmergedBranches = signal<UnmergedBranchRow[]>([]);
  allTokensPerDay = signal<TokenDayCount[]>([]);
  allCostSummaries = signal<CostSummary[]>([]);
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
    this.allCostSummaries().reduce((sum, c) => sum + c.total_cost_usd, 0),
  );

  chartProjects = computed<ChartProject[]>(() =>
    this.allProjectTasks().map(pt => ({ id: pt.project.id, name: pt.project.name })),
  );

  tokenStats = computed(() => {
    const days = this.allTokensPerDay();
    const todayStr = new Date().toISOString().slice(0, 10);
    const weekStart = new Date();
    weekStart.setDate(weekStart.getDate() - weekStart.getDay());
    const weekStartStr = weekStart.toISOString().slice(0, 10);

    const sumDays = (filtered: TokenDayCount[]) => {
      const input = filtered.reduce((s, d) => s + d.input_tokens, 0);
      const output = filtered.reduce((s, d) => s + d.output_tokens, 0);
      return { input, output, total: input + output };
    };

    const todayDays = days.filter(d => d.day === todayStr);
    const weekDays = days.filter(d => d.day >= weekStartStr);

    // Total from cost summaries (covers all time, not just loaded days)
    const totalCosts = this.allCostSummaries();
    const totalInput = totalCosts.reduce((s, c) => s + c.total_input_tokens, 0);
    const totalOutput = totalCosts.reduce((s, c) => s + c.total_output_tokens, 0);

    return {
      today: sumDays(todayDays),
      week: sumDays(weekDays),
      total: { input: totalInput, output: totalOutput, total: totalInput + totalOutput },
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

  workStatusColor(status: string): string {
    return (WORK_STATUS_COLORS as Record<string, string>)[status] ?? 'bg-ctp-overlay0/20 text-ctp-overlay0';
  }

  navigateToWork(workId: string): void {
    this.router.navigate(['/work'], { queryParams: { workId } });
  }

  private startPolling(): void {
    timer(0, 30_000)
      .pipe(
        switchMap(() => this.diraigentApi.getProjects()),
        switchMap(projects => {
          if (projects.length === 0) {
            return of({ projectTasks: [] as ProjectTasks[], activeWork: [] as ActiveWorkRow[], unmerged: [] as UnmergedBranchRow[], tokensPerDay: [] as TokenDayCount[], costSummaries: [] as CostSummary[] });
          }
          return forkJoin(
            projects.map(project => {
              const retentionDays = (project.metadata?.['done_retention_days'] as number) ?? 1;
              const cutoff = new Date(Date.now() - retentionDays * 24 * 60 * 60 * 1000).toISOString();
              const tasks$ = this.tasksApi
                .listForProject(project.id, { limit: 200, hide_done_before: cutoff })
                .pipe(
                  catchError(() => of({ data: [] as SpTask[], total: 0, limit: 200, offset: 0, has_more: false })),
                  map(res => res.data),
                );
              const work$ = this.http
                .get<SpWork[]>(`${environment.apiServer}/${project.id}/work`, { params: { limit: '200' } })
                .pipe(catchError(() => of([] as SpWork[])));
              const branches$ = this.gitApi
                .listBranchesForProject(project.id, 'agent/')
                .pipe(catchError(() => of({ current_branch: '', branches: [] as BranchInfo[] })));
              const metrics$ = this.diraigentApi.getProjectMetrics(project.id, 30).pipe(
                catchError(() => of({ tokens_per_day: [] as TokenDayCount[], cost_summary: { total_input_tokens: 0, total_output_tokens: 0, total_cost_usd: 0 } as CostSummary } as any)),
              );
              return forkJoin([tasks$, work$, branches$, metrics$]).pipe(
                map(([tasks, works, branchRes, metrics]) => ({
                  project, tasks, works,
                  branches: branchRes.branches,
                  tokensPerDay: (metrics.tokens_per_day ?? []) as TokenDayCount[],
                  costSummary: (metrics.cost_summary ?? { total_input_tokens: 0, total_output_tokens: 0, total_cost_usd: 0 }) as CostSummary,
                })),
              );
            }),
          ).pipe(
            map(results => {
              const unmerged: UnmergedBranchRow[] = [];
              for (const r of results) {
                const taskByPrefix = new Map(r.tasks.map(t => [t.id.substring(0, 12), t]));
                for (const branch of r.branches) {
                  const matchedTask = branch.task_id_prefix ? taskByPrefix.get(branch.task_id_prefix) : null;
                  // Skip branches whose task is already done/cancelled (merged)
                  if (matchedTask && (matchedTask.state === 'done' || matchedTask.state === 'cancelled')) continue;
                  unmerged.push({
                    branch,
                    taskTitle: matchedTask?.title ?? null,
                    taskState: matchedTask?.state ?? null,
                    projectName: r.project.name,
                  });
                }
              }
              return {
                projectTasks: results.map(r => ({ project: r.project, tasks: r.tasks })) as ProjectTasks[],
                activeWork: results
                  .flatMap(r =>
                    r.works
                      .filter(w => !['achieved', 'abandoned'].includes(w.status))
                      .map(w => ({ work: w, projectName: r.project.name }) as ActiveWorkRow),
                  )
                  .sort((a, b) =>
                    b.work.priority - a.work.priority
                    || new Date(b.work.updated_at).getTime() - new Date(a.work.updated_at).getTime(),
                  ),
                unmerged,
                tokensPerDay: results.flatMap(r => r.tokensPerDay),
                costSummaries: results.map(r => r.costSummary),
              };
            }),
          );
        }),
        takeUntilDestroyed(this.destroyRef),
      )
      .subscribe({
        next: ({ projectTasks, activeWork, unmerged, tokensPerDay, costSummaries }) => {
          this.allProjectTasks.set(projectTasks);
          this.allActiveWork.set(activeWork);
          this.unmergedBranches.set(unmerged);
          this.allTokensPerDay.set(tokensPerDay);
          this.allCostSummaries.set(costSummaries);
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
