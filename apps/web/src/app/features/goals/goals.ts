import { Component, inject, signal, computed, effect } from '@angular/core';
import { NgTemplateOutlet, DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { forkJoin, of } from 'rxjs';
import { catchError } from 'rxjs/operators';
import { ProjectContext } from '../../core/services/project-context.service';
import {
  GoalsApiService,
  SpGoal,
  GoalStatus,
  GoalType,
  GoalTodo,
  SpGoalCreate,
  SpGoalProgress,
  SpGoalStats,
} from '../../core/services/goals-api.service';
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

const STATUSES: GoalStatus[] = ['active', 'achieved', 'paused', 'abandoned'];

const STATUS_COLORS: Record<GoalStatus, string> = {
  active: 'bg-ctp-green/20 text-ctp-green',
  achieved: 'bg-ctp-blue/20 text-ctp-blue',
  paused: 'bg-ctp-yellow/20 text-ctp-yellow',
  abandoned: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

const PROGRESS_COLORS: Record<GoalStatus, string> = {
  active: 'bg-ctp-green',
  achieved: 'bg-ctp-blue',
  paused: 'bg-ctp-yellow',
  abandoned: 'bg-ctp-overlay0',
};

const GOAL_TYPES: GoalType[] = ['epic', 'feature', 'milestone', 'sprint', 'initiative'];

const TYPE_COLORS: Record<GoalType, string> = {
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
  imports: [TranslocoModule, FormsModule, DatePipe, NgTemplateOutlet, TaskFormComponent, TaskListComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.work') }}</h1>
        <div class="flex items-center gap-3">
          <button (click)="openCreateStandaloneTask()" class="px-4 py-2 bg-ctp-green text-ctp-base rounded-lg text-sm font-medium hover:opacity-90">
            {{ t('tasks.create') }}
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
          [(ngModel)]="selectedGoalType"
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
              <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ typeColor(goal.goal_type) }}">
                {{ t('goals.type.' + goal.goal_type) }}
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
              @if (goal.target_date) {
                <p class="text-xs text-text-muted mt-1 ml-6">{{ t('goals.targetDate') }}: {{ goal.target_date | date:'mediumDate' }}</p>
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
                <select [(ngModel)]="formGoalType" (change)="saveInlineField()"
                  class="text-xs rounded-lg px-2 py-1 border border-border bg-surface text-text-primary
                         focus:outline-none focus:ring-1 focus:ring-accent">
                  @for (gt of goalTypes; track gt) {
                    <option [value]="gt">{{ t('goals.type.' + gt) }}</option>
                  }
                </select>
                <div class="flex items-center gap-1">
                  <span class="text-xs text-text-secondary">P</span>
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

              <!-- Target date -->
              <div class="mb-4">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('goals.targetDate') }}</h3>
                <input type="date" [(ngModel)]="formTargetDate" (change)="saveInlineField()"
                  class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent" />
              </div>

              <!-- Linked Tasks -->
              <div class="pt-3 border-t border-border mb-3">
                <div class="flex items-center justify-between mb-2">
                  <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider">{{ t('goals.linkedTasks') }}</h3>
                  <div class="flex gap-2">
                    <button (click)="openCreateTaskForGoal()" class="px-3 py-1.5 text-xs bg-ctp-green text-bg rounded hover:opacity-90">
                      {{ t('goals.createTaskBtn') }}
                    </button>
                    <button (click)="openTaskPicker()" class="px-3 py-1.5 text-xs bg-accent text-bg rounded hover:opacity-90">
                      {{ t('goals.linkTasksBtn') }}
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
        <!-- Section: Active Goals -->
        @if (activeGoals().length > 0) {
          <div class="mb-6">
            <h2 class="text-sm font-semibold text-text-secondary uppercase tracking-wider mb-2 flex items-center gap-2">
              {{ t('work.activeGoals') }}
              <span class="text-xs font-normal">({{ activeGoals().length }})</span>
            </h2>
            <div class="space-y-2">
              @for (g of activeGoals(); track g.id) {
                <ng-container *ngTemplateOutlet="goalItem; context: { $implicit: g }"></ng-container>
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
                <label for="goal-target-date" class="block text-sm text-text-secondary mb-1">{{ t('goals.targetDate') }}</label>
                <input id="goal-target-date" type="date" [(ngModel)]="formTargetDate"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent" />
              </div>
              <div>
                <label for="goal-type" class="block text-sm text-text-secondary mb-1">{{ t('goals.fieldType') }}</label>
                <select id="goal-type" [(ngModel)]="formGoalType"
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
                          @if (task.priority > 0) {
                            <span class="text-xs text-text-secondary">P{{ task.priority }}</span>
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
  private api = inject(GoalsApiService);
  private tasksApi = inject(TasksApiService);
  private verificationsApi = inject(VerificationsApiService);
  private playbooksApi = inject(PlaybooksApiService);
  private ctx = inject(ProjectContext);

  readonly statuses = STATUSES;
  readonly goalTypes = GOAL_TYPES;
  readonly taskStates = TASK_STATES;

  items = signal<SpGoal[]>([]);
  loading = signal(false);
  selected = signal<SpGoal | null>(null);
  searchQuery = signal('');
  selectedStatus = '';
  selectedGoalType = '';
  progressMap = signal<Map<string, SpGoalProgress>>(new Map());
  statsMap = signal<Map<string, SpGoalStats>>(new Map());
  childrenMap = signal<Map<string, SpGoal[]>>(new Map());

  showForm = signal(false);
  editing = signal<SpGoal | null>(null);
  formTitle = '';
  formDescription = '';
  formCriteria = '';
  formTargetDate = '';
  formGoalType: GoalType = 'epic';
  formPriority = 0;
  formAutoStatus = false;

  // Todos
  goalTodos = signal<GoalTodo[]>([]);
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
  private creatingForGoal = false;

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

  // Task detail data (shared between linked and unlinked task expansion)
  selectedLinkedTask = signal<SpTask | null>(null);
  selectedUnlinkedTask = signal<SpTask | null>(null);
  taskDetailUpdates = signal<SpTaskUpdate[]>([]);
  taskDetailComments = signal<SpTaskComment[]>([]);
  taskDetailDependencies = signal<SpTaskDependencies>({ depends_on: [], blocks: [] });
  taskDetailVerifications = signal<SpVerification[]>([]);
  taskDetailChangedFiles = signal<ChangedFileSummary[]>([]);
  taskDetailGitStatus = signal<import('../../core/services/git-api.service').TaskBranchStatus | null>(null);
  taskDetailLoading = signal(false);
  taskDetailPushing = signal(false);
  taskDetailReverting = signal(false);
  taskDetailResolving = signal(false);
  goalPlaybooks = signal<SpPlaybook[]>([]);
  editingTask = signal<SpTask | null>(null);

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
    });
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

  // --- Goals ---

  loadGoals(): void {
    this.loading.set(true);
    const status = this.selectedStatus as GoalStatus | '';
    const goalType = this.selectedGoalType as GoalType | '';
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

  private loadAllProgress(goals: SpGoal[]): void {
    const map = new Map<string, SpGoalProgress>();
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

  private loadAllStats(goals: SpGoal[]): void {
    const map = new Map<string, SpGoalStats>(this.statsMap());
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

  selectItem(goal: SpGoal): void {
    this.selected.set(goal.id === this.selected()?.id ? null : goal);
    this.statsFilter.set('');
    this.selectedLinkedTask.set(null);
    if (this.selected()) {
      this.formTitle = goal.title;
      this.formDescription = goal.description;
      this.formCriteria = goal.success_criteria;
      this.formTargetDate = goal.target_date ?? '';
      this.formGoalType = goal.goal_type;
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

  statusColor(status: GoalStatus): string {
    return STATUS_COLORS[status] ?? '';
  }

  progressColor(status: GoalStatus): string {
    return PROGRESS_COLORS[status] ?? '';
  }

  typeColor(type: GoalType): string {
    return TYPE_COLORS[type] ?? '';
  }

  setStatsFilter(state: string): void {
    this.statsFilter.set(this.statsFilter() === state ? '' : state);
  }

  clearStatsFilter(): void {
    this.statsFilter.set('');
  }

  transitionStatus(goal: SpGoal, newStatus: GoalStatus): void {
    this.api.update(goal.id, { status: newStatus }).subscribe({
      next: () => this.loadGoals(),
    });
  }

  openCreate(): void {
    this.editing.set(null);
    this.formTitle = '';
    this.formDescription = '';
    this.formCriteria = '';
    this.formTargetDate = '';
    this.formGoalType = 'epic';
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
      (sel.target_date ?? '') === this.formTargetDate &&
      sel.goal_type === this.formGoalType &&
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
        target_date: this.formTargetDate || null,
        goal_type: this.formGoalType,
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
    const targetDate = this.formTargetDate || null;
    const existing = this.editing();
    if (existing) {
      this.api.update(existing.id, {
        title: this.formTitle,
        description: this.formDescription,
        success_criteria: this.formCriteria,
        target_date: targetDate,
        goal_type: this.formGoalType,
        priority: this.formPriority,
        auto_status: this.formAutoStatus,
      }).subscribe({
        next: () => {
          this.closeForm();
          this.loadGoals();
        },
      });
    } else {
      const data: SpGoalCreate = {
        title: this.formTitle,
        description: this.formDescription,
        success_criteria: this.formCriteria,
        target_date: targetDate,
        goal_type: this.formGoalType,
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

  confirmDelete(goal: SpGoal): void {
    this.api.delete(goal.id).subscribe({
      next: () => {
        this.selected.set(null);
        this.loadGoals();
      },
    });
  }

  // --- Todos ---

  private loadTodos(goal: SpGoal): void {
    const raw = (goal.metadata?.['todos'] as GoalTodo[] | undefined) ?? [];
    this.goalTodos.set(raw);
    this.nextTodoId = raw.reduce((max, t) => Math.max(max, t.id + 1), 1);
    this.newTodoText = '';
  }

  addTodo(): void {
    const text = this.newTodoText.trim();
    if (!text) return;
    const todo: GoalTodo = { id: this.nextTodoId++, text, done: false };
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

  private saveTodos(todos: GoalTodo[]): void {
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
      return;
    }
    this.selectedLinkedTask.set(task);
    this.resetTaskDetail();
    if (task) {
      this.loadTaskDetail(task.id);
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
    forkJoin({
      updates: this.tasksApi.listUpdates(taskId).pipe(catchError(() => of([] as SpTaskUpdate[]))),
      comments: this.tasksApi.listComments(taskId).pipe(catchError(() => of([] as SpTaskComment[]))),
      dependencies: this.tasksApi.listDependencies(taskId).pipe(catchError(() => of({ depends_on: [], blocks: [] } as SpTaskDependencies))),
      changedFiles: this.tasksApi.listChangedFiles(taskId).pipe(catchError(() => of([] as ChangedFileSummary[]))),
      verifications: this.verificationsApi.list({ task_id: taskId, limit: 20 }).pipe(
        catchError(() => of({ data: [] as SpVerification[], total: 0, limit: 20, offset: 0, has_more: false })),
      ),
    }).subscribe({
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
      return;
    }
    this.selectedLinkedTask.set(null);
    this.selectedUnlinkedTask.set(task);
    this.resetTaskDetail();
    if (task) {
      this.loadTaskDetail(task.id);
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

  // --- Task form (shared between goal-linked and standalone) ---

  openCreateStandaloneTask(): void {
    this.creatingForGoal = false;
    this.editingTask.set(null);
    this.showTaskForm.set(true);
  }

  openCreateTaskForGoal(): void {
    this.creatingForGoal = true;
    this.editingTask.set(null);
    this.showTaskForm.set(true);
  }

  closeTaskForm(): void {
    this.showTaskForm.set(false);
  }

  onCreateTask(req: CreateTaskRequest): void {
    if (this.creatingForGoal) {
      const sel = this.selected();
      if (!sel) return;
      req.goal_id = sel.id;
      this.tasksApi.create(req).subscribe({
        next: () => {
          this.closeTaskForm();
          this.loadLinkedTasks(sel.id);
          this.loadAllProgress([sel]);
          this.loadStatsAndChildren(sel.id);
        },
      });
    } else {
      this.tasksApi.create(req).subscribe({
        next: () => {
          this.closeTaskForm();
          this.loadUnlinkedTasks();
        },
      });
    }
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
