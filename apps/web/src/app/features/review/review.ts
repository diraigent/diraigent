import { Component, inject, signal, OnDestroy } from '@angular/core';
import { DatePipe, SlicePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { forkJoin, of, Subscription } from 'rxjs';
import { catchError, map, switchMap } from 'rxjs/operators';
import { TasksApiService, SpTask, SpTaskUpdate } from '../../core/services/tasks-api.service';
import { DiraigentApiService, DgProject } from '../../core/services/diraigent-api.service';
import { ReviewSseService } from '../../core/services/review-sse.service';
import { NavBadgeService } from '../../core/services/nav-badge.service';
import { ObservationsPage } from '../observations/observations';

export interface ReviewTask {
  task: SpTask;
  project: DgProject;
  artifacts: SpTaskUpdate[];
  acceptanceCriteria: string[];
  expanded: boolean;
}

@Component({
  selector: 'app-review',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, SlicePipe, ObservationsPage],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.review') }}</h1>
      </div>

      <!-- Tabs -->
      <div class="flex items-center gap-1 border-b border-border mb-4">
        <button (click)="activeTab.set('review')"
          class="px-4 py-2 text-sm font-medium transition-colors relative"
          [class.text-accent]="activeTab() === 'review'"
          [class.text-text-secondary]="activeTab() !== 'review'"
          [class.hover:text-text-primary]="activeTab() !== 'review'">
          {{ t('review.tabReview') }}
          @if (reviewItems().length > 0) {
            <span class="ml-1.5 min-w-[1.25rem] h-5 px-1 rounded-full bg-ctp-yellow/20 text-ctp-yellow text-[10px] font-semibold inline-flex items-center justify-center leading-none">
              {{ reviewItems().length }}
            </span>
          }
          @if (activeTab() === 'review') {
            <span class="absolute bottom-0 left-0 right-0 h-0.5 bg-accent rounded-t"></span>
          }
        </button>
        <button (click)="activeTab.set('observations')"
          class="px-4 py-2 text-sm font-medium transition-colors relative"
          [class.text-accent]="activeTab() === 'observations'"
          [class.text-text-secondary]="activeTab() !== 'observations'"
          [class.hover:text-text-primary]="activeTab() !== 'observations'">
          {{ t('review.tabObservations') }}
          @if (badges.openObservations() > 0) {
            <span class="ml-1.5 min-w-[1.25rem] h-5 px-1 rounded-full bg-ctp-red/20 text-ctp-red text-[10px] font-semibold inline-flex items-center justify-center leading-none">
              {{ badges.openObservations() > 99 ? '99+' : badges.openObservations() }}
            </span>
          }
          @if (activeTab() === 'observations') {
            <span class="absolute bottom-0 left-0 right-0 h-0.5 bg-accent rounded-t"></span>
          }
        </button>
      </div>

      @if (activeTab() === 'observations') {
        <app-observations />
      } @else {

      <!-- Review tab content -->
      <div class="flex items-center justify-end mb-4">
        <button (click)="load()" [disabled]="loading()"
          class="flex items-center gap-2 px-3 py-2 text-sm text-text-secondary hover:text-text-primary border border-border rounded-lg hover:bg-surface transition-colors disabled:opacity-50">
          <svg class="w-4 h-4" [class.animate-spin]="loading()" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
          </svg>
          {{ t('review.refresh') }}
        </button>
      </div>

      @if (loading() && reviewItems().length === 0) {
        <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
      } @else if (reviewItems().length === 0) {
        <!-- Empty state -->
        <div class="flex flex-col items-center justify-center py-24 text-center">
          <svg class="w-16 h-16 text-text-secondary/30 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"/>
          </svg>
          <p class="text-lg font-medium text-text-secondary">{{ t('review.emptyTitle') }}</p>
          <p class="text-sm text-text-secondary/60 mt-1">{{ t('review.emptyDescription') }}</p>
        </div>
      } @else {
        <div class="space-y-4">
          @for (item of reviewItems(); track item.task.id) {
            <div class="bg-surface border border-border rounded-xl overflow-hidden">
              <!-- Card header -->
              <div class="p-5">
                <div class="flex items-start justify-between gap-4">
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-2 mb-1">
                      <span class="text-xs font-medium px-2 py-0.5 rounded-full bg-ctp-yellow/15 text-ctp-yellow">
                        {{ t('review.awaitingReview') }}
                      </span>
                      <span class="text-xs text-text-secondary">
                        {{ item.project.name }} · #{{ item.task.number }}
                      </span>
                      @if (item.task.kind) {
                        <span class="text-xs px-1.5 py-0.5 rounded bg-surface-hover text-text-secondary">
                          {{ item.task.kind }}
                        </span>
                      }
                    </div>
                    <h2 class="text-base font-semibold text-text-primary">{{ item.task.title }}</h2>
                    @if (taskDescription(item.task)) {
                      <p class="text-sm text-text-secondary mt-1 line-clamp-2">{{ taskDescription(item.task) }}</p>
                    }
                    @if (item.task.playbook_step !== null && item.task.playbook_step !== undefined) {
                      <p class="text-xs text-text-secondary mt-1">
                        {{ t('review.completedStep') }}: {{ item.task.playbook_step + 1 }}
                        @if (item.task.playbook_id) {
                          <span class="opacity-60"> ({{ item.task.playbook_id | slice:0:8 }})</span>
                        }
                      </p>
                    }
                  </div>
                  <button (click)="toggleExpand(item)"
                    class="p-1.5 text-text-secondary hover:text-text-primary rounded transition-colors shrink-0">
                    <svg class="w-5 h-5 transition-transform" [class.rotate-180]="item.expanded"
                      fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                    </svg>
                  </button>
                </div>

                <!-- Action buttons -->
                <div class="flex items-center gap-2 mt-4">
                  <button (click)="approve(item)"
                    [disabled]="actioning() === item.task.id"
                    class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-lg
                           bg-ctp-green/20 text-ctp-green hover:bg-ctp-green/30 transition-colors
                           disabled:opacity-50 disabled:cursor-not-allowed">
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7"/>
                    </svg>
                    {{ t('review.approve') }}
                  </button>
                  <button (click)="openRework(item)"
                    [disabled]="actioning() === item.task.id"
                    class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-lg
                           bg-ctp-yellow/20 text-ctp-yellow hover:bg-ctp-yellow/30 transition-colors
                           disabled:opacity-50 disabled:cursor-not-allowed">
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
                    </svg>
                    {{ t('review.rework') }}
                  </button>
                  <button (click)="reopen(item)"
                    [disabled]="actioning() === item.task.id"
                    class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-lg
                           bg-ctp-overlay0/15 text-ctp-overlay0 hover:bg-ctp-overlay0/25 transition-colors
                           disabled:opacity-50 disabled:cursor-not-allowed">
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"/>
                    </svg>
                    {{ t('review.reopen') }}
                  </button>
                  @if (actioning() === item.task.id) {
                    <span class="text-xs text-text-secondary">{{ t('common.saving') }}</span>
                  }
                </div>

                <!-- Rework comment input -->
                @if (reworkTarget() === item.task.id) {
                  <div class="mt-3 flex gap-2">
                    <input type="text" [(ngModel)]="reworkComment"
                      [placeholder]="t('review.reworkCommentPlaceholder')"
                      class="flex-1 bg-bg text-text-primary text-sm rounded-lg px-3 py-1.5 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent" />
                    <button (click)="submitRework(item)"
                      class="px-3 py-1.5 text-xs font-medium bg-ctp-yellow/20 text-ctp-yellow rounded-lg hover:bg-ctp-yellow/30 transition-colors">
                      {{ t('review.sendRework') }}
                    </button>
                    <button (click)="cancelRework()"
                      class="px-3 py-1.5 text-xs text-text-secondary hover:text-text-primary rounded-lg hover:bg-surface-hover transition-colors">
                      {{ t('review.cancel') }}
                    </button>
                  </div>
                }
              </div>

              <!-- Expanded details -->
              @if (item.expanded) {
                <div class="border-t border-border px-5 py-4 space-y-4 bg-bg">
                  <!-- Acceptance criteria -->
                  @if (item.acceptanceCriteria.length > 0) {
                    <div>
                      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">
                        {{ t('review.acceptanceCriteria') }}
                      </h3>
                      <ul class="space-y-1.5">
                        @for (criterion of item.acceptanceCriteria; track criterion) {
                          <li class="flex items-start gap-2 text-sm text-text-primary">
                            <svg class="w-4 h-4 text-ctp-overlay0 mt-0.5 shrink-0" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                              <path stroke-linecap="round" stroke-linejoin="round" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"/>
                            </svg>
                            {{ criterion }}
                          </li>
                        }
                      </ul>
                    </div>
                  }

                  <!-- Agent artifacts -->
                  @if (item.artifacts.length > 0) {
                    <div>
                      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">
                        {{ t('review.artifacts') }} ({{ item.artifacts.length }})
                      </h3>
                      <div class="space-y-2">
                        @for (artifact of item.artifacts; track artifact.id) {
                          <div class="rounded-lg border border-border bg-surface p-3">
                            <div class="flex items-center gap-2 mb-1">
                              <span class="text-xs font-medium px-1.5 py-0.5 rounded bg-ctp-blue/15 text-ctp-blue">
                                artifact
                              </span>
                              <span class="text-xs text-text-secondary">{{ artifact.created_at | date:'short' }}</span>
                            </div>
                            <pre class="text-xs text-text-primary whitespace-pre-wrap break-all leading-relaxed">{{ artifact.content }}</pre>
                          </div>
                        }
                      </div>
                    </div>
                  } @else {
                    <p class="text-xs text-text-secondary">{{ t('review.noArtifacts') }}</p>
                  }

                  <!-- Task metadata -->
                  <div class="text-xs text-text-secondary border-t border-border pt-3">
                    {{ t('review.updatedAt') }}: {{ item.task.updated_at | date:'medium' }}
                    @if (item.task.assigned_agent_id) {
                      · {{ t('review.agent') }}: {{ item.task.assigned_agent_id | slice:0:8 }}
                    }
                  </div>
                </div>
              }
            </div>
          }
        </div>
      }

      } <!-- end review tab -->
    </div>
  `,
})
export class ReviewPage implements OnDestroy {
  private tasksApi = inject(TasksApiService);
  private diraigentApi = inject(DiraigentApiService);
  private reviewSse = inject(ReviewSseService);
  badges = inject(NavBadgeService);

  activeTab = signal<'review' | 'observations'>('review');
  reviewItems = signal<ReviewTask[]>([]);
  loading = signal(false);
  actioning = signal<string | null>(null);
  reworkTarget = signal<string | null>(null);
  reworkComment = '';

  private prevCount = 0;
  private sseSub: Subscription;

  constructor() {
    // Initial load on page open
    this.load();

    // Re-fetch whenever the SSE stream signals a human_review change.
    // This replaces the 30 s polling timer — updates are now instant.
    this.sseSub = this.reviewSse.events.pipe(
      switchMap(() => this.fetchReviewTasks()),
    ).subscribe(items => {
      const newCount = items.length;
      if (newCount > this.prevCount && this.prevCount >= 0 && !document.hasFocus()) {
        this.notify(newCount);
      }
      this.prevCount = newCount;
      const existingMap = new Map(this.reviewItems().map(i => [i.task.id, i.expanded]));
      this.reviewItems.set(
        items.map(item => ({ ...item, expanded: existingMap.get(item.task.id) ?? false })),
      );
    });
  }

  ngOnDestroy(): void {
    this.sseSub.unsubscribe();
  }

  load(): void {
    this.loading.set(true);
    this.fetchReviewTasks().subscribe(items => {
      const newCount = items.length;
      if (newCount > this.prevCount && this.prevCount >= 0 && !document.hasFocus()) {
        this.notify(newCount);
      }
      this.prevCount = newCount;
      const existingMap = new Map(this.reviewItems().map(i => [i.task.id, i.expanded]));
      this.reviewItems.set(
        items.map(item => ({ ...item, expanded: existingMap.get(item.task.id) ?? false })),
      );
      this.loading.set(false);
    });
  }

  toggleExpand(item: ReviewTask): void {
    this.reviewItems.update(items =>
      items.map(i => (i.task.id === item.task.id ? { ...i, expanded: !i.expanded } : i)),
    );
  }

  approve(item: ReviewTask): void {
    this.actioning.set(item.task.id);
    this.tasksApi.transition(item.task.id, 'done').subscribe({
      next: () => {
        this.actioning.set(null);
        this.reviewItems.update(items => items.filter(i => i.task.id !== item.task.id));
        this.prevCount = this.reviewItems().length;
      },
      error: () => this.actioning.set(null),
    });
  }

  openRework(item: ReviewTask): void {
    this.reworkComment = '';
    this.reworkTarget.set(item.task.id);
  }

  cancelRework(): void {
    this.reworkTarget.set(null);
    this.reworkComment = '';
  }

  submitRework(item: ReviewTask): void {
    this.reworkTarget.set(null);
    this.actioning.set(item.task.id);
    const comment = this.reworkComment.trim();
    const transition$ = this.tasksApi.transition(item.task.id, 'ready');
    if (comment) {
      transition$.pipe(
        switchMap(() => this.tasksApi.createComment(item.task.id, { content: comment }).pipe(catchError(() => of(null)))),
      ).subscribe({
        next: () => {
          this.actioning.set(null);
          this.reviewItems.update(items => items.filter(i => i.task.id !== item.task.id));
          this.prevCount = this.reviewItems().length;
        },
        error: () => this.actioning.set(null),
      });
    } else {
      transition$.subscribe({
        next: () => {
          this.actioning.set(null);
          this.reviewItems.update(items => items.filter(i => i.task.id !== item.task.id));
          this.prevCount = this.reviewItems().length;
        },
        error: () => this.actioning.set(null),
      });
    }
    this.reworkComment = '';
  }

  reopen(item: ReviewTask): void {
    this.actioning.set(item.task.id);
    this.tasksApi.transition(item.task.id, 'backlog').subscribe({
      next: () => {
        this.actioning.set(null);
        this.reviewItems.update(items => items.filter(i => i.task.id !== item.task.id));
        this.prevCount = this.reviewItems().length;
      },
      error: () => this.actioning.set(null),
    });
  }

  taskDescription(task: SpTask): string {
    const ctx = task.context as Record<string, unknown>;
    return (ctx?.['spec'] as string) ?? (ctx?.['description'] as string) ?? '';
  }

  private fetchReviewTasks() {
    return this.diraigentApi.getProjects().pipe(
      catchError(() => of([] as import('../../core/services/diraigent-api.service').DgProject[])),
      switchMap(projects => {
        if (!projects.length) return of([] as ReviewTask[]);
        return forkJoin(
          projects.map(project =>
            this.tasksApi.listForProject(project.id, { state: 'human_review', limit: 100 }).pipe(
              catchError(() => of({ data: [] as SpTask[], total: 0, limit: 100, offset: 0, has_more: false })),
              switchMap(resp => {
                const tasks = resp.data;
                if (!tasks.length) return of([] as ReviewTask[]);
                return forkJoin(
                  tasks.map(task =>
                    this.tasksApi.listUpdates(task.id).pipe(
                      catchError(() => of([] as SpTaskUpdate[])),
                      map(updates => ({
                        task,
                        project,
                        artifacts: updates.filter(u => u.kind === 'artifact'),
                        acceptanceCriteria: this.extractCriteria(task),
                        expanded: false,
                      })),
                    ),
                  ),
                );
              }),
            ),
          ),
        ).pipe(map(perProject => perProject.flat()));
      }),
    );
  }

  private extractCriteria(task: SpTask): string[] {
    const ctx = task.context as Record<string, unknown>;
    const raw = ctx?.['acceptance_criteria'];
    if (Array.isArray(raw)) return raw as string[];
    return [];
  }

  private notify(count: number): void {
    try {
      if ('Notification' in window && Notification.permission === 'granted') {
        new Notification('Human Review Queue', {
          body: `${count} task${count !== 1 ? 's' : ''} awaiting review`,
          icon: '/favicon.ico',
        });
      } else if ('Notification' in window && Notification.permission !== 'denied') {
        Notification.requestPermission().then(perm => {
          if (perm === 'granted') {
            new Notification('Human Review Queue', {
              body: `${count} task${count !== 1 ? 's' : ''} awaiting review`,
              icon: '/favicon.ico',
            });
          }
        });
      }
    } catch {
      // Notifications not supported
    }
  }
}
