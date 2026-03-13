import { Component, inject, signal, DestroyRef } from '@angular/core';
import { DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute, Router } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { forkJoin } from 'rxjs';
import {
  CdkDragDrop,
  CdkDrag,
  CdkDragHandle,
  CdkDragPlaceholder,
  CdkDropList,
  moveItemInArray,
} from '@angular/cdk/drag-drop';
import {
  PlansApiService,
  SpPlan,
  PlanStatus,
  SpPlanProgress,
} from '../../../../core/services/plans-api.service';
import { SpTask } from '../../../../core/services/tasks-api.service';
import { taskStateColor } from '../../../../shared/ui-constants';

const STATUS_COLORS: Record<PlanStatus, string> = {
  active: 'bg-ctp-green/20 text-ctp-green',
  completed: 'bg-ctp-blue/20 text-ctp-blue',
  cancelled: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

const PROGRESS_COLORS: Record<PlanStatus, string> = {
  active: 'bg-ctp-green',
  completed: 'bg-ctp-blue',
  cancelled: 'bg-ctp-overlay0',
};

@Component({
  selector: 'app-plan-detail',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, CdkDrag, CdkDragHandle, CdkDragPlaceholder, CdkDropList],
  styles: [`
    .cdk-drag-animating {
      transition: transform 250ms cubic-bezier(0, 0, 0.2, 1);
    }
    .cdk-drop-list-dragging .cdk-drag:not(.cdk-drag-placeholder) {
      transition: transform 250ms cubic-bezier(0, 0, 0.2, 1);
    }
  `],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Back link + header -->
      <div class="flex items-center gap-3 mb-4">
        <button (click)="goBack()" class="text-text-secondary hover:text-text-primary transition-colors">
          <svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" />
          </svg>
        </button>
        @if (plan(); as p) {
          <h1 class="text-2xl font-semibold text-text-primary flex-1">{{ p.title }}</h1>
          <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ statusColor(p.status) }}">
            {{ t('plans.status.' + p.status) }}
          </span>
        }
      </div>

      @if (loading()) {
        <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
      } @else if (plan(); as p) {
        <!-- Plan metadata -->
        <div class="bg-surface border border-border rounded-lg p-4 mb-4">
          @if (p.description) {
            <p class="text-sm text-text-secondary mb-3 whitespace-pre-wrap">{{ p.description }}</p>
          }

          <!-- Progress -->
          @if (progress(); as prog) {
            <div class="mb-3">
              <div class="flex items-center justify-between text-xs text-text-secondary mb-1">
                <span>{{ t('plans.progress') }}: {{ prog.done_tasks }}/{{ prog.total_tasks }} {{ t('plans.tasks') }}</span>
                <div class="flex items-center gap-3">
                  @if (prog.working_tasks > 0) {
                    <span class="text-ctp-yellow">{{ prog.working_tasks }} {{ t('plans.inProgress') }}</span>
                  }
                  @if (prog.cancelled_tasks > 0) {
                    <span class="text-ctp-overlay0">{{ prog.cancelled_tasks }} {{ t('plans.cancelled') }}</span>
                  }
                  @if (prog.total_tasks > 0) {
                    <span>{{ progressPercent(prog) }}%</span>
                  }
                </div>
              </div>
              <div class="w-full bg-surface-hover rounded-full h-2">
                <div class="{{ progressBarColor() }} h-2 rounded-full transition-all"
                  [style.width.%]="progressPercent(prog)"></div>
              </div>
            </div>
          }

          <div class="flex items-center gap-3 text-xs text-text-secondary">
            <span>{{ t('plans.updatedAt') }}: {{ p.updated_at | date:'short' }}</span>
            <span>{{ t('plans.createdAt') }}: {{ p.created_at | date:'short' }}</span>
          </div>
        </div>

        <!-- Tasks section -->
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-lg font-semibold text-text-primary">{{ t('plans.linkedTasks') }}</h2>
          <button (click)="showAddTask.set(true)"
            class="px-3 py-1.5 text-sm font-medium bg-accent text-bg rounded-lg hover:opacity-90">
            {{ t('plans.addTask') }}
          </button>
        </div>

        @if (tasks().length === 0) {
          <p class="text-text-secondary text-sm bg-surface border border-border rounded-lg p-4">
            {{ t('plans.noTasks') }}
          </p>
        } @else {
          <div cdkDropList (cdkDropListDropped)="onTaskDrop($event)" class="space-y-1">
            @for (task of tasks(); track task.id) {
              <div cdkDrag
                class="bg-surface border border-border rounded-lg p-3 flex items-center gap-3 hover:border-accent/50 transition-colors">
                <!-- Drag handle -->
                <div cdkDragHandle class="cursor-grab text-text-secondary hover:text-text-primary">
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M4 8h16M4 16h16" />
                  </svg>
                </div>

                <!-- Drag placeholder -->
                <div *cdkDragPlaceholder class="bg-accent/10 border-2 border-dashed border-accent rounded-lg h-12"></div>

                <!-- Task number -->
                <span class="text-xs text-text-secondary font-mono shrink-0">#{{ task.number }}</span>

                <!-- State badge -->
                <span class="px-1.5 py-0.5 rounded-full text-[10px] font-medium shrink-0 {{ stateColor(task.state) }}">
                  {{ task.state }}
                </span>

                <!-- Title -->
                <span class="text-sm text-text-primary flex-1 truncate">{{ task.title }}</span>

                <!-- Kind -->
                <span class="text-xs text-text-secondary shrink-0">{{ task.kind }}</span>

                <!-- Remove button -->
                <button (click)="removeTask(task)" class="text-ctp-red/60 hover:text-ctp-red transition-colors shrink-0">
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            }
          </div>
        }

        <!-- Add task modal -->
        @if (showAddTask()) {
          <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-[70]"
            role="dialog" aria-modal="true" tabindex="-1"
            (click)="showAddTask.set(false)" (keydown.escape)="showAddTask.set(false)">
            <div class="bg-bg border border-border rounded-xl p-6 w-full max-w-md"
              role="document" tabindex="-1"
              (click)="$event.stopPropagation()" (keydown)="$event.stopPropagation()">
              <h3 class="text-lg font-semibold text-text-primary mb-4">{{ t('plans.addTask') }}</h3>
              <div class="space-y-3">
                <div>
                  <label for="addTaskIdInput" class="block text-sm font-medium text-text-primary mb-1">{{ t('plans.taskId') }}</label>
                  <input id="addTaskIdInput" type="text" [(ngModel)]="addTaskId"
                    [placeholder]="t('plans.taskIdPlaceholder')"
                    class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                </div>
                <div class="flex justify-end gap-3">
                  <button (click)="showAddTask.set(false)"
                    class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                    {{ t('plans.cancel') }}
                  </button>
                  <button (click)="addTask()"
                    [disabled]="!addTaskId.trim()"
                    class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90
                           disabled:opacity-50 disabled:cursor-not-allowed">
                    {{ t('plans.add') }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        }
      }
    </div>
  `,
})
export class PlanDetailPage {
  private plansApi = inject(PlansApiService);
  private route = inject(ActivatedRoute);
  private router = inject(Router);
  private destroyRef = inject(DestroyRef);

  plan = signal<SpPlan | null>(null);
  tasks = signal<SpTask[]>([]);
  progress = signal<SpPlanProgress | null>(null);
  loading = signal(true);
  showAddTask = signal(false);
  addTaskId = '';

  protected readonly stateColor = taskStateColor;

  constructor() {
    this.route.params.pipe(takeUntilDestroyed(this.destroyRef)).subscribe(params => {
      const id = params['id'];
      if (id) this.loadPlan(id);
    });
  }

  private loadPlan(id: string): void {
    this.loading.set(true);
    forkJoin({
      plan: this.plansApi.get(id),
      tasks: this.plansApi.listTasks(id, { limit: 100 }),
      progress: this.plansApi.progress(id),
    }).subscribe({
      next: ({ plan, tasks, progress }) => {
        this.plan.set(plan);
        this.tasks.set(tasks.data);
        this.progress.set(progress);
        this.loading.set(false);
      },
      error: () => {
        this.loading.set(false);
      },
    });
  }

  statusColor(status: string): string {
    return STATUS_COLORS[status as PlanStatus] ?? 'bg-ctp-overlay0/20 text-ctp-overlay0';
  }

  progressBarColor(): string {
    const p = this.plan();
    return p ? (PROGRESS_COLORS[p.status as PlanStatus] ?? 'bg-ctp-overlay0') : 'bg-ctp-overlay0';
  }

  progressPercent(prog: SpPlanProgress): number {
    if (prog.total_tasks === 0) return 0;
    return Math.round((prog.done_tasks / prog.total_tasks) * 100);
  }

  onTaskDrop(event: CdkDragDrop<SpTask[]>): void {
    const list = [...this.tasks()];
    moveItemInArray(list, event.previousIndex, event.currentIndex);
    this.tasks.set(list);
    // Persist new order
    const taskIds = list.map(t => t.id);
    const planId = this.plan()?.id;
    if (planId) {
      this.plansApi.reorderTasks(planId, taskIds).subscribe();
    }
  }

  addTask(): void {
    const planId = this.plan()?.id;
    const taskId = this.addTaskId.trim();
    if (!planId || !taskId) return;
    this.plansApi.addTask(planId, taskId).subscribe({
      next: () => {
        this.addTaskId = '';
        this.showAddTask.set(false);
        this.loadPlan(planId);
      },
    });
  }

  removeTask(task: SpTask): void {
    const planId = this.plan()?.id;
    if (!planId) return;
    this.plansApi.removeTask(planId, task.id).subscribe({
      next: () => this.loadPlan(planId),
    });
  }

  goBack(): void {
    this.router.navigate(['/plans']);
  }
}
