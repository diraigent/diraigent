import { Component, inject, signal, computed, DestroyRef, HostListener } from '@angular/core';
import { DatePipe } from '@angular/common';
import { Router } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { timer, switchMap, forkJoin, from, mergeMap, toArray, of } from 'rxjs';
import { catchError, map } from 'rxjs/operators';
import { TasksApiService, SpTask } from '../../core/services/tasks-api.service';
import { DiraigentApiService, DgProject, TokenDayCount, CostSummary, DashboardProjectSummary } from '../../core/services/diraigent-api.service';
import { GitApiService, BranchInfo } from '../../core/services/git-api.service';
import { WORK_STATUS_COLORS, taskStateColor, taskTransitions } from '../../shared/ui-constants';
import { TokenUsageChartComponent, ChartProject } from './token-usage-chart';

interface UnmergedBranchRow {
  branch: BranchInfo;
  projectName: string;
}

interface ActiveWorkRow {
  work: { id: string; title: string; status: string; work_type: string; priority: number; updated_at: string };
  projectName: string;
}

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
          @for (pt of projectSummaries(); track pt.project.id) {
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
  private router = inject(Router);
  private destroyRef = inject(DestroyRef);

  projectSummaries = signal<DashboardProjectSummary[]>([]);
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
    const sums = this.projectSummaries().reduce(
      (acc, ps) => ({
        active: acc.active + ps.task_summary.in_progress + ps.task_summary.ready + ps.task_summary.backlog + ps.task_summary.human_review,
        ready: acc.ready + ps.task_summary.ready,
        inProgress: acc.inProgress + ps.task_summary.in_progress + ps.task_summary.human_review,
        doneToday: acc.doneToday, // not available from summary — keep 0
        cancelledToday: acc.cancelledToday,
      }),
      { active: 0, ready: 0, inProgress: 0, doneToday: 0, cancelledToday: 0 },
    );
    return sums;
  });

  totalCostUsd = computed(() =>
    this.allCostSummaries().reduce((sum, c) => sum + c.total_cost_usd, 0),
  );

  chartProjects = computed<ChartProject[]>(() =>
    this.projectSummaries().map(ps => ({ id: ps.project.id, name: ps.project.name })),
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

    const totalCosts = this.allCostSummaries();
    const totalInput = totalCosts.reduce((s, c) => s + c.total_input_tokens, 0);
    const totalOutput = totalCosts.reduce((s, c) => s + c.total_output_tokens, 0);

    return {
      today: sumDays(todayDays),
      week: sumDays(weekDays),
      total: { input: totalInput, output: totalOutput, total: totalInput + totalOutput },
    };
  });

  constructor() {
    this.startPolling();
  }

  projectActive(pt: DashboardProjectSummary): number {
    const s = pt.task_summary;
    return s.in_progress + s.ready + s.backlog + s.human_review;
  }

  projectReady(pt: DashboardProjectSummary): number {
    return pt.task_summary.ready;
  }

  projectInProgress(pt: DashboardProjectSummary): number {
    return pt.task_summary.in_progress + pt.task_summary.human_review;
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
    this.tasksApi.transition(task.id, target).subscribe();
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
        switchMap(() => this.diraigentApi.getDashboardSummary(30)),
        switchMap(summary => {
          // Update summary data immediately (1 request done)
          this.projectSummaries.set(summary.projects);
          this.allActiveWork.set(
            summary.projects
              .flatMap(ps =>
                ps.active_work.map(w => ({ work: w, projectName: ps.project.name })),
              )
              .sort((a, b) =>
                b.work.priority - a.work.priority
                || new Date(b.work.updated_at).getTime() - new Date(a.work.updated_at).getTime(),
              ),
          );
          this.allTokensPerDay.set(summary.tokens_per_day);
          this.allCostSummaries.set(summary.projects.map(ps => ps.cost_summary));
          this.loading.set(false);
          this.error.set(false);
          this.lastUpdated.set(new Date());

          // Fetch branches per project (lightweight WS calls, done in parallel)
          if (summary.projects.length === 0) {
            this.unmergedBranches.set([]);
            return of(null);
          }
          return from(summary.projects).pipe(
            mergeMap(ps =>
              this.gitApi.listBranchesForProject(ps.project.id, 'agent/').pipe(
                map(res => ({ projectName: ps.project.name, branches: res.branches })),
                catchError(() => of({ projectName: ps.project.name, branches: [] as BranchInfo[] })),
              ),
              3, // concurrency limit
            ),
            toArray(),
            map(results => {
              const rows: UnmergedBranchRow[] = [];
              for (const r of results) {
                for (const branch of r.branches) {
                  rows.push({ branch, projectName: r.projectName });
                }
              }
              this.unmergedBranches.set(rows);
              return null;
            }),
          );
        }),
        takeUntilDestroyed(this.destroyRef),
      )
      .subscribe({
        error: () => {
          this.loading.set(false);
          this.error.set(true);
        },
      });
  }
}
