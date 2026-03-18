import { Component, inject, signal, OnDestroy, DestroyRef } from '@angular/core';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { Subscription, timer } from 'rxjs';
import {
  CiApiService,
  CiRunWithJobs,
  CiJobWithSteps,
  CiStep,
} from '../../core/services/ci-api.service';
import { ProjectContext } from '../../core/services/project-context.service';
import { CI_STATUS_COLORS } from '../../shared/ui-constants';

@Component({
  selector: 'app-run-detail',
  standalone: true,
  imports: [TranslocoModule, RouterLink],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Breadcrumb -->
      <nav class="flex items-center gap-2 text-sm text-text-muted mb-4">
        <a routerLink="/pipelines" class="hover:text-text-primary transition-colors">
          {{ t('pipelines.title') }}
        </a>
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
        </svg>
        @if (run()) {
          <span class="text-text-secondary">{{ run()!.workflow_name }}</span>
        } @else {
          <span class="text-text-secondary">{{ t('pipelines.detail.loading') }}</span>
        }
      </nav>

      <!-- Loading state -->
      @if (loading() && !run()) {
        <div class="space-y-4">
          <div class="bg-surface rounded-lg border border-border p-6 animate-pulse">
            <div class="flex items-center gap-4 mb-4">
              <div class="h-6 w-48 bg-bg-subtle rounded"></div>
              <div class="h-5 w-20 bg-bg-subtle rounded-full"></div>
            </div>
            <div class="grid grid-cols-2 sm:grid-cols-4 gap-4">
              <div class="h-4 w-24 bg-bg-subtle rounded"></div>
              <div class="h-4 w-32 bg-bg-subtle rounded"></div>
              <div class="h-4 w-20 bg-bg-subtle rounded"></div>
              <div class="h-4 w-28 bg-bg-subtle rounded"></div>
            </div>
          </div>
          @for (i of [1,2,3]; track i) {
            <div class="bg-surface rounded-lg border border-border p-4 animate-pulse">
              <div class="flex items-center gap-4">
                <div class="h-4 w-40 bg-bg-subtle rounded"></div>
                <div class="h-5 w-16 bg-bg-subtle rounded-full"></div>
                <div class="h-4 w-20 bg-bg-subtle rounded ml-auto"></div>
              </div>
            </div>
          }
        </div>
      }

      <!-- Error state -->
      @if (error()) {
        <div class="text-center py-16">
          <svg class="mx-auto h-12 w-12 text-ctp-red mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
              d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" />
          </svg>
          <h3 class="text-lg font-medium text-text-primary mb-1">{{ t('pipelines.detail.errorTitle') }}</h3>
          <p class="text-sm text-text-muted">{{ error() }}</p>
          <a routerLink="/pipelines" class="inline-block mt-4 px-4 py-2 text-sm font-medium text-accent hover:text-accent/80 transition-colors">
            &larr; {{ t('pipelines.detail.backToList') }}
          </a>
        </div>
      }

      <!-- Run detail content -->
      @if (run(); as r) {
        <!-- Run metadata header -->
        <div class="bg-surface rounded-lg border border-border p-4 sm:p-6 mb-6">
          <div class="flex flex-wrap items-start justify-between gap-3 mb-4">
            <div class="flex items-center gap-3">
              <h1 class="text-xl font-semibold text-text-primary">{{ r.workflow_name }}</h1>
              <span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-semibold uppercase tracking-wide"
                    [class]="providerBadgeClass(r.provider)">
                {{ r.provider || 'forgejo' }}
              </span>
              <span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium"
                    [class]="statusColor(r.status)">
                @if (r.status === 'running') {
                  <svg class="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24">
                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
                  </svg>
                } @else if (r.status === 'success') {
                  <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                  </svg>
                } @else if (r.status === 'failure') {
                  <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                } @else if (r.status === 'pending') {
                  <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                }
                {{ r.status }}
              </span>
            </div>

            <!-- Refresh button -->
            <button
              (click)="loadRun()"
              class="px-3 py-1.5 text-sm font-medium text-text-secondary hover:text-text-primary
                     bg-surface border border-border rounded-lg transition-colors"
              [disabled]="loading()">
              {{ t('pipelines.refresh') }}
            </button>
          </div>

          <!-- Metadata grid -->
          <div class="grid grid-cols-2 sm:grid-cols-4 gap-y-3 gap-x-6 text-sm">
            @if (r.branch) {
              <div>
                <span class="text-text-muted block mb-0.5">{{ t('pipelines.branch') }}</span>
                <span class="inline-flex items-center gap-1 text-xs font-mono bg-bg-subtle px-2 py-0.5 rounded border border-border">
                  <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M13 10V3L4 14h7v7l9-11h-7z" />
                  </svg>
                  {{ r.branch }}
                </span>
              </div>
            }
            @if (r.commit_sha) {
              <div>
                <span class="text-text-muted block mb-0.5">{{ t('pipelines.commit') }}</span>
                <span class="font-mono text-xs text-text-secondary">{{ r.commit_sha.substring(0, 7) }}</span>
              </div>
            }
            @if (r.triggered_by) {
              <div>
                <span class="text-text-muted block mb-0.5">{{ t('pipelines.triggeredBy') }}</span>
                <span class="text-text-secondary">{{ r.triggered_by }}</span>
              </div>
            }
            <div>
              <span class="text-text-muted block mb-0.5">{{ t('pipelines.detail.duration') }}</span>
              <span class="text-text-secondary font-mono text-xs">{{ computeDuration(r.started_at, r.finished_at) }}</span>
            </div>
            @if (r.started_at) {
              <div>
                <span class="text-text-muted block mb-0.5">{{ t('pipelines.started') }}</span>
                <span class="text-text-muted text-xs" [title]="r.started_at">
                  {{ relativeTime(r.started_at) }}
                </span>
              </div>
            }
          </div>
        </div>

        <!-- Polling indicator -->
        @if (isPolling()) {
          <div class="flex items-center gap-2 mb-3 text-xs text-text-muted">
            <span class="relative flex h-2 w-2">
              <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-ctp-green opacity-75"></span>
              <span class="relative inline-flex rounded-full h-2 w-2 bg-ctp-green"></span>
            </span>
            {{ t('pipelines.detail.autoPolling') }}
          </div>
        }

        <!-- Jobs list -->
        <h2 class="text-lg font-medium text-text-primary mb-3">
          {{ t('pipelines.detail.jobs') }}
          <span class="text-text-muted text-sm font-normal">({{ r.jobs.length }})</span>
        </h2>

        @if (r.jobs.length === 0) {
          <div class="text-center py-8 text-text-muted text-sm">
            {{ t('pipelines.detail.noJobs') }}
          </div>
        } @else {
          <div class="space-y-2">
            @for (job of r.jobs; track job.id) {
              <div class="bg-surface rounded-lg border border-border overflow-hidden">
                <!-- Job header (clickable) -->
                <button
                  (click)="toggleJob(job.id)"
                  class="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-bg-subtle/50 transition-colors">
                  <!-- Expand/collapse icon -->
                  <svg class="w-4 h-4 text-text-muted transition-transform"
                       [class.rotate-90]="expandedJobs().has(job.id)"
                       fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                  </svg>

                  <!-- Status icon -->
                  <span class="inline-flex items-center justify-center w-5 h-5 rounded-full"
                        [class]="statusColor(job.status)">
                    @if (job.status === 'running') {
                      <svg class="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24">
                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
                      </svg>
                    } @else if (job.status === 'success') {
                      <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                      </svg>
                    } @else if (job.status === 'failure') {
                      <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                      </svg>
                    } @else if (job.status === 'pending') {
                      <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                      </svg>
                    }
                  </span>

                  <!-- Job name -->
                  <span class="font-medium text-sm text-text-primary flex-1">{{ job.name }}</span>

                  <!-- Runner -->
                  @if (job.runner) {
                    <span class="text-xs text-text-muted hidden sm:inline">{{ job.runner }}</span>
                  }

                  <!-- Duration -->
                  <span class="text-xs font-mono text-text-muted">
                    {{ computeDuration(job.started_at, job.finished_at) }}
                  </span>
                </button>

                <!-- Steps (expanded) -->
                @if (expandedJobs().has(job.id)) {
                  <div class="border-t border-border bg-bg-subtle/30">
                    @if (loadingJobs().has(job.id)) {
                      <div class="px-4 py-3 space-y-2">
                        @for (i of [1,2,3]; track i) {
                          <div class="flex items-center gap-3 animate-pulse">
                            <div class="h-3 w-3 bg-bg-subtle rounded-full"></div>
                            <div class="h-3 w-32 bg-bg-subtle rounded"></div>
                            <div class="h-3 w-12 bg-bg-subtle rounded ml-auto"></div>
                          </div>
                        }
                      </div>
                    } @else if (jobSteps().get(job.id); as steps) {
                      @if (steps.length === 0) {
                        <div class="px-4 py-3 text-xs text-text-muted">
                          {{ t('pipelines.detail.noSteps') }}
                        </div>
                      } @else {
                        <div class="divide-y divide-border/50">
                          @for (step of steps; track step.id; let idx = $index) {
                            <div class="flex items-center gap-3 px-4 py-2 text-sm"
                                 [class.bg-ctp-red/5]="step.exit_code !== null && step.exit_code !== 0">
                              <!-- Step number -->
                              <span class="text-xs text-text-muted w-5 text-right">{{ idx + 1 }}</span>

                              <!-- Step status icon -->
                              <span class="inline-flex items-center justify-center w-4 h-4 rounded-full flex-shrink-0"
                                    [class]="statusColor(step.status)">
                                @if (step.status === 'running') {
                                  <svg class="w-2.5 h-2.5 animate-spin" fill="none" viewBox="0 0 24 24">
                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
                                  </svg>
                                } @else if (step.status === 'success') {
                                  <svg class="w-2.5 h-2.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                                  </svg>
                                } @else if (step.status === 'failure') {
                                  <svg class="w-2.5 h-2.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                                  </svg>
                                } @else {
                                  <svg class="w-2.5 h-2.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                                  </svg>
                                }
                              </span>

                              <!-- Step name -->
                              <span class="flex-1 text-text-primary"
                                    [class.text-ctp-red]="step.exit_code !== null && step.exit_code !== 0">
                                {{ step.name }}
                              </span>

                              <!-- Exit code badge -->
                              @if (step.exit_code !== null && step.exit_code !== 0) {
                                <span class="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono font-medium bg-ctp-red/20 text-ctp-red">
                                  {{ t('pipelines.detail.exitCode') }} {{ step.exit_code }}
                                </span>
                              }

                              <!-- Step duration -->
                              <span class="text-xs font-mono text-text-muted">
                                {{ computeDuration(step.started_at, step.finished_at) }}
                              </span>
                            </div>
                          }
                        </div>
                      }
                    }
                  </div>
                }
              </div>
            }
          </div>
        }
      }
    </div>
  `,
})
export class RunDetailPage implements OnDestroy {
  private ciApi = inject(CiApiService);
  private ctx = inject(ProjectContext);
  private route = inject(ActivatedRoute);
  private destroyRef = inject(DestroyRef);

  // State
  run = signal<CiRunWithJobs | null>(null);
  loading = signal(false);
  error = signal<string | null>(null);

  // Job expansion
  expandedJobs = signal<Set<string>>(new Set());
  loadingJobs = signal<Set<string>>(new Set());
  jobSteps = signal<Map<string, CiStep[]>>(new Map());

  // Polling
  isPolling = signal(false);
  private pollSub: Subscription | null = null;

  private runId = '';

  constructor() {
    this.route.paramMap
      .pipe(takeUntilDestroyed(this.destroyRef))
      .subscribe(params => {
        this.runId = params.get('runId') ?? '';
        if (this.runId) {
          this.loadRun();
        }
      });
  }

  ngOnDestroy(): void {
    this.stopPolling();
  }

  statusColor(status: string): string {
    return CI_STATUS_COLORS[status] ?? 'bg-ctp-overlay0/20 text-ctp-overlay0';
  }

  providerBadgeClass(provider: string): string {
    switch (provider) {
      case 'github':
        return 'bg-ctp-mauve/15 text-ctp-mauve border border-ctp-mauve/30';
      case 'forgejo':
        return 'bg-ctp-peach/15 text-ctp-peach border border-ctp-peach/30';
      default:
        return 'bg-ctp-overlay0/15 text-ctp-overlay0 border border-ctp-overlay0/30';
    }
  }

  toggleJob(jobId: string): void {
    const current = new Set(this.expandedJobs());
    if (current.has(jobId)) {
      current.delete(jobId);
    } else {
      current.add(jobId);
      // Load steps if not already loaded
      if (!this.jobSteps().has(jobId)) {
        this.loadJobSteps(jobId);
      }
    }
    this.expandedJobs.set(current);
  }

  loadRun(): void {
    const pid = this.ctx.projectId();
    if (!pid || !this.runId) return;

    this.loading.set(true);
    this.error.set(null);

    this.ciApi.getRun(pid, this.runId)
      .pipe(takeUntilDestroyed(this.destroyRef))
      .subscribe({
        next: (res: CiRunWithJobs) => {
          this.run.set(res);
          this.loading.set(false);

          // Refresh steps for any expanded jobs
          for (const jobId of this.expandedJobs()) {
            this.loadJobSteps(jobId);
          }

          // Start or stop polling based on run status
          const isRunning = res.status === 'running' || res.status === 'pending' ||
            res.jobs.some(j => j.status === 'running' || j.status === 'pending');
          if (isRunning) {
            this.startPolling();
          } else {
            this.stopPolling();
          }
        },
        error: (err) => {
          this.loading.set(false);
          this.error.set(err?.error?.message ?? err?.message ?? 'Failed to load run');
          this.stopPolling();
        },
      });
  }

  computeDuration(startedAt: string | null, finishedAt: string | null): string {
    if (!startedAt) return '—';

    const start = new Date(startedAt).getTime();
    const end = finishedAt ? new Date(finishedAt).getTime() : Date.now();
    const diffMs = end - start;

    if (diffMs < 0) return '—';

    const seconds = Math.floor(diffMs / 1000);
    if (seconds < 60) return `${seconds}s`;

    const minutes = Math.floor(seconds / 60);
    const remainSec = seconds % 60;
    if (minutes < 60) return `${minutes}m ${remainSec}s`;

    const hours = Math.floor(minutes / 60);
    const remainMin = minutes % 60;
    return `${hours}h ${remainMin}m`;
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

  private loadJobSteps(jobId: string): void {
    const pid = this.ctx.projectId();
    if (!pid || !this.runId) return;

    const currentLoading = new Set(this.loadingJobs());
    currentLoading.add(jobId);
    this.loadingJobs.set(currentLoading);

    this.ciApi.getJob(pid, this.runId, jobId)
      .pipe(takeUntilDestroyed(this.destroyRef))
      .subscribe({
        next: (res: CiJobWithSteps) => {
          const steps = new Map(this.jobSteps());
          steps.set(jobId, res.steps);
          this.jobSteps.set(steps);

          const loading = new Set(this.loadingJobs());
          loading.delete(jobId);
          this.loadingJobs.set(loading);
        },
        error: () => {
          const loading = new Set(this.loadingJobs());
          loading.delete(jobId);
          this.loadingJobs.set(loading);

          // Set empty steps so the UI shows the "no steps" message
          const steps = new Map(this.jobSteps());
          steps.set(jobId, []);
          this.jobSteps.set(steps);
        },
      });
  }

  private startPolling(): void {
    if (this.pollSub) return;

    this.isPolling.set(true);
    this.pollSub = timer(15_000, 15_000)
      .pipe(takeUntilDestroyed(this.destroyRef))
      .subscribe(() => this.loadRun());
  }

  private stopPolling(): void {
    this.isPolling.set(false);
    this.pollSub?.unsubscribe();
    this.pollSub = null;
  }
}
