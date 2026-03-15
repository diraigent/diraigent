import { Component, inject, signal, effect, OnDestroy, DestroyRef } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { Subscription, timer } from 'rxjs';
import { CiApiService, CiRun, CiRunFilters, PaginatedResponse } from '../../core/services/ci-api.service';
import { ProjectContext } from '../../core/services/project-context.service';
import { CI_STATUS_COLORS } from '../../shared/ui-constants';

@Component({
  selector: 'app-pipelines',
  standalone: true,
  imports: [TranslocoModule, FormsModule],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <h1 class="text-2xl font-semibold text-text-primary mb-3 sm:mb-6">{{ t('pipelines.title') }}</h1>

      <!-- Filters -->
      <div class="flex flex-wrap items-center gap-3 mb-4">
        <!-- Branch filter -->
        <div class="relative">
          <select
            [ngModel]="branchFilter()"
            (ngModelChange)="branchFilter.set($event); loadRuns()"
            class="appearance-none bg-surface border border-border rounded-lg px-3 py-1.5 pr-8 text-sm text-text-primary
                   focus:outline-none focus:ring-1 focus:ring-accent cursor-pointer">
            <option value="">{{ t('pipelines.allBranches') }}</option>
            @for (b of branches(); track b) {
              <option [value]="b">{{ b }}</option>
            }
          </select>
          <svg class="absolute right-2 top-1/2 -translate-y-1/2 w-4 h-4 text-text-muted pointer-events-none" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
          </svg>
        </div>

        <!-- Status multi-select filter -->
        <div class="flex items-center gap-1.5">
          @for (s of allStatuses; track s) {
            <button
              (click)="toggleStatus(s)"
              class="px-2.5 py-1 rounded-md text-xs font-medium transition-colors border"
              [class]="isStatusActive(s) ? statusColor(s) + ' border-transparent' : 'bg-transparent border-border text-text-muted hover:text-text-secondary'">
              {{ s }}
            </button>
          }
        </div>

        <!-- Refresh button -->
        <button
          (click)="loadRuns()"
          class="ml-auto px-3 py-1.5 text-sm font-medium text-text-secondary hover:text-text-primary
                 bg-surface border border-border rounded-lg transition-colors"
          [disabled]="loading()">
          {{ t('pipelines.refresh') }}
        </button>
      </div>

      <!-- Polling indicator -->
      @if (isPolling()) {
        <div class="flex items-center gap-2 mb-3 text-xs text-text-muted">
          <span class="relative flex h-2 w-2">
            <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-ctp-green opacity-75"></span>
            <span class="relative inline-flex rounded-full h-2 w-2 bg-ctp-green"></span>
          </span>
          {{ t('pipelines.autoPolling') }}
        </div>
      }

      <!-- Loading skeleton -->
      @if (loading() && runs().length === 0) {
        <div class="space-y-3">
          @for (i of [1,2,3,4,5]; track i) {
            <div class="bg-surface rounded-lg border border-border p-4 animate-pulse">
              <div class="flex items-center gap-4">
                <div class="h-4 w-32 bg-bg-subtle rounded"></div>
                <div class="h-4 w-20 bg-bg-subtle rounded"></div>
                <div class="h-5 w-16 bg-bg-subtle rounded-full"></div>
                <div class="h-4 w-16 bg-bg-subtle rounded ml-auto"></div>
              </div>
            </div>
          }
        </div>
      }

      <!-- Empty state -->
      @else if (!loading() && runs().length === 0) {
        <div class="text-center py-16">
          <svg class="mx-auto h-12 w-12 text-text-muted mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
              d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
          </svg>
          <h3 class="text-lg font-medium text-text-primary mb-1">{{ t('pipelines.emptyTitle') }}</h3>
          <p class="text-sm text-text-muted">{{ t('pipelines.emptyDescription') }}</p>
        </div>
      }

      <!-- Runs table -->
      @else {
        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b border-border text-left text-text-muted">
                <th class="pb-2 pr-4 font-medium">{{ t('pipelines.workflow') }}</th>
                <th class="pb-2 pr-4 font-medium">{{ t('pipelines.branch') }}</th>
                <th class="pb-2 pr-4 font-medium">{{ t('pipelines.commit') }}</th>
                <th class="pb-2 pr-4 font-medium">{{ t('pipelines.status') }}</th>
                <th class="pb-2 pr-4 font-medium">{{ t('pipelines.triggeredBy') }}</th>
                <th class="pb-2 font-medium">{{ t('pipelines.started') }}</th>
              </tr>
            </thead>
            <tbody>
              @for (run of runs(); track run.id) {
                <tr (click)="navigateToRun(run.id)" class="border-b border-border/50 hover:bg-surface/50 transition-colors cursor-pointer">
                  <td class="py-3 pr-4">
                    <span class="font-medium text-text-primary">{{ run.workflow_name }}</span>
                  </td>
                  <td class="py-3 pr-4">
                    @if (run.branch) {
                      <span class="inline-flex items-center gap-1 text-xs font-mono bg-surface px-2 py-0.5 rounded border border-border">
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                            d="M13 10V3L4 14h7v7l9-11h-7z" />
                        </svg>
                        {{ run.branch }}
                      </span>
                    } @else {
                      <span class="text-text-muted">—</span>
                    }
                  </td>
                  <td class="py-3 pr-4">
                    @if (run.commit_sha) {
                      <span class="font-mono text-xs text-text-secondary">{{ run.commit_sha.substring(0, 7) }}</span>
                    } @else {
                      <span class="text-text-muted">—</span>
                    }
                  </td>
                  <td class="py-3 pr-4">
                    <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium"
                          [class]="statusColor(run.status)">
                      @if (run.status === 'running') {
                        <svg class="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24">
                          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
                        </svg>
                      } @else if (run.status === 'success') {
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                        </svg>
                      } @else if (run.status === 'failure') {
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                      } @else if (run.status === 'pending') {
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                      }
                      {{ run.status }}
                    </span>
                  </td>
                  <td class="py-3 pr-4">
                    <span class="text-text-secondary text-xs">{{ run.triggered_by ?? '—' }}</span>
                  </td>
                  <td class="py-3">
                    <span class="text-text-muted text-xs" [title]="run.started_at ?? run.created_at">
                      {{ relativeTime(run.started_at ?? run.created_at) }}
                    </span>
                  </td>
                </tr>
              }
            </tbody>
          </table>
        </div>

        <!-- Pagination -->
        @if (totalRuns() > perPage) {
          <div class="flex items-center justify-between mt-4 text-sm text-text-muted">
            <span>{{ t('pipelines.showing') }} {{ (currentPage() - 1) * perPage + 1 }}–{{ Math.min(currentPage() * perPage, totalRuns()) }} {{ t('pipelines.of') }} {{ totalRuns() }}</span>
            <div class="flex items-center gap-2">
              <button
                (click)="goToPage(currentPage() - 1)"
                [disabled]="currentPage() <= 1"
                class="px-3 py-1 rounded border border-border bg-surface text-text-secondary hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed">
                {{ t('pipelines.prev') }}
              </button>
              <button
                (click)="goToPage(currentPage() + 1)"
                [disabled]="!hasMore()"
                class="px-3 py-1 rounded border border-border bg-surface text-text-secondary hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed">
                {{ t('pipelines.next') }}
              </button>
            </div>
          </div>
        }
      }
    </div>
  `,
})
export class PipelinesPage implements OnDestroy {
  Math = Math;

  private ciApi = inject(CiApiService);
  private ctx = inject(ProjectContext);
  private destroyRef = inject(DestroyRef);
  private router = inject(Router);

  // State
  runs = signal<CiRun[]>([]);
  loading = signal(false);
  totalRuns = signal(0);
  currentPage = signal(1);
  hasMore = signal(false);
  branches = signal<string[]>([]);

  // Filters
  branchFilter = signal('');
  activeStatuses = signal<Set<string>>(new Set());

  // Polling
  isPolling = signal(false);
  private pollSub: Subscription | null = null;

  readonly perPage = 25;
  readonly allStatuses = ['success', 'failure', 'running', 'pending', 'skipped', 'cancelled'];

  constructor() {
    // Reload when project changes
    effect(() => {
      const pid = this.ctx.projectId();
      if (pid) {
        this.currentPage.set(1);
        this.branchFilter.set('');
        this.activeStatuses.set(new Set());
        this.loadRuns();
      }
    });
  }

  ngOnDestroy(): void {
    this.stopPolling();
  }

  statusColor(status: string): string {
    return CI_STATUS_COLORS[status] ?? 'bg-ctp-overlay0/20 text-ctp-overlay0';
  }

  isStatusActive(status: string): boolean {
    const active = this.activeStatuses();
    return active.size === 0 || active.has(status);
  }

  toggleStatus(status: string): void {
    const current = new Set(this.activeStatuses());
    if (current.has(status)) {
      current.delete(status);
    } else {
      current.add(status);
    }
    this.activeStatuses.set(current);
    this.currentPage.set(1);
    this.loadRuns();
  }

  goToPage(page: number): void {
    this.currentPage.set(page);
    this.loadRuns();
  }

  loadRuns(): void {
    const pid = this.ctx.projectId();
    if (!pid) return;

    this.loading.set(true);

    const activeSet = this.activeStatuses();
    const statusFilter = activeSet.size > 0 ? [...activeSet].join(',') : undefined;

    const filters: CiRunFilters = {
      page: this.currentPage(),
      per_page: this.perPage,
      branch: this.branchFilter() || undefined,
      status: statusFilter,
    };

    this.ciApi.listRuns(pid, filters)
      .pipe(takeUntilDestroyed(this.destroyRef))
      .subscribe({
        next: (res: PaginatedResponse<CiRun>) => {
          this.runs.set(res.data);
          this.totalRuns.set(res.total);
          this.hasMore.set(res.has_more);
          this.loading.set(false);

          // Extract distinct branches for the filter dropdown
          this.updateBranches(res.data);

          // Start or stop polling based on whether any run is in-progress
          const hasRunning = res.data.some(r => r.status === 'running' || r.status === 'pending');
          if (hasRunning) {
            this.startPolling();
          } else {
            this.stopPolling();
          }
        },
        error: () => {
          this.loading.set(false);
          this.stopPolling();
        },
      });
  }

  navigateToRun(runId: string): void {
    this.router.navigate(['/pipelines', runId]);
  }

  relativeTime(iso: string): string {
    const now = Date.now();
    const then = new Date(iso).getTime();
    const diff = now - then;

    if (diff < 0) return 'just now';

    const seconds = Math.floor(diff / 1000);
    if (seconds < 60) return `${seconds}s ago`;

    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;

    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;

    const days = Math.floor(hours / 24);
    return `${days}d ago`;
  }

  private updateBranches(data: CiRun[]): void {
    const existing = new Set(this.branches());
    for (const run of data) {
      if (run.branch) existing.add(run.branch);
    }
    this.branches.set([...existing].sort());
  }

  private startPolling(): void {
    if (this.pollSub) return; // already polling

    this.isPolling.set(true);
    this.pollSub = timer(30_000, 30_000)
      .pipe(takeUntilDestroyed(this.destroyRef))
      .subscribe(() => this.loadRuns());
  }

  private stopPolling(): void {
    this.isPolling.set(false);
    this.pollSub?.unsubscribe();
    this.pollSub = null;
  }
}
