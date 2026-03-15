import { Component, ChangeDetectorRef, inject, signal, computed, effect, DestroyRef, PLATFORM_ID, ViewEncapsulation } from '@angular/core';
import { NgTemplateOutlet, DatePipe, SlicePipe, isPlatformBrowser } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { Subscription, forkJoin, of, from, timer, switchMap, EMPTY } from 'rxjs';
import { catchError, concatMap, toArray, mergeMap } from 'rxjs/operators';
import {
  CdkDragDrop,
  CdkDrag,
  CdkDragHandle,
  CdkDragPlaceholder,
  CdkDropList,
  moveItemInArray,
} from '@angular/cdk/drag-drop';
import { ProjectContext } from '../../core/services/project-context.service';
import {
  WorkApiService,
  SpWork,
  WorkStatus,
  WorkType,
  WorkTodo,
  SpWorkCreate,
  SpWorkProgress,
  SpWorkStats,
  PlannedTask,
} from '../../core/services/work-api.service';
import {
  TasksApiService,
  SpTask,
  SpTaskUpdate,
  SpTaskComment,
  SpTaskDependencies,
  ChangedFileSummary,
  CreateTaskRequest,
  UpdateTaskRequest,
} from '../../core/services/tasks-api.service';
import { TaskFormComponent } from '../tasks/components/task-form/task-form';
import { TaskListComponent } from '../tasks/pages/task-list/task-list';
import { VerificationsApiService, SpVerification } from '../../core/services/verifications-api.service';
import { PlaybooksApiService, SpPlaybook } from '../../core/services/playbooks-api.service';
import { GitApiService, BranchInfo, MainPushStatus, TaskBranchStatus } from '../../core/services/git-api.service';
import { ChatService } from '../../core/services/chat.service';

const STATUSES: WorkStatus[] = ['active', 'achieved', 'paused', 'abandoned'];

const STATUS_COLORS: Record<WorkStatus, string> = {
  active: 'bg-ctp-green/20 text-ctp-green',
  achieved: 'bg-ctp-blue/20 text-ctp-blue',
  paused: 'bg-ctp-yellow/20 text-ctp-yellow',
  abandoned: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

const PROGRESS_COLORS: Record<WorkStatus, string> = {
  active: 'bg-ctp-green',
  achieved: 'bg-ctp-blue',
  paused: 'bg-ctp-yellow',
  abandoned: 'bg-ctp-overlay0',
};

const GOAL_TYPES: WorkType[] = ['epic', 'feature', 'milestone', 'sprint', 'initiative'];

const TYPE_COLORS: Record<WorkType, string> = {
  epic: 'bg-ctp-mauve/20 text-ctp-mauve',
  feature: 'bg-ctp-blue/20 text-ctp-blue',
  milestone: 'bg-ctp-peach/20 text-ctp-peach',
  sprint: 'bg-ctp-teal/20 text-ctp-teal',
  initiative: 'bg-ctp-lavender/20 text-ctp-lavender',
};

const TASK_STATES = ['backlog', 'ready', 'working', 'done', 'cancelled'];

@Component({
  selector: 'app-work',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, SlicePipe, NgTemplateOutlet, TaskFormComponent, TaskListComponent, CdkDrag, CdkDragHandle, CdkDragPlaceholder, CdkDropList],
  encapsulation: ViewEncapsulation.None,
  styles: [`
    .cdk-drag-animating {
      transition: none !important;
      transform: none !important;
    }
    .cdk-drop-list-dragging .cdk-drag:not(.cdk-drag-placeholder) {
      transition: transform 250ms cubic-bezier(0, 0, 0.2, 1);
    }
  `],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.work') }}</h1>
        <div class="flex items-center gap-3">
          @if (mainStatus(); as ms) {
            @if (ms.ahead > 0 && ms.behind > 0) {
              <button (click)="onResolveAndPushMain()"
                [disabled]="resolvingMain()"
                title="Local main and remote have diverged ({{ ms.ahead }} ahead, {{ ms.behind }} behind). Click to rebase and push."
                class="flex items-center gap-2 px-3 py-2 text-sm font-medium rounded-lg
                       bg-ctp-red/15 text-ctp-red hover:bg-ctp-red/25 transition-colors
                       disabled:opacity-50 disabled:cursor-not-allowed">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M12 9v4m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
                </svg>
                @if (resolvingMain()) {
                  resolving...
                } @else {
                  merge conflict (↑{{ ms.ahead }} ↓{{ ms.behind }})
                }
              </button>
            } @else if (ms.ahead > 0) {
              <button (click)="onPushMain()"
                [disabled]="pushingMain()"
                class="flex items-center gap-2 px-3 py-2 text-sm font-medium rounded-lg
                       bg-ctp-yellow/15 text-ctp-yellow hover:bg-ctp-yellow/25 transition-colors
                       disabled:opacity-50 disabled:cursor-not-allowed">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M5 10l7-7m0 0l7 7m-7-7v18" />
                </svg>
                @if (pushingMain()) {
                  pushing...
                } @else {
                  push to remote ({{ ms.ahead }})
                }
              </button>
            }
          }
          <button (click)="onReleaseProject()"
            [disabled]="releasing()"
            title="Squash-merge dev → main, tag, and push to all remotes"
            class="flex items-center gap-2 px-3 py-2 text-sm font-medium rounded-lg
                   bg-ctp-mauve/15 text-ctp-mauve hover:bg-ctp-mauve/25 transition-colors
                   disabled:opacity-50 disabled:cursor-not-allowed">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
              <path d="M7 7h.01M7 3h5a1.99 1.99 0 011.41.59l7 7a2 2 0 010 2.82l-5 5a2 2 0 01-2.82 0l-7-7A2 2 0 013 10V5a2 2 0 012-2z" />
            </svg>
            @if (releasing()) {
              {{ t('git.releasing') }}
            } @else {
              {{ t('git.release') }}
            }
          </button>
          <button (click)="openCreate()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
            {{ t('goals.create') }}
          </button>
        </div>
      </div>

      <!-- Goal Filters -->
      <div class="flex flex-wrap gap-3 mb-6">
        <input
          type="text"
          [placeholder]="t('goals.searchPlaceholder')"
          [ngModel]="searchQuery()"
          (ngModelChange)="searchQuery.set($event)"
          class="flex-1 min-w-[200px] bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
        <select
          [(ngModel)]="selectedStatus"
          (ngModelChange)="loadGoals()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('goals.allStatuses') }}</option>
          @for (s of statuses; track s) {
            <option [value]="s">{{ t('goals.status.' + s) }}</option>
          }
        </select>
        <select
          [(ngModel)]="selectedWorkType"
          (ngModelChange)="loadGoals()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('goals.allTypes') }}</option>
          @for (gt of goalTypes; track gt) {
            <option [value]="gt">{{ t('goals.type.' + gt) }}</option>
          }
        </select>
      </div>

      <!-- Goal accordion item template (reused across sections) -->
      <ng-template #goalItem let-goal>
        <div class="rounded-lg border transition-colors"
          [class]="goal.id === selected()?.id
            ? 'bg-accent/10 border-accent'
            : 'bg-surface border-border hover:border-accent/50'"
          [class.border-l-4]="goal.status === 'active' || goal.status === 'paused'"
          [class.border-l-ctp-green]="goal.status === 'active'"
          [class.border-l-ctp-yellow]="goal.status === 'paused'"
          [class.opacity-60]="goal.status === 'achieved'">
          <!-- Accordion header -->
          <button (click)="selectItem(goal)" class="w-full text-left p-4">
            <div class="flex items-center gap-2">
              <svg class="w-4 h-4 text-text-secondary shrink-0 transition-transform duration-200"
                [class.rotate-90]="goal.id === selected()?.id"
                fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
              </svg>
              <span class="text-sm font-medium text-text-primary">{{ goal.title }}</span>
              <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ typeColor(goal.work_type) }}">
                {{ t('goals.type.' + goal.work_type) }}
              </span>
              <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ statusColor(goal.status) }}">
                {{ t('goals.status.' + goal.status) }}
              </span>
            </div>
            @if (goal.id !== selected()?.id) {
              @if (goal.description) {
                <p class="text-xs text-text-secondary line-clamp-1 mt-1 ml-6">{{ goal.description }}</p>
              }
              <!-- Progress bar (collapsed) -->
              @if (progressMap().get(goal.id); as prog) {
                <div class="mt-2 ml-6">
                  <div class="flex items-center justify-between text-xs text-text-secondary mb-1">
                    <span class="flex items-center gap-2">
                      <span>{{ prog.done_tasks }}/{{ prog.total_tasks }} {{ t('goals.tasks') }}</span>
                      @if (statsMap().get(goal.id); as stats) {
                        @if (stats.working_count > 0) {
                          <span class="inline-flex items-center gap-1 text-ctp-yellow">
                            <span class="relative flex h-2 w-2">
                              <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-ctp-yellow opacity-75"></span>
                              <span class="relative inline-flex rounded-full h-2 w-2 bg-ctp-yellow"></span>
                            </span>
                            {{ stats.working_count }} in progress
                          </span>
                        }
                      }
                    </span>
                    <span>{{ prog.percentage }}%</span>
                  </div>
                  <div class="h-1.5 bg-surface-hover rounded-full overflow-hidden flex">
                    <div class="h-full transition-all {{ progressColor(goal.status) }}"
                         [class.rounded-l-full]="true"
                         [class.rounded-r-full]="!statsMap().get(goal.id)?.working_count"
                         [style.width.%]="prog.percentage"></div>
                    @if (statsMap().get(goal.id); as stats) {
                      @if (stats.working_count > 0 && stats.total_count > 0) {
                        <div class="h-full bg-ctp-yellow/60 transition-all"
                             [class.rounded-r-full]="true"
                             [style.width.%]="(stats.working_count / stats.total_count) * 100"></div>
                      }
                    }
                  </div>
                </div>
              }
            }
          </button>

          <!-- Expanded detail (inline) -->
          @if (goal.id === selected()?.id) {
            <div class="px-4 pb-4 pt-0 border-t border-border/50 mt-0">
              <!-- Inline title edit -->
              <div class="pt-3 mb-3">
                <input type="text" [(ngModel)]="formTitle" (blur)="saveInlineField()"
                  class="w-full bg-surface text-text-primary text-sm font-medium rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent"
                  [placeholder]="t('goals.fieldTitle')" />
              </div>

              <!-- Actions row -->
              <div class="flex items-center gap-2 mb-3 flex-wrap">
                <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ statusColor(goal.status) }}">
                  {{ t('goals.status.' + goal.status) }}
                </span>
                <select [(ngModel)]="formWorkType" (change)="saveInlineField()"
                  class="text-xs rounded-lg px-2 py-1 border border-border bg-surface text-text-primary
                         focus:outline-none focus:ring-1 focus:ring-accent">
                  @for (gt of goalTypes; track gt) {
                    <option [value]="gt">{{ t('goals.type.' + gt) }}</option>
                  }
                </select>
                <div class="flex items-center gap-1">
                  <span class="text-xs text-text-secondary">{{ t('goals.fieldPriority') }}</span>
                  <input type="number" [(ngModel)]="formPriority" (blur)="saveInlineField()"
                    class="w-14 text-xs rounded-lg px-2 py-1 border border-border bg-surface text-text-primary
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                </div>
                <label class="flex items-center gap-1 text-xs text-text-secondary cursor-pointer">
                  <input type="checkbox" [(ngModel)]="formAutoStatus" (change)="saveInlineField()"
                    class="rounded border-border text-accent focus:ring-accent" />
                  {{ t('goals.autoStatus') }}
                </label>
                <div class="flex gap-2 ml-auto">
                  <button (click)="confirmDelete(goal)" class="p-1.5 text-text-secondary hover:text-ctp-red rounded" [title]="t('goals.delete')">
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                      <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                    </svg>
                  </button>
                </div>
              </div>

              <!-- Status transitions -->
              @if (availableTransitions().length > 0) {
                <div class="flex flex-wrap gap-2 mb-4">
                  @for (s of availableTransitions(); track s) {
                    <button (click)="transitionStatus(goal, s)"
                      class="px-3 py-1 text-xs rounded-lg border border-border text-text-secondary hover:text-text-primary hover:border-accent/50 transition-colors">
                      {{ t('goals.transitionTo') }} {{ t('goals.status.' + s) }}
                    </button>
                  }
                </div>
              }

              <!-- Progress -->
              @if (progressMap().get(goal.id); as prog) {
                <div class="mb-4">
                  <div class="flex items-center justify-between text-sm text-text-secondary mb-1">
                    <span class="flex items-center gap-2">
                      <span>{{ t('goals.progress') }}</span>
                      @if (statsMap().get(goal.id); as stats) {
                        @if (stats.working_count > 0) {
                          <span class="inline-flex items-center gap-1 text-ctp-yellow text-xs">
                            <span class="relative flex h-2 w-2">
                              <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-ctp-yellow opacity-75"></span>
                              <span class="relative inline-flex rounded-full h-2 w-2 bg-ctp-yellow"></span>
                            </span>
                            {{ stats.working_count }} in progress
                          </span>
                        }
                      }
                    </span>
                    <span>{{ prog.done_tasks }}/{{ prog.total_tasks }} ({{ prog.percentage }}%)</span>
                  </div>
                  <div class="h-2 bg-surface-hover rounded-full overflow-hidden flex">
                    <div class="h-full transition-all {{ progressColor(goal.status) }}"
                         [class.rounded-l-full]="true"
                         [class.rounded-r-full]="!statsMap().get(goal.id)?.working_count"
                         [style.width.%]="prog.percentage"></div>
                    @if (statsMap().get(goal.id); as stats) {
                      @if (stats.working_count > 0 && stats.total_count > 0) {
                        <div class="h-full bg-ctp-yellow/60 transition-all"
                             [class.rounded-r-full]="true"
                             [style.width.%]="(stats.working_count / stats.total_count) * 100"></div>
                      }
                    }
                  </div>
                </div>
              }
              <!-- Children -->
              @if (childrenMap().get(goal.id); as children) {
                @if (children.length > 0) {
                  <div class="mb-4">
                    <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{{ t('goals.children') }} ({{ children.length }})</h3>
                    <div class="space-y-1">
                      @for (child of children; track child.id) {
                        <button (click)="selectItem(child)" class="w-full text-left text-xs p-2 rounded bg-surface-hover hover:bg-accent/10 flex items-center justify-between"
                          [class.border-l-4]="child.status === 'active' || child.status === 'paused'"
                          [class.border-l-ctp-green]="child.status === 'active'"
                          [class.border-l-ctp-yellow]="child.status === 'paused'"
                          [class.opacity-60]="child.status === 'achieved'">
                          <span class="text-text-primary">{{ child.title }}</span>
                          <span class="px-1.5 py-0.5 rounded-full text-xs {{ statusColor(child.status) }}">{{ t('goals.status.' + child.status) }}</span>
                        </button>
                      }
                    </div>
                  </div>
                }
              }

              <!-- Todos -->
              <div class="mb-4">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('goals.todos') }}</h3>
                <div class="flex gap-2 mb-2">
                  <input type="text" [(ngModel)]="newTodoText" [placeholder]="t('goals.todoPlaceholder')"
                    class="flex-1 bg-surface text-text-primary text-xs rounded px-2 py-1.5 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary"
                    (keydown.enter)="addTodo()" />
                  <button (click)="addTodo()" [disabled]="!newTodoText.trim()"
                    class="px-3 py-1.5 bg-accent text-bg rounded text-xs font-medium hover:opacity-90 disabled:opacity-30">
                    {{ t('goals.addTodo') }}
                  </button>
                </div>
                @if (goalTodos().length > 0) {
                  <div class="space-y-1">
                    @for (todo of goalTodos(); track todo.id) {
                      <div class="flex items-center gap-2 group">
                        <input type="checkbox" [checked]="todo.done" (change)="toggleTodo(todo.id)"
                          class="rounded border-border text-accent focus:ring-accent shrink-0" />
                        <span class="text-sm text-text-primary flex-1" [class.line-through]="todo.done" [class.opacity-50]="todo.done">{{ todo.text }}</span>
                        <button (click)="removeTodo(todo.id)"
                          class="p-0.5 text-text-secondary hover:text-ctp-red opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
                          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                            <path d="M6 18L18 6M6 6l12 12" />
                          </svg>
                        </button>
                      </div>
                    }
                  </div>
                  @if (hasDoneTodos()) {
                    <button (click)="clearDoneTodos()" class="mt-1 text-xs text-text-secondary hover:text-accent">
                      {{ t('goals.clearDone') }}
                    </button>
                  }
                } @else {
                  <p class="text-xs text-text-muted">{{ t('goals.todosEmpty') }}</p>
                }
              </div>

              <!-- Description -->
              <div class="mb-4">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('goals.fieldDescription') }}</h3>
                <textarea [(ngModel)]="formDescription" (blur)="saveInlineField()" rows="12"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent resize-y"
                  [placeholder]="t('goals.fieldDescription')"></textarea>
              </div>

              <!-- Success criteria -->
              <div class="mb-4">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('goals.fieldCriteria') }}</h3>
                <textarea [(ngModel)]="formCriteria" (blur)="saveInlineField()" rows="3"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent resize-y"
                  [placeholder]="t('goals.fieldCriteria')"></textarea>
              </div>

              <!-- Stats (clickable to filter linked tasks) -->
              @if (statsMap().get(goal.id); as stats) {
                <div class="mb-4">
                  <div class="flex items-center justify-between mb-2">
                    <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider">{{ t('goals.stats') }}</h3>
                    <div class="flex items-center gap-2">
                      @if (stats.total_cost_usd > 0) {
                        <span class="text-xs text-text-secondary">{{ '$' + stats.total_cost_usd.toFixed(2) }}</span>
                      }
                      @if (statsFilter()) {
                        <button (click)="clearStatsFilter()" class="text-xs text-accent hover:underline">
                          {{ t('common.clearFilter') || 'Clear' }}
                        </button>
                      }
                    </div>
                  </div>
                  <div class="flex flex-wrap gap-1.5 text-xs">
                    <button (click)="setStatsFilter('backlog')"
                      class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 transition-colors cursor-pointer"
                      [class]="statsFilter() === 'backlog'
                        ? 'bg-accent/20 ring-1 ring-accent'
                        : 'bg-surface-hover hover:bg-accent/10'">
                      <span class="text-text-secondary">Backlog</span>
                      <span class="text-text-primary font-medium">{{ stats.backlog_count }}</span>
                    </button>
                    <button (click)="setStatsFilter('ready')"
                      class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 transition-colors cursor-pointer"
                      [class]="statsFilter() === 'ready'
                        ? 'bg-ctp-blue/20 ring-1 ring-ctp-blue'
                        : stats.ready_count > 0
                          ? 'bg-ctp-blue/10 hover:bg-ctp-blue/20'
                          : 'bg-surface-hover hover:bg-accent/10'">
                      <span [class]="stats.ready_count > 0 ? 'text-ctp-blue' : 'text-text-secondary'">Ready</span>
                      <span class="font-medium" [class]="stats.ready_count > 0 ? 'text-ctp-blue' : 'text-text-primary'">{{ stats.ready_count }}</span>
                    </button>
                    <button (click)="setStatsFilter('working')"
                      class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 transition-colors cursor-pointer"
                      [class]="statsFilter() === 'working'
                        ? 'bg-ctp-yellow/20 ring-1 ring-ctp-yellow'
                        : stats.working_count > 0
                          ? 'bg-ctp-yellow/10 hover:bg-ctp-yellow/20'
                          : 'bg-surface-hover hover:bg-accent/10'">
                      <span [class]="stats.working_count > 0 ? 'text-ctp-yellow' : 'text-text-secondary'">Working</span>
                      <span class="font-medium" [class]="stats.working_count > 0 ? 'text-ctp-yellow' : 'text-text-primary'">
                        {{ stats.working_count }}
                        @if (stats.working_count > 0) {
                          <span class="relative inline-flex h-1.5 w-1.5 ml-0.5 -translate-y-px">
                            <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-ctp-yellow opacity-75"></span>
                            <span class="relative inline-flex rounded-full h-1.5 w-1.5 bg-ctp-yellow"></span>
                          </span>
                        }
                      </span>
                    </button>
                    <button (click)="setStatsFilter('done')"
                      class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 transition-colors cursor-pointer"
                      [class]="statsFilter() === 'done'
                        ? 'bg-ctp-green/20 ring-1 ring-ctp-green'
                        : 'bg-surface-hover hover:bg-accent/10'">
                      <span class="text-text-secondary">Done</span>
                      <span class="text-ctp-green font-medium">{{ stats.done_count }}</span>
                    </button>
                    <button (click)="setStatsFilter('cancelled')"
                      class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 transition-colors cursor-pointer"
                      [class]="statsFilter() === 'cancelled'
                        ? 'bg-accent/20 ring-1 ring-accent'
                        : 'bg-surface-hover hover:bg-accent/10'">
                      <span class="text-text-secondary">Cancelled</span>
                      <span class="text-text-primary font-medium">{{ stats.cancelled_count }}</span>
                    </button>
                    @if (stats.blocked_count > 0) {
                      <button (click)="setStatsFilter('blocked')"
                        class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 transition-colors cursor-pointer"
                        [class]="statsFilter() === 'blocked'
                          ? 'bg-ctp-red/20 ring-1 ring-ctp-red'
                          : 'bg-ctp-red/10 hover:bg-ctp-red/20'">
                        <span class="text-ctp-red">Blocked</span>
                        <span class="text-ctp-red font-medium">{{ stats.blocked_count }}</span>
                      </button>
                    }
                  </div>
                </div>
              }

              <!-- Linked Tasks -->
              <div class="pt-3 border-t border-border mb-3">
                <div class="flex items-center justify-between mb-2">
                  <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider">{{ t('goals.linkedTasks') }}</h3>
                  <div class="flex gap-2">
                    <button (click)="planTasksForGoal()"
                      [disabled]="planLoading()"
                      class="px-3 py-1.5 text-xs bg-ctp-mauve text-bg rounded hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1">
                      @if (planLoading()) {
                        <svg class="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24">
                          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
                        </svg>
                        {{ planStatusMessage() || t('goals.planLoading') }}
                      } @else {
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                          <path d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
                        </svg>
                        {{ t('goals.planTasksBtn') }}
                      }
                    </button>
                    <button (click)="executeWorkItem()"
                      [disabled]="executeLoading()"
                      class="px-3 py-1.5 text-xs bg-ctp-peach text-bg rounded hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1">
                      @if (executeLoading()) {
                        <svg class="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24">
                          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
                        </svg>
                      } @else {
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                          <path stroke-linecap="round" stroke-linejoin="round" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                      }
                      {{ t('goals.executeBtn') }}
                    </button>
                    <button (click)="openCreateTaskForGoal()" class="px-3 py-1.5 text-xs bg-ctp-green text-bg rounded hover:opacity-90">
                      {{ t('goals.createTaskBtn') }}
                    </button>
                    @if (statsMap().get(selected()!.id)?.backlog_count) {
                      <button (click)="startAllBacklogTasks()" class="px-3 py-1.5 text-xs bg-ctp-blue text-bg rounded hover:opacity-90">
                        {{ t('goals.startAllBtn') }}
                      </button>
                    }
                  </div>
                </div>
                @if (linkedTasksLoading()) {
                  <p class="text-xs text-text-secondary">{{ t('common.loading') }}</p>
                } @else if (linkedTasks().length === 0) {
                  <p class="text-xs text-text-secondary">{{ t('goals.noLinkedTasks') }}</p>
                } @else {
                  @if (statsFilter()) {
                    <p class="text-xs text-text-secondary mb-1">
                      {{ t('goals.filteredBy') || 'Filtered by' }}: <span class="font-medium text-accent">{{ statsFilter() }}</span>
                      ({{ filteredLinkedTasks().length }}/{{ linkedTasks().length }})
                    </p>
                  }
                  <div class="max-h-[600px] overflow-y-auto">
                    @if (selectedLinkedTask()) {
                      <div class="flex items-center gap-1 mb-2">
                        <button (click)="toggleMarkTask(selectedLinkedTask()!.id)"
                          class="p-1.5 rounded border border-border hover:bg-surface-hover transition-colors cursor-pointer"
                          [class.text-ctp-yellow]="markedTaskIds().has(selectedLinkedTask()!.id)"
                          [class.text-text-secondary]="!markedTaskIds().has(selectedLinkedTask()!.id)"
                          [title]="markedTaskIds().has(selectedLinkedTask()!.id) ? t('goals.unmarkTask') : t('goals.markTask')">
                          <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                            @if (markedTaskIds().has(selectedLinkedTask()!.id)) {
                              <path d="M5 2h14a1 1 0 011 1v19.143a.5.5 0 01-.766.424L12 18.03l-7.234 4.536A.5.5 0 014 22.143V3a1 1 0 011-1z" />
                            } @else {
                              <path d="M5 2h14a1 1 0 011 1v19.143a.5.5 0 01-.766.424L12 18.03l-7.234 4.536A.5.5 0 014 22.143V3a1 1 0 011-1zm1 2v15.432l6-3.761 6 3.761V4H6z" />
                            }
                          </svg>
                        </button>
                        <button (click)="unlinkSingleTask(selectedLinkedTask()!.id)"
                          class="px-2 py-1 text-xs rounded border border-border text-text-secondary hover:text-ctp-red hover:border-ctp-red/30 transition-colors cursor-pointer"
                          [title]="t('goals.unlink')">
                          <svg class="w-3.5 h-3.5 inline-block" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                            <path d="M6 18L18 6M6 6l12 12" />
                          </svg>
                        </button>
                      </div>
                    }
                    <app-task-list
                      [compact]="true"
                      [tasks]="filteredLinkedTasks()"
                      [loading]="linkedTasksLoading()"
                      [selectedId]="selectedLinkedTask()?.id ?? null"
                      [branchMap]="branchMap()"
                      [detailUpdates]="taskDetailUpdates()"
                      [detailComments]="taskDetailComments()"
                      [detailDependencies]="taskDetailDependencies()"
                      [detailVerifications]="taskDetailVerifications()"
                      [detailChangedFiles]="taskDetailChangedFiles()"
                      [detailGitStatus]="taskDetailGitStatus()"
                      [detailPlaybooks]="goalPlaybooks()"
                      [detailPushing]="taskDetailPushing()"
                      [detailReverting]="taskDetailReverting()"
                      [detailResolving]="taskDetailResolving()"
                      [detailLoading]="taskDetailLoading()"
                      (taskSelect)="selectLinkedTask($event)"
                      (stateChange)="onLinkedTaskStateChange($event.task, $event.target)"
                      (detailTransition)="onLinkedTaskStateChange(selectedLinkedTask()!, $event)"
                      (detailClaim)="onTaskClaim()"
                      (detailPush)="onTaskPush($event)"
                      (detailResolve)="onTaskResolve(selectedLinkedTask()!)"
                      (detailRevert)="onTaskRevert(selectedLinkedTask()!)"
                      (detailPostUpdate)="onLinkedTaskPostUpdate($event)"
                      (detailPostComment)="onLinkedTaskPostComment($event)"
                      (detailDelete)="onLinkedTaskDelete()"
                      (detailInlineUpdate)="onLinkedTaskInlineUpdate($event)"
                      (flagToggle)="onLinkedTaskFlagToggle($event.task, $event.flagged)"
                      (detailAddDep)="onLinkedTaskAddDep($event)"
                      (detailRemoveDep)="onLinkedTaskRemoveDep($event)"
                      (detailPlaybookChange)="onLinkedTaskPlaybookChange($event)"
                      (detailPlaybookStepChange)="onLinkedTaskPlaybookStepChange($event)" />
                  </div>
                }
              </div>

              <div class="text-xs text-text-secondary">
                {{ t('goals.updatedAt') }}: {{ goal.updated_at | date:'medium' }}
              </div>
            </div>
          }
        </div>
      </ng-template>

      @if (loading() && unlinkedTasksLoading()) {
        <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
      } @else {
        <!-- Section: Active Work -->
        @if (activeGoals().length > 0) {
          <div class="mb-6">
            <h2 class="text-sm font-semibold text-text-secondary uppercase tracking-wider mb-2 flex items-center gap-2">
              {{ t('work.activeGoals') }}
              <span class="text-xs font-normal">({{ activeGoals().length }})</span>
            </h2>
            <div cdkDropList [cdkDropListData]="activeGoals()" (cdkDropListDropped)="dropGoal($event)" [cdkDropListDisabled]="isTouch()" class="space-y-2">
              @for (g of activeGoals(); track g.id; let i = $index) {
                <div cdkDrag [cdkDragDisabled]="isTouch()" class="flex items-stretch gap-0">
                  <div *cdkDragPlaceholder class="rounded-lg border-2 border-dashed border-accent/30 bg-accent/5 h-20 w-full"></div>
                  @if (!isTouch()) {
                    <div cdkDragHandle
                      class="flex items-center px-1.5 cursor-grab active:cursor-grabbing text-text-muted hover:text-text-secondary shrink-0">
                      <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
                        <path d="M8 6a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm0 8a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm0 8a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm8-16a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm0 8a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm0 8a2 2 0 1 1 0-4 2 2 0 0 1 0 4z"/>
                      </svg>
                    </div>
                  } @else {
                    <!-- Mobile: up/down reorder buttons instead of drag handle -->
                    <div class="flex flex-col items-center justify-center px-1 shrink-0 gap-0.5">
                      @if (i > 0) {
                        <button (click)="moveGoal(i, -1)" class="p-0.5 text-text-muted hover:text-text-primary">
                          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path d="M5 15l7-7 7 7"/></svg>
                        </button>
                      } @else {
                        <div class="w-3.5 h-3.5 p-0.5"></div>
                      }
                      @if (i < activeGoals().length - 1) {
                        <button (click)="moveGoal(i, 1)" class="p-0.5 text-text-muted hover:text-text-primary">
                          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path d="M19 9l-7 7-7-7"/></svg>
                        </button>
                      } @else {
                        <div class="w-3.5 h-3.5 p-0.5"></div>
                      }
                    </div>
                  }
                  <div class="flex-1 min-w-0">
                    <ng-container *ngTemplateOutlet="goalItem; context: { $implicit: g }"></ng-container>
                  </div>
                </div>
              }
            </div>
          </div>
        }

        <!-- Section: Active Tasks (unlinked) -->
        @if (activeUnlinkedTasks().length > 0) {
          <div class="mb-6">
            <h2 class="text-sm font-semibold text-text-secondary uppercase tracking-wider mb-2 flex items-center gap-2">
              {{ t('work.activeTasks') }}
              <span class="text-xs font-normal">({{ activeUnlinkedTasks().length }})</span>
            </h2>
            <app-task-list
              [compact]="true"
              [tasks]="activeUnlinkedTasks()"
              [loading]="false"
              [selectedId]="selectedUnlinkedTask()?.id ?? null"
              [branchMap]="branchMap()"
              [detailUpdates]="taskDetailUpdates()"
              [detailComments]="taskDetailComments()"
              [detailDependencies]="taskDetailDependencies()"
              [detailVerifications]="taskDetailVerifications()"
              [detailChangedFiles]="taskDetailChangedFiles()"
              [detailGitStatus]="taskDetailGitStatus()"
              [detailPlaybooks]="goalPlaybooks()"
              [detailPushing]="taskDetailPushing()"
              [detailReverting]="taskDetailReverting()"
              [detailResolving]="taskDetailResolving()"
              [detailLoading]="taskDetailLoading()"
              (taskSelect)="selectUnlinkedTask($event)"
              (stateChange)="onUnlinkedTaskStateChange($event.task, $event.target)"
              (detailTransition)="onUnlinkedTaskStateChange(selectedUnlinkedTask()!, $event)"
              (detailClaim)="onTaskClaim()"
              (detailPush)="onTaskPush($event)"
              (detailResolve)="onTaskResolve(selectedUnlinkedTask()!)"
              (detailRevert)="onTaskRevert(selectedUnlinkedTask()!)"
              (detailPostUpdate)="onUnlinkedTaskPostUpdate($event)"
              (detailPostComment)="onUnlinkedTaskPostComment($event)"
              (detailDelete)="onUnlinkedTaskDelete()"
              (detailInlineUpdate)="onUnlinkedTaskInlineUpdate($event)"
              (flagToggle)="onUnlinkedTaskFlagToggle($event.task, $event.flagged)"
              (detailAddDep)="onUnlinkedTaskAddDep($event)"
              (detailRemoveDep)="onUnlinkedTaskRemoveDep($event)"
              (detailPlaybookChange)="onUnlinkedTaskPlaybookChange($event)"
              (detailPlaybookStepChange)="onUnlinkedTaskPlaybookStepChange($event)" />
          </div>
        }

        <!-- Section: Backlog (unlinked) — collapsed by default -->
        @if (backlogUnlinkedTasks().length > 0) {
          <div class="mb-6">
            <button (click)="toggleSection('backlog')"
              class="w-full flex items-center gap-2 mb-2 py-1.5 text-sm font-semibold text-text-secondary uppercase tracking-wider hover:text-text-primary transition-colors">
              <svg class="w-4 h-4 shrink-0 transition-transform duration-200"
                [class.rotate-90]="!collapsedSections().has('backlog')"
                fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
              </svg>
              {{ t('work.backlog') }}
              <span class="text-xs font-normal">({{ backlogUnlinkedTasks().length }})</span>
            </button>
            @if (!collapsedSections().has('backlog')) {
              <app-task-list
                [compact]="true"
                [tasks]="backlogUnlinkedTasks()"
                [loading]="false"
                [selectedId]="selectedUnlinkedTask()?.id ?? null"
                [branchMap]="branchMap()"
                [detailUpdates]="taskDetailUpdates()"
                [detailComments]="taskDetailComments()"
                [detailDependencies]="taskDetailDependencies()"
                [detailVerifications]="taskDetailVerifications()"
                [detailChangedFiles]="taskDetailChangedFiles()"
                [detailGitStatus]="taskDetailGitStatus()"
                [detailPlaybooks]="goalPlaybooks()"
                [detailPushing]="taskDetailPushing()"
                [detailReverting]="taskDetailReverting()"
                [detailResolving]="taskDetailResolving()"
                [detailLoading]="taskDetailLoading()"
                (taskSelect)="selectUnlinkedTask($event)"
                (stateChange)="onUnlinkedTaskStateChange($event.task, $event.target)"
                (detailTransition)="onUnlinkedTaskStateChange(selectedUnlinkedTask()!, $event)"
                (detailClaim)="onTaskClaim()"
                  (detailPush)="onTaskPush($event)"
                (detailResolve)="onTaskResolve(selectedUnlinkedTask()!)"
                (detailRevert)="onTaskRevert(selectedUnlinkedTask()!)"
                (detailPostUpdate)="onUnlinkedTaskPostUpdate($event)"
                (detailPostComment)="onUnlinkedTaskPostComment($event)"
                (detailDelete)="onUnlinkedTaskDelete()"
                (detailInlineUpdate)="onUnlinkedTaskInlineUpdate($event)"
                (flagToggle)="onUnlinkedTaskFlagToggle($event.task, $event.flagged)"
                (detailAddDep)="onUnlinkedTaskAddDep($event)"
                (detailRemoveDep)="onUnlinkedTaskRemoveDep($event)"
                (detailPlaybookChange)="onUnlinkedTaskPlaybookChange($event)"
                (detailPlaybookStepChange)="onUnlinkedTaskPlaybookStepChange($event)" />
            }
          </div>
        }

        <!-- Section: Completed (achieved goals + done unlinked tasks) — collapsed by default -->
        @if (achievedGoals().length > 0 || doneUnlinkedTasks().length > 0) {
          <div class="mb-6">
            <button (click)="toggleSection('completed')"
              class="w-full flex items-center gap-2 mb-2 py-1.5 text-sm font-semibold text-text-secondary uppercase tracking-wider hover:text-text-primary transition-colors">
              <svg class="w-4 h-4 shrink-0 transition-transform duration-200"
                [class.rotate-90]="!collapsedSections().has('completed')"
                fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
              </svg>
              {{ t('work.completed') }}
              <span class="text-xs font-normal">({{ achievedGoals().length + doneUnlinkedTasks().length }})</span>
            </button>
            @if (!collapsedSections().has('completed')) {
              @if (achievedGoals().length > 0) {
                <div class="space-y-2 mb-4">
                  @for (g of achievedGoals(); track g.id) {
                    <ng-container *ngTemplateOutlet="goalItem; context: { $implicit: g }"></ng-container>
                  }
                </div>
              }
              @if (doneUnlinkedTasks().length > 0) {
                <app-task-list
                  [compact]="true"
                  [tasks]="doneUnlinkedTasks()"
                  [loading]="false"
                  [selectedId]="selectedUnlinkedTask()?.id ?? null"
                  [branchMap]="branchMap()"
                  [detailUpdates]="taskDetailUpdates()"
                  [detailComments]="taskDetailComments()"
                  [detailDependencies]="taskDetailDependencies()"
                  [detailVerifications]="taskDetailVerifications()"
                  [detailChangedFiles]="taskDetailChangedFiles()"
                  [detailGitStatus]="taskDetailGitStatus()"
                  [detailPlaybooks]="goalPlaybooks()"
                  [detailPushing]="taskDetailPushing()"
                  [detailReverting]="taskDetailReverting()"
                  [detailResolving]="taskDetailResolving()"
                  [detailLoading]="taskDetailLoading()"
                  (taskSelect)="selectUnlinkedTask($event)"
                  (stateChange)="onUnlinkedTaskStateChange($event.task, $event.target)"
                  (detailTransition)="onUnlinkedTaskStateChange(selectedUnlinkedTask()!, $event)"
                  (detailClaim)="onTaskClaim()"
                  (detailPush)="onTaskPush($event)"
                  (detailResolve)="onTaskResolve(selectedUnlinkedTask()!)"
                  (detailRevert)="onTaskRevert(selectedUnlinkedTask()!)"
                  (detailPostUpdate)="onUnlinkedTaskPostUpdate($event)"
                  (detailPostComment)="onUnlinkedTaskPostComment($event)"
                  (detailDelete)="onUnlinkedTaskDelete()"
                  (detailInlineUpdate)="onUnlinkedTaskInlineUpdate($event)"
                  (flagToggle)="onUnlinkedTaskFlagToggle($event.task, $event.flagged)"
                  (detailAddDep)="onUnlinkedTaskAddDep($event)"
                  (detailRemoveDep)="onUnlinkedTaskRemoveDep($event)"
                  (detailPlaybookChange)="onUnlinkedTaskPlaybookChange($event)"
                  (detailPlaybookStepChange)="onUnlinkedTaskPlaybookStepChange($event)" />
              }
            }
          </div>
        }

        <!-- Section: Archived (paused/abandoned goals + cancelled unlinked tasks) — collapsed by default -->
        @if (archivedGoals().length > 0 || cancelledUnlinkedTasks().length > 0) {
          <div class="mb-6">
            <button (click)="toggleSection('archived')"
              class="w-full flex items-center gap-2 mb-2 py-1.5 text-sm font-semibold text-text-secondary uppercase tracking-wider hover:text-text-primary transition-colors">
              <svg class="w-4 h-4 shrink-0 transition-transform duration-200"
                [class.rotate-90]="!collapsedSections().has('archived')"
                fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
              </svg>
              {{ t('work.archived') }}
              <span class="text-xs font-normal">({{ archivedGoals().length + cancelledUnlinkedTasks().length }})</span>
            </button>
            @if (!collapsedSections().has('archived')) {
              @if (archivedGoals().length > 0) {
                <div class="space-y-2 mb-4">
                  @for (g of archivedGoals(); track g.id) {
                    <ng-container *ngTemplateOutlet="goalItem; context: { $implicit: g }"></ng-container>
                  }
                </div>
              }
              @if (cancelledUnlinkedTasks().length > 0) {
                <app-task-list
                  [compact]="true"
                  [tasks]="cancelledUnlinkedTasks()"
                  [loading]="false"
                  [selectedId]="selectedUnlinkedTask()?.id ?? null"
                  [branchMap]="branchMap()"
                  [detailUpdates]="taskDetailUpdates()"
                  [detailComments]="taskDetailComments()"
                  [detailDependencies]="taskDetailDependencies()"
                  [detailVerifications]="taskDetailVerifications()"
                  [detailChangedFiles]="taskDetailChangedFiles()"
                  [detailGitStatus]="taskDetailGitStatus()"
                  [detailPlaybooks]="goalPlaybooks()"
                  [detailPushing]="taskDetailPushing()"
                  [detailReverting]="taskDetailReverting()"
                  [detailResolving]="taskDetailResolving()"
                  [detailLoading]="taskDetailLoading()"
                  (taskSelect)="selectUnlinkedTask($event)"
                  (stateChange)="onUnlinkedTaskStateChange($event.task, $event.target)"
                  (detailTransition)="onUnlinkedTaskStateChange(selectedUnlinkedTask()!, $event)"
                  (detailClaim)="onTaskClaim()"
                  (detailPush)="onTaskPush($event)"
                  (detailResolve)="onTaskResolve(selectedUnlinkedTask()!)"
                  (detailRevert)="onTaskRevert(selectedUnlinkedTask()!)"
                  (detailPostUpdate)="onUnlinkedTaskPostUpdate($event)"
                  (detailPostComment)="onUnlinkedTaskPostComment($event)"
                  (detailDelete)="onUnlinkedTaskDelete()"
                  (detailInlineUpdate)="onUnlinkedTaskInlineUpdate($event)"
                  (flagToggle)="onUnlinkedTaskFlagToggle($event.task, $event.flagged)"
                  (detailAddDep)="onUnlinkedTaskAddDep($event)"
                  (detailRemoveDep)="onUnlinkedTaskRemoveDep($event)"
                  (detailPlaybookChange)="onUnlinkedTaskPlaybookChange($event)"
                  (detailPlaybookStepChange)="onUnlinkedTaskPlaybookStepChange($event)" />
              }
            }
          </div>
        }

        <!-- Empty state -->
        @if (activeGoals().length === 0 && achievedGoals().length === 0 && archivedGoals().length === 0
             && activeUnlinkedTasks().length === 0 && backlogUnlinkedTasks().length === 0
             && doneUnlinkedTasks().length === 0 && cancelledUnlinkedTasks().length === 0) {
          <p class="text-text-secondary text-sm">{{ t('common.empty') }}</p>
        }
      }

      <!-- Create/Edit goal modal -->
      @if (showForm()) {
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-[70]"
             role="button" tabindex="0" aria-label="Close modal"
             (click)="closeForm()" (keydown.enter)="closeForm()">
          <div class="bg-bg border border-border rounded-xl p-6 w-full max-w-lg max-h-[90vh] overflow-y-auto"
               tabindex="-1" (click)="$event.stopPropagation()" (keydown.enter)="$event.stopPropagation()">
            <h2 class="text-lg font-semibold text-text-primary mb-4">
              {{ editing() ? t('goals.editTitle') : t('goals.createTitle') }}
            </h2>
            <div class="space-y-4">
              <div>
                <label for="goal-title" class="block text-sm text-text-secondary mb-1">{{ t('goals.fieldTitle') }}</label>
                <input id="goal-title" type="text" [(ngModel)]="formTitle"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent" />
              </div>
              <div>
                <label for="goal-description" class="block text-sm text-text-secondary mb-1">{{ t('goals.fieldDescription') }}</label>
                <textarea id="goal-description" [(ngModel)]="formDescription" rows="12"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
              </div>
              <div>
                <label for="goal-criteria" class="block text-sm text-text-secondary mb-1">{{ t('goals.fieldCriteria') }}</label>
                <textarea id="goal-criteria" [(ngModel)]="formCriteria" rows="3"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
              </div>
              <div>
                <label for="goal-type" class="block text-sm text-text-secondary mb-1">{{ t('goals.fieldType') }}</label>
                <select id="goal-type" [(ngModel)]="formWorkType"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent">
                  @for (gt of goalTypes; track gt) {
                    <option [value]="gt">{{ t('goals.type.' + gt) }}</option>
                  }
                </select>
              </div>
              <div>
                <label for="goal-priority" class="block text-sm text-text-secondary mb-1">{{ t('goals.fieldPriority') }}</label>
                <input id="goal-priority" type="number" [(ngModel)]="formPriority"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent" />
              </div>
              <div class="flex items-center gap-2">
                <input id="goal-auto-status" type="checkbox" [(ngModel)]="formAutoStatus"
                  class="rounded border-border text-accent focus:ring-accent" />
                <label for="goal-auto-status" class="text-sm text-text-secondary">{{ t('goals.autoStatus') }}</label>
              </div>
              <div class="flex justify-end gap-3 pt-2">
                <button (click)="closeForm()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                  {{ t('goals.cancel') }}
                </button>
                <button (click)="submitForm()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
                  {{ editing() ? t('goals.save') : t('goals.create') }}
                </button>
              </div>
            </div>
          </div>
        </div>
      }

      <!-- Task Picker modal -->
      @if (showTaskPicker()) {
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-[70]"
             role="button" tabindex="0" aria-label="Close task picker"
             (click)="closeTaskPicker()" (keydown.enter)="closeTaskPicker()">
          <div class="bg-bg border border-border rounded-xl p-6 w-full max-w-2xl max-h-[90vh] flex flex-col"
               tabindex="-1" (click)="$event.stopPropagation()" (keydown.enter)="$event.stopPropagation()">
            <div class="flex items-center justify-between mb-4">
              <h2 class="text-lg font-semibold text-text-primary">{{ t('goals.pickTasks') }}</h2>
              <button (click)="closeTaskPicker()" class="p-1.5 text-text-secondary hover:text-text-primary rounded">
                <svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            <!-- Picker filters -->
            <div class="flex flex-wrap gap-2 mb-3">
              <input
                type="text"
                [placeholder]="t('goals.pickerSearchPlaceholder')"
                [(ngModel)]="pickerSearch"
                (ngModelChange)="loadPickerTasks()"
                class="flex-1 min-w-[150px] bg-surface text-text-primary text-xs rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
              <select
                [(ngModel)]="pickerStateFilter"
                (ngModelChange)="loadPickerTasks()"
                class="bg-surface text-text-primary text-xs rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent">
                <option value="">{{ t('tasks.allStates') }}</option>
                @for (s of taskStates; track s) {
                  <option [value]="s">{{ s }}</option>
                }
              </select>
              <label class="flex items-center gap-1.5 text-xs text-text-secondary cursor-pointer">
                <input type="checkbox" [(ngModel)]="pickerUnlinkedOnly" (ngModelChange)="loadPickerTasks()"
                  class="rounded border-border text-accent focus:ring-accent" />
                {{ t('goals.unlinkedOnly') }}
              </label>
            </div>

            <!-- Task list -->
            <div class="flex-1 overflow-y-auto min-h-0 mb-3 border border-border rounded-lg">
              @if (pickerLoading()) {
                <p class="text-xs text-text-secondary p-3">{{ t('common.loading') }}</p>
              } @else if (pickerTasks().length === 0) {
                <p class="text-xs text-text-secondary p-3">{{ t('common.empty') }}</p>
              } @else {
                <div class="divide-y divide-border">
                  @for (task of pickerTasks(); track task.id) {
                    <label class="flex items-center gap-3 p-2.5 hover:bg-surface-hover cursor-pointer transition-colors">
                      <input type="checkbox"
                        [checked]="pickerSelectedIds().has(task.id)"
                        (change)="togglePickerTask(task.id)"
                        class="rounded border-border text-accent focus:ring-accent shrink-0" />
                      <div class="flex-1 min-w-0">
                        <span class="text-sm text-text-primary block truncate">{{ task.title }}</span>
                        <div class="flex items-center gap-1.5 mt-0.5">
                          <span class="px-1.5 py-0.5 rounded-full text-xs bg-ctp-blue/20 text-ctp-blue">{{ task.kind }}</span>
                          <span class="px-1.5 py-0.5 rounded-full text-xs bg-ctp-green/20 text-ctp-green">{{ task.state }}</span>
                          @if (task.urgent) {
                            <span class="text-xs text-ctp-red font-medium">Urgent</span>
                          }
                        </div>
                      </div>
                    </label>
                  }
                </div>
              }
            </div>

            <!-- Footer -->
            <div class="flex items-center justify-between pt-2 border-t border-border">
              <span class="text-xs text-text-secondary">
                {{ pickerSelectedIds().size }} {{ t('goals.tasksSelected') }}
              </span>
              <div class="flex gap-2">
                <button (click)="closeTaskPicker()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                  {{ t('goals.cancel') }}
                </button>
                <button (click)="linkSelectedTasks()"
                  [disabled]="pickerSelectedIds().size === 0"
                  class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed">
                  {{ t('goals.linkSelected') }}
                </button>
              </div>
            </div>
          </div>
        </div>
      }

      <!-- Plan Preview Dialog -->
      @if (showPlanDialog()) {
        <div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-[70]"
             role="button" tabindex="0" aria-label="Close plan dialog"
             (click)="closePlanDialog()" (keydown.enter)="closePlanDialog()">
          <div class="bg-bg border border-border rounded-xl p-6 w-full max-w-2xl max-h-[90vh] flex flex-col"
               tabindex="-1" (click)="$event.stopPropagation()" (keydown.enter)="$event.stopPropagation()">
            <div class="flex items-center justify-between mb-2">
              <h2 class="text-lg font-semibold text-text-primary">{{ t('goals.planDialogTitle') }}</h2>
              <button (click)="closePlanDialog()" class="p-1.5 text-text-secondary hover:text-text-primary rounded">
                <svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
            <p class="text-sm text-text-secondary mb-4">{{ t('goals.planDialogDesc') }}</p>

            @if (planSuccessCriteria().length > 0) {
              <div class="mb-4 bg-ctp-green/5 border border-ctp-green/20 rounded-lg p-3">
                <h3 class="text-xs font-semibold text-ctp-green uppercase tracking-wider mb-2">{{ t('goals.planGeneratedCriteria') }}</h3>
                <ul class="space-y-1">
                  @for (criterion of planSuccessCriteria(); track $index) {
                    <li class="flex items-start gap-1.5 text-xs text-text-primary">
                      <span class="text-ctp-green mt-0.5 shrink-0">&#10003;</span>
                      <span>{{ criterion }}</span>
                    </li>
                  }
                </ul>
              </div>
            }

            @if (plannedTasks().length === 0) {
              <p class="text-sm text-text-secondary py-8 text-center">{{ t('goals.planEmpty') }}</p>
            } @else {
              <div class="flex-1 overflow-y-auto min-h-0 mb-4 space-y-3">
                @for (task of plannedTasks(); track $index) {
                  <div class="bg-surface border border-border rounded-lg p-3">
                    <div class="flex items-center gap-2 mb-2">
                      <input type="text" [(ngModel)]="task.title"
                        class="flex-1 bg-bg text-text-primary text-sm font-medium rounded px-2 py-1.5 border border-border
                               focus:outline-none focus:ring-1 focus:ring-accent" />
                      <span class="px-2 py-0.5 rounded-full text-xs font-medium shrink-0"
                        [class]="planKindColor(task.kind)">
                        {{ task.kind }}
                      </span>
                      <button (click)="togglePlanTaskExpanded($index)"
                        class="p-1 text-text-secondary hover:text-text-primary rounded shrink-0"
                        [title]="planExpandedTasks().has($index) ? 'Collapse' : 'Expand'">
                        <svg class="w-4 h-4 transition-transform duration-200"
                          [class.rotate-90]="planExpandedTasks().has($index)"
                          fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
                        </svg>
                      </button>
                      <button (click)="removePlannedTask($index)"
                        class="p-1 text-text-secondary hover:text-ctp-red rounded shrink-0"
                        [title]="t('goals.planRemoveTask')">
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                          <path d="M6 18L18 6M6 6l12 12" />
                        </svg>
                      </button>
                    </div>
                    @if (planExpandedTasks().has($index)) {
                      <div class="mt-2 space-y-2">
                        <div>
                          <span class="text-xs font-semibold text-text-secondary uppercase tracking-wider block">{{ t('goals.planTaskSpec') }}</span>
                          <textarea [(ngModel)]="task.spec" rows="4"
                            class="w-full mt-1 bg-bg text-text-primary text-xs rounded px-2 py-1.5 border border-border
                                   focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
                        </div>
                        @if (task.depends_on && task.depends_on.length > 0) {
                          <div>
                            <span class="text-xs font-semibold text-text-secondary uppercase tracking-wider block">{{ t('goals.planDependsOn') }}</span>
                            <div class="mt-1 flex flex-wrap gap-1">
                              @for (depIdx of task.depends_on; track depIdx) {
                                @if (plannedTasks()[depIdx]) {
                                  <span class="px-1.5 py-0.5 bg-ctp-overlay0/20 text-text-secondary text-xs rounded">
                                    {{ depIdx + 1 }}. {{ plannedTasks()[depIdx].title | slice:0:40 }}
                                  </span>
                                }
                              }
                            </div>
                          </div>
                        }
                        @if (task.acceptance_criteria && task.acceptance_criteria.length > 0) {
                          <div>
                            <span class="text-xs font-semibold text-text-secondary uppercase tracking-wider block">{{ t('goals.planTaskCriteria') }}</span>
                            <ul class="mt-1 space-y-1">
                              @for (criterion of task.acceptance_criteria; track $index) {
                                <li class="flex items-start gap-1.5 text-xs text-text-primary">
                                  <span class="text-ctp-green mt-0.5 shrink-0">✓</span>
                                  <span>{{ criterion }}</span>
                                </li>
                              }
                            </ul>
                          </div>
                        }
                      </div>
                    }
                  </div>
                }
              </div>
            }

            <!-- Footer -->
            <div class="flex items-center justify-end gap-2 pt-3 border-t border-border">
              <button (click)="closePlanDialog()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                {{ t('goals.planCancel') }}
              </button>
              @if (plannedTasks().length > 0) {
                <button (click)="confirmPlan()"
                  [disabled]="planCreating()"
                  class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2">
                  @if (planCreating()) {
                    <svg class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
                    </svg>
                    {{ t('goals.planCreating') }}
                  } @else {
                    {{ t('goals.planCreateBtn', { count: plannedTasks().length }) }}
                  }
                </button>
              }
            </div>
          </div>
        </div>
      }

      <!-- Task Create form (reuse TaskFormComponent) -->
      <app-task-form
        [show]="showTaskForm()"
        [editing]="editingTask()"
        (submitCreate)="onCreateTask($event)"
        (submitUpdate)="onUpdateTaskForGoal($event)"
        (closed)="closeTaskForm()" />
    </div>
  `,
})
export class WorkPage {
  private api = inject(WorkApiService);
  private tasksApi = inject(TasksApiService);
  private verificationsApi = inject(VerificationsApiService);
  private playbooksApi = inject(PlaybooksApiService);
  private ctx = inject(ProjectContext);
  private git = inject(GitApiService);
  private chat = inject(ChatService);
  private destroyRef = inject(DestroyRef);
  private cdr = inject(ChangeDetectorRef);
  private platformId = inject(PLATFORM_ID);

  /** True on touch-primary devices — disables CDK drag to preserve mobile scrolling. */
  isTouch = signal(false);

  readonly statuses = STATUSES;
  readonly goalTypes = GOAL_TYPES;
  readonly taskStates = TASK_STATES;

  items = signal<SpWork[]>([]);
  loading = signal(false);
  selected = signal<SpWork | null>(null);
  searchQuery = signal('');
  selectedStatus = '';
  selectedWorkType = '';
  progressMap = signal<Map<string, SpWorkProgress>>(new Map());
  statsMap = signal<Map<string, SpWorkStats>>(new Map());
  childrenMap = signal<Map<string, SpWork[]>>(new Map());

  showForm = signal(false);
  editing = signal<SpWork | null>(null);
  formTitle = '';
  formDescription = '';
  formCriteria = '';
  formWorkType: WorkType = 'epic';
  formPriority = 0;
  formAutoStatus = false;

  // Todos
  goalTodos = signal<WorkTodo[]>([]);
  newTodoText = '';
  private nextTodoId = 1;
  hasDoneTodos = computed(() => this.goalTodos().some(todo => todo.done));

  // Linked tasks (for selected goal)
  linkedTasks = signal<SpTask[]>([]);
  linkedTasksLoading = signal(false);
  statsFilter = signal<string>('');

  // Unlinked tasks
  unlinkedTasks = signal<SpTask[]>([]);
  unlinkedTasksLoading = signal(false);

  // Section collapse state
  collapsedSections = signal<Set<string>>(new Set(['completed', 'archived']));

  // Task create form
  showTaskForm = signal(false);

  // Marked tasks (per-goal, stored in localStorage)
  markedTaskIds = signal<Set<string>>(new Set());

  // Task picker
  showTaskPicker = signal(false);
  pickerTasks = signal<SpTask[]>([]);
  pickerLoading = signal(false);
  pickerSelectedIds = signal<Set<string>>(new Set());
  pickerSearch = '';
  pickerStateFilter = '';
  pickerUnlinkedOnly = true;

  // Plan tasks
  planLoading = signal(false);
  planStatusMessage = signal('');
  plannedTasks = signal<PlannedTask[]>([]);
  planSuccessCriteria = signal<string[]>([]);
  showPlanDialog = signal(false);
  planCreating = signal(false);
  planExpandedTasks = signal<Set<number>>(new Set());

  // Task detail data (shared between linked and unlinked task expansion)
  selectedLinkedTask = signal<SpTask | null>(null);
  selectedUnlinkedTask = signal<SpTask | null>(null);
  taskDetailUpdates = signal<SpTaskUpdate[]>([]);
  taskDetailComments = signal<SpTaskComment[]>([]);
  taskDetailDependencies = signal<SpTaskDependencies>({ depends_on: [], blocks: [] });
  taskDetailVerifications = signal<SpVerification[]>([]);
  taskDetailChangedFiles = signal<ChangedFileSummary[]>([]);
  taskDetailGitStatus = signal<TaskBranchStatus | null>(null);
  taskDetailLoading = signal(false);
  taskDetailPushing = signal(false);
  taskDetailReverting = signal(false);
  taskDetailResolving = signal(false);
  goalPlaybooks = signal<SpPlaybook[]>([]);
  executeLoading = signal(false);
  editingTask = signal<SpTask | null>(null);

  // Git status
  mainStatus = signal<MainPushStatus | null>(null);
  pushingMain = signal(false);
  resolvingMain = signal(false);
  releasing = signal(false);
  branchMap = signal<Map<string, BranchInfo>>(new Map());
  private gitPollSub?: Subscription;
  private detailPollSub?: Subscription;
  private gitSub?: Subscription;

  // --- Computed: goal sections ---

  private filteredGoals = computed(() => {
    const q = this.searchQuery().toLowerCase().trim();
    return q
      ? this.items().filter(g => g.title.toLowerCase().includes(q) || g.description.toLowerCase().includes(q))
      : this.items();
  });

  activeGoals = computed(() => this.filteredGoals().filter(g => g.status === 'active'));
  achievedGoals = computed(() => this.filteredGoals().filter(g => g.status === 'achieved'));
  archivedGoals = computed(() => this.filteredGoals().filter(g => g.status === 'paused' || g.status === 'abandoned'));

  // --- Computed: unlinked task sections ---

  private static readonly INACTIVE_STATES = new Set(['backlog', 'done', 'cancelled']);

  activeUnlinkedTasks = computed(() =>
    this.unlinkedTasks().filter(t => !WorkPage.INACTIVE_STATES.has(t.state)),
  );
  backlogUnlinkedTasks = computed(() => this.unlinkedTasks().filter(t => t.state === 'backlog'));
  doneUnlinkedTasks = computed(() => this.unlinkedTasks().filter(t => t.state === 'done'));
  cancelledUnlinkedTasks = computed(() => this.unlinkedTasks().filter(t => t.state === 'cancelled'));

  // --- Computed: linked tasks ---

  availableTransitions = computed(() => {
    const sel = this.selected();
    if (!sel || sel.auto_status) return [];
    return STATUSES.filter(s => s !== sel.status);
  });

  private static readonly NON_WORKING_STATES = new Set(['backlog', 'ready', 'done', 'cancelled']);

  filteredLinkedTasks = computed(() => {
    const filter = this.statsFilter();
    const tasks = this.sortedLinkedTasks();
    if (!filter) return tasks;
    if (filter === 'working') {
      return tasks.filter(t => !WorkPage.NON_WORKING_STATES.has(t.state));
    }
    if (filter === 'blocked') return tasks;
    return tasks.filter(t => t.state === filter);
  });

  sortedLinkedTasks = computed(() => {
    const tasks = this.linkedTasks();
    const marked = this.markedTaskIds();
    return [...tasks].sort((a, b) => {
      const aMarked = marked.has(a.id) ? 0 : 1;
      const bMarked = marked.has(b.id) ? 0 : 1;
      return aMarked - bMarked;
    });
  });

  constructor() {
    if (isPlatformBrowser(this.platformId)) {
      this.isTouch.set(window.matchMedia('(pointer: coarse)').matches);
    }
    effect(() => {
      this.ctx.projectId();
      this.selected.set(null);
      this.selectedLinkedTask.set(null);
      this.selectedUnlinkedTask.set(null);
      this.loadGoals();
      this.loadUnlinkedTasks();
      this.playbooksApi.list().subscribe({
        next: (pbs) => this.goalPlaybooks.set(pbs),
      });
      this.startGitPolling();
    });
  }

  // --- Git polling ---

  private startGitPolling(): void {
    this.gitPollSub?.unsubscribe();
    this.gitPollSub = timer(0, 15_000)
      .pipe(
        switchMap(() => forkJoin({
          branches: this.git.listBranches('agent/').pipe(catchError(() => of({ current_branch: '', branches: [] as BranchInfo[] }))),
          mainStatus: this.git.mainStatus().pipe(catchError(() => of(null as MainPushStatus | null))),
        })),
        takeUntilDestroyed(this.destroyRef),
      )
      .subscribe({
        next: ({ branches, mainStatus }) => {
          this.mainStatus.set(mainStatus);
          const map = new Map<string, BranchInfo>();
          for (const b of branches.branches) {
            if (b.task_id_prefix) {
              map.set(b.task_id_prefix, b);
            }
          }
          this.branchMap.set(map);
        },
      });
  }

  onPushMain(): void {
    this.pushingMain.set(true);
    this.git.pushMain().subscribe({
      next: () => {
        this.pushingMain.set(false);
        this.mainStatus.set(null);
      },
      error: () => this.pushingMain.set(false),
    });
  }

  onReleaseProject(): void {
    if (!confirm('Create a new release? This will squash-merge dev → main, create a version tag, and push to all remotes.')) return;
    this.releasing.set(true);
    this.git.release().subscribe({
      next: (res) => {
        this.releasing.set(false);
        alert(res.message);
        this.git.mainStatus().pipe(catchError(() => of(null as MainPushStatus | null))).subscribe({
          next: (ms) => this.mainStatus.set(ms),
        });
      },
      error: (err) => {
        this.releasing.set(false);
        const errorMsg = err?.error?.error || err?.error?.message || err?.message || 'Unknown error';
        alert(`Release failed: ${errorMsg}`);
      },
    });
  }

  onResolveAndPushMain(): void {
    this.resolvingMain.set(true);
    const ms = this.mainStatus();
    this.git.resolveAndPushMain().subscribe({
      next: () => {
        this.resolvingMain.set(false);
        this.mainStatus.set(null);
      },
      error: (err) => {
        this.resolvingMain.set(false);
        const detail = ms ? ` (local is ${ms.ahead} commit(s) ahead, ${ms.behind} behind remote)` : '';
        const errorMsg = err?.error?.error || err?.error?.message || err?.message || 'Unknown error';
        const prompt =
          `There is a merge conflict on the main branch${detail}. ` +
          `The automatic rebase+push failed with: ${errorMsg}. ` +
          `Can you resolve this merge conflict and push main to the remote?`;
        this.chat.openWithMessage(prompt);
      },
    });
  }

  // --- Detail polling ---

  private startDetailPolling(taskId: string): void {
    this.stopDetailPolling();
    this.detailPollSub = timer(10_000, 10_000)
      .pipe(
        switchMap(() => this.fetchTaskDetail(taskId)),
        takeUntilDestroyed(this.destroyRef),
      )
      .subscribe({
        next: ({ updates, comments, dependencies, changedFiles, verifications }) => {
          const sel = this.selectedLinkedTask() ?? this.selectedUnlinkedTask();
          if (sel?.id === taskId) {
            this.taskDetailUpdates.set(updates);
            this.taskDetailComments.set(comments);
            this.taskDetailDependencies.set(dependencies);
            this.taskDetailChangedFiles.set(changedFiles);
            this.taskDetailVerifications.set(verifications.data);
          }
        },
      });
  }

  private stopDetailPolling(): void {
    this.detailPollSub?.unsubscribe();
    this.detailPollSub = undefined;
  }

  private fetchTaskDetail(taskId: string) {
    return forkJoin({
      updates: this.tasksApi.listUpdates(taskId).pipe(catchError(() => of([] as SpTaskUpdate[]))),
      comments: this.tasksApi.listComments(taskId).pipe(catchError(() => of([] as SpTaskComment[]))),
      dependencies: this.tasksApi.listDependencies(taskId).pipe(catchError(() => of({ depends_on: [], blocks: [] } as SpTaskDependencies))),
      changedFiles: this.tasksApi.listChangedFiles(taskId).pipe(catchError(() => of([] as ChangedFileSummary[]))),
      verifications: this.verificationsApi.list({ task_id: taskId, limit: 20 }).pipe(
        catchError(() => of({ data: [] as SpVerification[], total: 0, limit: 20, offset: 0, has_more: false })),
      ),
    });
  }

  private loadGitStatus(taskId: string): void {
    this.gitSub?.unsubscribe();
    this.gitSub = this.git.taskBranchStatus(taskId).pipe(
      catchError(() => of(null as TaskBranchStatus | null)),
      takeUntilDestroyed(this.destroyRef),
    ).subscribe({
      next: (status) => {
        const sel = this.selectedLinkedTask() ?? this.selectedUnlinkedTask();
        if (sel?.id === taskId) {
          this.taskDetailGitStatus.set(status);
        }
      },
    });
  }

  // --- Task-level git operations ---

  onTaskPush(branch: string): void {
    this.taskDetailPushing.set(true);
    this.git.pushBranch(branch).subscribe({
      next: (res) => {
        this.taskDetailPushing.set(false);
        if (res.success) {
          const sel = this.selectedLinkedTask() ?? this.selectedUnlinkedTask();
          if (sel) this.loadGitStatus(sel.id);
        }
      },
      error: () => this.taskDetailPushing.set(false),
    });
  }

  onTaskRevert(task: SpTask): void {
    this.taskDetailReverting.set(true);
    this.git.revertTask(task.id).subscribe({
      next: () => {
        this.taskDetailReverting.set(false);
        this.loadUnlinkedTasks();
        const sel = this.selected();
        if (sel) this.loadLinkedTasks(sel.id);
      },
      error: () => this.taskDetailReverting.set(false),
    });
  }

  onTaskResolve(task: SpTask): void {
    this.taskDetailResolving.set(true);
    this.git.resolveTaskBranch(task.id).subscribe({
      next: () => {
        this.taskDetailResolving.set(false);
        this.loadGitStatus(task.id);
      },
      error: (err) => {
        this.taskDetailResolving.set(false);
        const errorMsg = err?.error?.error || err?.error?.message || err?.message || 'Unknown error';
        const branch = this.taskDetailGitStatus()?.branch ?? `agent/task-${task.id.substring(0, 12)}`;
        const prompt =
          `There is a merge conflict on branch ${branch}. ` +
          `The automatic rebase failed with: ${errorMsg}. ` +
          `Can you resolve this merge conflict by rebasing the branch onto the default branch?`;
        this.chat.openWithMessage(prompt);
      },
    });
  }

  onTaskClaim(): void {
    // Claim requires an agent_id; not applicable in web UI context.
  }

  // --- Section collapse ---

  toggleSection(section: string): void {
    const current = new Set(this.collapsedSections());
    if (current.has(section)) {
      current.delete(section);
    } else {
      current.add(section);
    }
    this.collapsedSections.set(current);
  }

  // --- Drag & drop / reorder ---

  moveGoal(index: number, direction: number): void {
    const target = index + direction;
    const active = [...this.activeGoals()];
    if (target < 0 || target >= active.length) return;
    [active[index], active[target]] = [active[target], active[index]];

    // Rebuild items: reordered active goals + rest in original order
    const activeIds = new Set(active.map(g => g.id));
    const rest = this.items().filter(g => !activeIds.has(g.id));
    this.items.set([...active, ...rest]);

    // Persist to server
    const goalIds = active.map(g => g.id);
    this.api.reorder(goalIds).subscribe({
      error: () => this.loadGoals(),
    });
  }

  dropGoal(event: CdkDragDrop<SpWork[]>): void {
    if (event.previousIndex === event.currentIndex) return;
    const active = [...this.activeGoals()];
    moveItemInArray(active, event.previousIndex, event.currentIndex);

    // Rebuild items: reordered active goals + rest in original order
    const activeIds = new Set(active.map(g => g.id));
    const rest = this.items().filter(g => !activeIds.has(g.id));
    this.items.set([...active, ...rest]);

    // Force immediate DOM update before CDK's post-drop animation setup.
    // Without this, CDK calculates animation targets from old DOM positions,
    // then Angular re-renders later and the item snaps back to its old spot.
    this.cdr.detectChanges();

    // Persist to server
    const goalIds = active.map(g => g.id);
    this.api.reorder(goalIds).subscribe({
      error: () => this.loadGoals(),
    });
  }

  // --- Goals ---

  loadGoals(): void {
    this.loading.set(true);
    const status = this.selectedStatus as WorkStatus | '';
    const goalType = this.selectedWorkType as WorkType | '';
    this.api.list(status || undefined, goalType || undefined).subscribe({
      next: (items) => {
        this.items.set(items);
        this.loading.set(false);
        if (this.selected()) {
          const still = items.find(i => i.id === this.selected()!.id);
          this.selected.set(still ?? null);
        }
        this.loadAllProgress(items);
      },
      error: () => this.loading.set(false),
    });
  }

  private loadAllProgress(goals: SpWork[]): void {
    const map = new Map<string, SpWorkProgress>();
    let remaining = goals.length;
    if (remaining === 0) {
      this.progressMap.set(map);
      return;
    }
    for (const goal of goals) {
      this.api.progress(goal.id).subscribe({
        next: (prog) => {
          map.set(goal.id, prog);
          remaining--;
          if (remaining === 0) this.progressMap.set(new Map(map));
        },
        error: () => {
          remaining--;
          if (remaining === 0) this.progressMap.set(new Map(map));
        },
      });
    }
    this.loadAllStats(goals);
  }

  private loadAllStats(goals: SpWork[]): void {
    const map = new Map<string, SpWorkStats>(this.statsMap());
    let remaining = goals.length;
    if (remaining === 0) return;
    for (const goal of goals) {
      this.api.stats(goal.id).subscribe({
        next: (stats) => {
          map.set(goal.id, stats);
          remaining--;
          if (remaining === 0) this.statsMap.set(new Map(map));
        },
        error: () => {
          remaining--;
          if (remaining === 0) this.statsMap.set(new Map(map));
        },
      });
    }
  }

  selectItem(goal: SpWork): void {
    this.selected.set(goal.id === this.selected()?.id ? null : goal);
    this.statsFilter.set('');
    this.selectedLinkedTask.set(null);
    if (this.selected()) {
      this.formTitle = goal.title;
      this.formDescription = goal.description;
      this.formCriteria = goal.success_criteria;
      this.formWorkType = goal.work_type;
      this.formPriority = goal.priority;
      this.formAutoStatus = goal.auto_status;
      this.loadTodos(goal);
      this.loadStatsAndChildren(goal.id);
      this.loadLinkedTasks(goal.id);
      this.loadMarkedTasks(goal.id);
    } else {
      this.markedTaskIds.set(new Set());
    }
  }

  private loadStatsAndChildren(goalId: string): void {
    this.api.stats(goalId).subscribe({
      next: (stats) => {
        const map = new Map(this.statsMap());
        map.set(goalId, stats);
        this.statsMap.set(map);
      },
    });
    this.api.children(goalId).subscribe({
      next: (children) => {
        const map = new Map(this.childrenMap());
        map.set(goalId, children);
        this.childrenMap.set(map);
      },
    });
  }

  private loadLinkedTasks(goalId: string): void {
    this.linkedTasksLoading.set(true);
    this.api.listTasks(goalId, { limit: 50 }).subscribe({
      next: (tasks) => {
        this.linkedTasks.set(tasks);
        this.linkedTasksLoading.set(false);
      },
      error: () => {
        this.linkedTasks.set([]);
        this.linkedTasksLoading.set(false);
      },
    });
  }

  statusColor(status: WorkStatus): string {
    return STATUS_COLORS[status] ?? '';
  }

  progressColor(status: WorkStatus): string {
    return PROGRESS_COLORS[status] ?? '';
  }

  typeColor(type: WorkType): string {
    return TYPE_COLORS[type] ?? '';
  }

  setStatsFilter(state: string): void {
    this.statsFilter.set(this.statsFilter() === state ? '' : state);
  }

  clearStatsFilter(): void {
    this.statsFilter.set('');
  }

  transitionStatus(goal: SpWork, newStatus: WorkStatus): void {
    this.api.update(goal.id, { status: newStatus }).subscribe({
      next: () => this.loadGoals(),
    });
  }

  openCreate(): void {
    this.saveInlineField();
    this.selected.set(null);
    this.editing.set(null);
    this.formTitle = '';
    this.formDescription = '';
    this.formCriteria = '';
    this.formWorkType = 'epic';
    this.formPriority = 0;
    this.formAutoStatus = false;
    this.showForm.set(true);
  }

  saveInlineField(): void {
    const sel = this.selected();
    if (!sel) return;
    if (
      sel.title === this.formTitle &&
      sel.description === this.formDescription &&
      sel.success_criteria === this.formCriteria &&
      sel.work_type === this.formWorkType &&
      sel.priority === this.formPriority &&
      sel.auto_status === this.formAutoStatus
    ) {
      return;
    }
    this.api
      .update(sel.id, {
        title: this.formTitle,
        description: this.formDescription,
        success_criteria: this.formCriteria,
        work_type: this.formWorkType,
        priority: this.formPriority,
        auto_status: this.formAutoStatus,
      })
      .subscribe({
        next: () => this.loadGoals(),
      });
  }

  closeForm(): void {
    this.showForm.set(false);
    this.editing.set(null);
  }

  submitForm(): void {
    const existing = this.editing();
    if (existing) {
      this.api.update(existing.id, {
        title: this.formTitle,
        description: this.formDescription,
        success_criteria: this.formCriteria,
        work_type: this.formWorkType,
        priority: this.formPriority,
        auto_status: this.formAutoStatus,
      }).subscribe({
        next: () => {
          this.closeForm();
          this.loadGoals();
        },
      });
    } else {
      const data: SpWorkCreate = {
        title: this.formTitle,
        description: this.formDescription,
        success_criteria: this.formCriteria,
        work_type: this.formWorkType,
        priority: this.formPriority,
        auto_status: this.formAutoStatus,
      };
      this.api.create(data).subscribe({
        next: () => {
          this.closeForm();
          this.loadGoals();
        },
      });
    }
  }

  confirmDelete(goal: SpWork): void {
    this.api.delete(goal.id).subscribe({
      next: () => {
        this.selected.set(null);
        this.loadGoals();
      },
    });
  }

  // --- Todos ---

  private loadTodos(goal: SpWork): void {
    const raw = (goal.metadata?.['todos'] as WorkTodo[] | undefined) ?? [];
    this.goalTodos.set(raw);
    this.nextTodoId = raw.reduce((max, t) => Math.max(max, t.id + 1), 1);
    this.newTodoText = '';
  }

  addTodo(): void {
    const text = this.newTodoText.trim();
    if (!text) return;
    const todo: WorkTodo = { id: this.nextTodoId++, text, done: false };
    const updated = [...this.goalTodos(), todo];
    this.goalTodos.set(updated);
    this.newTodoText = '';
    this.saveTodos(updated);
  }

  toggleTodo(id: number): void {
    const updated = this.goalTodos().map(t => (t.id === id ? { ...t, done: !t.done } : t));
    this.goalTodos.set(updated);
    this.saveTodos(updated);
  }

  removeTodo(id: number): void {
    const updated = this.goalTodos().filter(t => t.id !== id);
    this.goalTodos.set(updated);
    this.saveTodos(updated);
  }

  clearDoneTodos(): void {
    const updated = this.goalTodos().filter(t => !t.done);
    this.goalTodos.set(updated);
    this.saveTodos(updated);
  }

  private saveTodos(todos: WorkTodo[]): void {
    const sel = this.selected();
    if (!sel) return;
    const metadata = { ...(sel.metadata ?? {}), todos };
    this.api.update(sel.id, { metadata }).subscribe({
      next: () => {
        sel.metadata = metadata;
      },
    });
  }

  // --- Linked tasks ---

  unlinkSingleTask(taskId: string): void {
    const sel = this.selected();
    if (!sel) return;
    this.api.unlinkTask(sel.id, taskId).subscribe({
      next: () => {
        this.loadLinkedTasks(sel.id);
        this.loadAllProgress([sel]);
        this.loadStatsAndChildren(sel.id);
      },
    });
  }

  selectLinkedTask(task: SpTask): void {
    this.selectedUnlinkedTask.set(null);
    if (task && this.selectedLinkedTask()?.id === task.id) {
      this.selectedLinkedTask.set(null);
      this.stopDetailPolling();
      return;
    }
    this.selectedLinkedTask.set(task);
    this.resetTaskDetail();
    if (task) {
      this.loadTaskDetail(task.id);
    } else {
      this.stopDetailPolling();
    }
  }

  private resetTaskDetail(): void {
    this.taskDetailUpdates.set([]);
    this.taskDetailComments.set([]);
    this.taskDetailDependencies.set({ depends_on: [], blocks: [] });
    this.taskDetailVerifications.set([]);
    this.taskDetailChangedFiles.set([]);
    this.taskDetailGitStatus.set(null);
  }

  private loadTaskDetail(taskId: string): void {
    this.taskDetailLoading.set(true);
    this.fetchTaskDetail(taskId).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
      next: ({ updates, comments, dependencies, changedFiles, verifications }) => {
        this.taskDetailUpdates.set(updates);
        this.taskDetailComments.set(comments);
        this.taskDetailDependencies.set(dependencies);
        this.taskDetailChangedFiles.set(changedFiles);
        this.taskDetailVerifications.set(verifications.data);
        this.taskDetailLoading.set(false);
      },
      error: () => this.taskDetailLoading.set(false),
    });
    this.loadGitStatus(taskId);
    this.startDetailPolling(taskId);
  }

  onLinkedTaskStateChange(task: SpTask, target: string): void {
    this.tasksApi.transition(task.id, target).subscribe({
      next: () => {
        const sel = this.selected();
        if (sel) {
          this.loadLinkedTasks(sel.id);
          this.loadAllProgress([sel]);
          this.loadStatsAndChildren(sel.id);
        }
      },
    });
  }

  startAllBacklogTasks(): void {
    const backlogIds = this.linkedTasks()
      .filter(t => t.state === 'backlog')
      .map(t => t.id);
    if (backlogIds.length === 0) return;
    this.tasksApi.bulkTransition(backlogIds, 'ready').subscribe({
      next: () => {
        const sel = this.selected();
        if (sel) {
          this.loadLinkedTasks(sel.id);
          this.loadAllProgress([sel]);
          this.loadStatsAndChildren(sel.id);
        }
      },
    });
  }

  onLinkedTaskPostUpdate(event: { kind: string; content: string }): void {
    const task = this.selectedLinkedTask();
    if (!task) return;
    this.tasksApi.createUpdate(task.id, event).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onLinkedTaskPostComment(content: string): void {
    const task = this.selectedLinkedTask();
    if (!task) return;
    this.tasksApi.createComment(task.id, { content }).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onLinkedTaskDelete(): void {
    const task = this.selectedLinkedTask();
    if (!task) return;
    this.tasksApi.delete(task.id).subscribe({
      next: () => {
        this.selectedLinkedTask.set(null);
        const sel = this.selected();
        if (sel) {
          this.loadLinkedTasks(sel.id);
          this.loadAllProgress([sel]);
          this.loadStatsAndChildren(sel.id);
        }
      },
    });
  }

  onLinkedTaskInlineUpdate(data: UpdateTaskRequest): void {
    const task = this.selectedLinkedTask();
    if (!task) return;
    this.tasksApi.update(task.id, data).subscribe({
      next: () => {
        const sel = this.selected();
        if (sel) this.loadLinkedTasks(sel.id);
      },
    });
  }

  onLinkedTaskFlagToggle(task: SpTask, flagged: boolean): void {
    // Optimistic update — toggle immediately in the local linked tasks list
    this.linkedTasks.set(this.linkedTasks().map(t => t.id === task.id ? { ...t, flagged } : t));
    const selLinked = this.selectedLinkedTask();
    if (selLinked && selLinked.id === task.id) {
      this.selectedLinkedTask.set({ ...selLinked, flagged });
    }

    // Persist to server — reload on failure
    this.tasksApi.update(task.id, { flagged }).subscribe({
      error: () => {
        const sel = this.selected();
        if (sel) this.loadLinkedTasks(sel.id);
      },
    });
  }

  onLinkedTaskAddDep(depId: string): void {
    const task = this.selectedLinkedTask();
    if (!task) return;
    this.tasksApi.addDependency(task.id, depId).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onLinkedTaskRemoveDep(depId: string): void {
    const task = this.selectedLinkedTask();
    if (!task) return;
    this.tasksApi.removeDependency(task.id, depId).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onLinkedTaskPlaybookChange(playbookId: string | null): void {
    const task = this.selectedLinkedTask();
    if (!task) return;
    this.tasksApi.update(task.id, { playbook_id: playbookId, playbook_step: playbookId ? 0 : null }).subscribe({
      next: () => {
        const sel = this.selected();
        if (sel) this.loadLinkedTasks(sel.id);
      },
    });
  }

  onLinkedTaskPlaybookStepChange(step: number): void {
    const task = this.selectedLinkedTask();
    if (!task) return;
    this.tasksApi.update(task.id, { playbook_step: step }).subscribe({
      next: () => {
        const sel = this.selected();
        if (sel) this.loadLinkedTasks(sel.id);
      },
    });
  }

  // --- Unlinked tasks ---

  loadUnlinkedTasks(): void {
    this.unlinkedTasksLoading.set(true);
    this.tasksApi.list({ unlinked: true, limit: 200 }).subscribe({
      next: (res) => {
        this.unlinkedTasks.set(res.data);
        this.unlinkedTasksLoading.set(false);
      },
      error: () => {
        this.unlinkedTasks.set([]);
        this.unlinkedTasksLoading.set(false);
      },
    });
  }

  selectUnlinkedTask(task: SpTask | null): void {
    if (task && this.selectedUnlinkedTask()?.id === task.id) {
      this.selectedUnlinkedTask.set(null);
      this.stopDetailPolling();
      return;
    }
    this.selectedLinkedTask.set(null);
    this.selectedUnlinkedTask.set(task);
    this.resetTaskDetail();
    if (task) {
      this.loadTaskDetail(task.id);
    } else {
      this.stopDetailPolling();
    }
  }

  onUnlinkedTaskStateChange(task: SpTask, target: string): void {
    this.tasksApi.transition(task.id, target).subscribe({
      next: () => this.loadUnlinkedTasks(),
    });
  }

  onUnlinkedTaskDelete(): void {
    const task = this.selectedUnlinkedTask();
    if (!task) return;
    this.tasksApi.delete(task.id).subscribe({
      next: () => {
        this.selectedUnlinkedTask.set(null);
        this.loadUnlinkedTasks();
      },
    });
  }

  onUnlinkedTaskPostUpdate(event: { kind: string; content: string }): void {
    const task = this.selectedUnlinkedTask();
    if (!task) return;
    this.tasksApi.createUpdate(task.id, event).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onUnlinkedTaskPostComment(content: string): void {
    const task = this.selectedUnlinkedTask();
    if (!task) return;
    this.tasksApi.createComment(task.id, { content }).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onUnlinkedTaskInlineUpdate(data: UpdateTaskRequest): void {
    const task = this.selectedUnlinkedTask();
    if (!task) return;
    this.tasksApi.update(task.id, data).subscribe({
      next: () => this.loadUnlinkedTasks(),
    });
  }

  onUnlinkedTaskFlagToggle(task: SpTask, flagged: boolean): void {
    this.tasksApi.update(task.id, { flagged }).subscribe({
      next: () => this.loadUnlinkedTasks(),
    });
  }

  onUnlinkedTaskAddDep(depId: string): void {
    const task = this.selectedUnlinkedTask();
    if (!task) return;
    this.tasksApi.addDependency(task.id, depId).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onUnlinkedTaskRemoveDep(depId: string): void {
    const task = this.selectedUnlinkedTask();
    if (!task) return;
    this.tasksApi.removeDependency(task.id, depId).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onUnlinkedTaskPlaybookChange(playbookId: string | null): void {
    const task = this.selectedUnlinkedTask();
    if (!task) return;
    this.tasksApi.update(task.id, { playbook_id: playbookId, playbook_step: playbookId ? 0 : null }).subscribe({
      next: () => this.loadUnlinkedTasks(),
    });
  }

  onUnlinkedTaskPlaybookStepChange(step: number): void {
    const task = this.selectedUnlinkedTask();
    if (!task) return;
    this.tasksApi.update(task.id, { playbook_step: step }).subscribe({
      next: () => this.loadUnlinkedTasks(),
    });
  }

  // --- Task form (goal-linked only) ---

  openCreateTaskForGoal(): void {
    this.editingTask.set(null);
    this.showTaskForm.set(true);
  }

  closeTaskForm(): void {
    this.showTaskForm.set(false);
  }

  onCreateTask(req: CreateTaskRequest): void {
    const sel = this.selected();
    if (!sel) return;
    req.work_id = sel.id;
    this.tasksApi.create(req).subscribe({
      next: () => {
        this.closeTaskForm();
        this.loadLinkedTasks(sel.id);
        this.loadAllProgress([sel]);
        this.loadStatsAndChildren(sel.id);
      },
    });
  }

  executeWorkItem(): void {
    const sel = this.selected();
    if (!sel) return;
    this.executeLoading.set(true);
    const context: Record<string, unknown> = {};
    if (sel.description) {
      context['spec'] = sel.description;
    }
    if (sel.success_criteria) {
      context['acceptance_criteria'] = sel.success_criteria
        .split('\n')
        .map((l) => l.trim())
        .filter((l) => l.length > 0);
    }
    const req: CreateTaskRequest = {
      title: sel.title,
      work_id: sel.id,
      context,
    };
    this.tasksApi.create(req).subscribe({
      next: () => {
        this.executeLoading.set(false);
        this.loadLinkedTasks(sel.id);
        this.loadAllProgress([sel]);
        this.loadStatsAndChildren(sel.id);
      },
      error: () => {
        this.executeLoading.set(false);
      },
    });
  }

  onUpdateTaskForGoal(event: { id: string; data: UpdateTaskRequest }): void {
    this.tasksApi.update(event.id, event.data).subscribe({
      next: () => {
        this.closeTaskForm();
        const sel = this.selected();
        if (sel) {
          this.loadLinkedTasks(sel.id);
        }
      },
    });
  }

  // --- Task Picker ---

  openTaskPicker(): void {
    this.pickerSelectedIds.set(new Set());
    this.pickerSearch = '';
    this.pickerStateFilter = '';
    this.pickerUnlinkedOnly = true;
    this.showTaskPicker.set(true);
    this.loadPickerTasks();
  }

  closeTaskPicker(): void {
    this.showTaskPicker.set(false);
  }

  loadPickerTasks(): void {
    this.pickerLoading.set(true);
    this.tasksApi.list({
      search: this.pickerSearch || undefined,
      state: this.pickerStateFilter || undefined,
      unlinked: this.pickerUnlinkedOnly || undefined,
      limit: 50,
    }).subscribe({
      next: (res) => {
        this.pickerTasks.set(res.data);
        this.pickerLoading.set(false);
      },
      error: () => {
        this.pickerTasks.set([]);
        this.pickerLoading.set(false);
      },
    });
  }

  togglePickerTask(taskId: string): void {
    const current = new Set(this.pickerSelectedIds());
    if (current.has(taskId)) {
      current.delete(taskId);
    } else {
      current.add(taskId);
    }
    this.pickerSelectedIds.set(current);
  }

  linkSelectedTasks(): void {
    const sel = this.selected();
    const ids = Array.from(this.pickerSelectedIds());
    if (!sel || ids.length === 0) return;
    this.api.bulkLinkTasks(sel.id, ids).subscribe({
      next: () => {
        this.closeTaskPicker();
        this.loadLinkedTasks(sel.id);
        this.loadAllProgress([sel]);
        this.loadStatsAndChildren(sel.id);
      },
    });
  }

  // --- Plan tasks ---

  planTasksForGoal(): void {
    const sel = this.selected();
    if (!sel) return;
    this.planLoading.set(true);
    this.planStatusMessage.set('');
    this.api.planTasksStream(sel.id).subscribe({
      next: (event) => {
        if (event.type === 'status') {
          this.planStatusMessage.set(event.message);
        } else if (event.type === 'done') {
          this.plannedTasks.set(event.tasks);
          this.planSuccessCriteria.set(event.success_criteria ?? []);
          this.planExpandedTasks.set(new Set());
          this.showPlanDialog.set(true);
          this.planLoading.set(false);
          this.planStatusMessage.set('');
        }
      },
      error: () => {
        this.planLoading.set(false);
        this.planStatusMessage.set('');
        alert(this.planErrorMessage);
      },
    });
  }

  private readonly planErrorMessage = 'Failed to generate task plan. Please try again.';

  closePlanDialog(): void {
    this.showPlanDialog.set(false);
    this.plannedTasks.set([]);
    this.planSuccessCriteria.set([]);
    this.planCreating.set(false);
    this.planExpandedTasks.set(new Set());
  }

  removePlannedTask(index: number): void {
    const tasks = [...this.plannedTasks()];
    tasks.splice(index, 1);
    this.plannedTasks.set(tasks);
    // Clean up expanded state
    const expanded = new Set<number>();
    for (const i of this.planExpandedTasks()) {
      if (i < index) expanded.add(i);
      else if (i > index) expanded.add(i - 1);
    }
    this.planExpandedTasks.set(expanded);
  }

  togglePlanTaskExpanded(index: number): void {
    const current = new Set(this.planExpandedTasks());
    if (current.has(index)) {
      current.delete(index);
    } else {
      current.add(index);
    }
    this.planExpandedTasks.set(current);
  }

  planKindColor(kind: string): string {
    const colors: Record<string, string> = {
      feature: 'bg-ctp-blue/20 text-ctp-blue',
      bug: 'bg-ctp-red/20 text-ctp-red',
      refactor: 'bg-ctp-peach/20 text-ctp-peach',
      test: 'bg-ctp-green/20 text-ctp-green',
      docs: 'bg-ctp-lavender/20 text-ctp-lavender',
    };
    return colors[kind] ?? 'bg-ctp-overlay0/20 text-ctp-overlay0';
  }

  confirmPlan(): void {
    const sel = this.selected();
    if (!sel) return;
    const tasks = this.plannedTasks();
    if (tasks.length === 0) return;

    this.planCreating.set(true);

    // Step 1: Create tasks sequentially to collect IDs in order
    const createdIds: string[] = [];
    from(tasks.map((task, idx) => ({ task, idx }))).pipe(
      concatMap(({ task }) => {
        const req: CreateTaskRequest = {
          title: task.title,
          kind: task.kind,
          work_id: sel.id,
          context: {
            spec: task.spec,
            acceptance_criteria: task.acceptance_criteria,
          },
        };
        return this.tasksApi.create(req).pipe(
          catchError(() => of(null)),
        );
      }),
      toArray(),
    ).subscribe(results => {
      let failed = 0;
      for (const result of results) {
        if (result) {
          createdIds.push(result.id);
        } else {
          createdIds.push(''); // placeholder for failed task
          failed++;
        }
      }

      if (failed === tasks.length) {
        this.planCreating.set(false);
        alert(this.planErrorMessage);
        return;
      }

      // Step 2: Wire dependencies
      const depCalls: { taskId: string; dependsOn: string }[] = [];
      for (let i = 0; i < tasks.length; i++) {
        const deps = tasks[i].depends_on;
        if (!deps || !createdIds[i]) continue;
        for (const depIdx of deps) {
          if (depIdx >= 0 && depIdx < createdIds.length && createdIds[depIdx]) {
            depCalls.push({ taskId: createdIds[i], dependsOn: createdIds[depIdx] });
          }
        }
      }

      if (depCalls.length === 0) {
        this.finishPlanCreation(sel);
        return;
      }

      from(depCalls).pipe(
        mergeMap(({ taskId, dependsOn }) =>
          this.tasksApi.addDependency(taskId, dependsOn).pipe(catchError(() => EMPTY)),
          4, // concurrency limit
        ),
        toArray(),
      ).subscribe(() => {
        this.finishPlanCreation(sel);
      });
    });
  }

  private finishPlanCreation(sel: SpWork): void {
    this.planCreating.set(false);
    this.closePlanDialog();
    this.loadLinkedTasks(sel.id);
    this.loadAllProgress([sel]);
    this.loadStatsAndChildren(sel.id);
    this.loadGoals();
  }

  // --- Marked tasks ---

  toggleMarkTask(taskId: string): void {
    const goalId = this.selected()?.id;
    if (!goalId) return;
    const current = new Set(this.markedTaskIds());
    if (current.has(taskId)) {
      current.delete(taskId);
    } else {
      current.add(taskId);
    }
    this.markedTaskIds.set(current);
    this.saveMarkedTasks(goalId, current);
  }

  private loadMarkedTasks(goalId: string): void {
    try {
      const raw = localStorage.getItem(`goals:${goalId}:marked`);
      this.markedTaskIds.set(raw ? new Set(JSON.parse(raw)) : new Set());
    } catch {
      this.markedTaskIds.set(new Set());
    }
  }

  private saveMarkedTasks(goalId: string, ids: Set<string>): void {
    try {
      if (ids.size === 0) {
        localStorage.removeItem(`goals:${goalId}:marked`);
      } else {
        localStorage.setItem(`goals:${goalId}:marked`, JSON.stringify([...ids]));
      }
    } catch {
      // localStorage unavailable — marks are ephemeral
    }
  }
}
