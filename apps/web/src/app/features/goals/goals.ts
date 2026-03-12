import { Component, inject, signal, computed, effect, HostListener } from '@angular/core';
import { DatePipe, SlicePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { ProjectContext } from '../../core/services/project-context.service';
import {
  GoalsApiService,
  SpGoal,
  SpGoalComment,
  GoalStatus,
  GoalType,
  GoalTodo,
  SpGoalCreate,
  SpGoalProgress,
  SpGoalStats,
} from '../../core/services/goals-api.service';
import { TasksApiService, SpTask, CreateTaskRequest, UpdateTaskRequest } from '../../core/services/tasks-api.service';
import { TaskFormComponent } from '../tasks/components/task-form/task-form';
import { taskStateColor as sharedTaskStateColor, taskTransitions, TASK_PRIORITY_LABELS } from '../../shared/ui-constants';

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
  selector: 'app-goals',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, SlicePipe, TaskFormComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.goals') }}</h1>
        <button (click)="openCreate()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
          {{ t('goals.create') }}
        </button>
      </div>

      <!-- Filters -->
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

      <!-- Content: accordion list -->
      @if (loading()) {
        <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
      } @else if (filtered().length === 0) {
        <p class="text-text-secondary text-sm">{{ t('common.empty') }}</p>
      } @else {
        <div class="space-y-2">
          @for (goal of filtered(); track goal.id) {
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
                        <button (click)="openCreateTask()" class="px-3 py-1.5 text-xs bg-ctp-green text-bg rounded hover:opacity-90">
                          {{ t('goals.createTaskBtn') }}
                        </button>
                        <button (click)="openTaskPicker()" class="px-3 py-1.5 text-xs bg-accent text-bg rounded hover:opacity-90">
                          {{ t('goals.linkTasksBtn') }}
                        </button>
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
                      <div class="space-y-1 max-h-[500px] overflow-y-auto">
                        @for (task of filteredLinkedTasks(); track task.id) {
                          <div class="rounded-lg border border-border overflow-hidden transition-colors group"
                               [class.ring-1]="expandedTaskIds().has(task.id)"
                               [class.ring-accent]="expandedTaskIds().has(task.id)">
                            <!-- Accordion header -->
                            <div class="flex items-center gap-2 px-3 py-2 cursor-pointer hover:bg-surface-hover"
                                 role="button"
                                 tabindex="0"
                                 (click)="toggleTaskExpand(task.id)"
                                 (keydown.enter)="toggleTaskExpand(task.id)"
                                 (keydown.space)="$event.preventDefault(); toggleTaskExpand(task.id)">
                              <svg class="w-3.5 h-3.5 text-text-secondary shrink-0 transition-transform duration-150"
                                   [class.rotate-90]="expandedTaskIds().has(task.id)"
                                   fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
                              </svg>
                              <span class="text-text-muted text-xs shrink-0">#{{ task.number }}</span>
                              <span class="text-text-primary text-sm truncate flex-1 min-w-0">{{ task.title }}</span>
                              <span class="px-1.5 py-0.5 rounded-full text-xs shrink-0 bg-ctp-blue/20 text-ctp-blue">{{ task.kind }}</span>
                              <!-- State badge (clickable for transitions) -->
                              <div class="relative shrink-0">
                                <button class="px-1.5 py-0.5 rounded-full text-xs font-medium cursor-pointer hover:ring-1 hover:ring-accent {{ taskStateColor(task.state) }}"
                                        (click)="toggleTaskStateMenu($event, task.id)">
                                  {{ task.state }}
                                </button>
                                @if (openTaskMenuId() === task.id) {
                                  <div class="absolute z-50 mt-1 right-0 bg-surface border border-border rounded-lg shadow-lg py-1 min-w-[120px]">
                                    @for (target of getTaskTransitions(task.state); track target) {
                                      <button (click)="onTaskTransition($event, task, target)"
                                        class="w-full text-left px-3 py-1.5 text-xs hover:bg-surface-hover transition-colors flex items-center gap-2 cursor-pointer">
                                        <span class="w-2 h-2 rounded-full {{ taskStateColor(target) }}"></span>
                                        <span class="text-text-primary">{{ target }}</span>
                                      </button>
                                    } @empty {
                                      <span class="px-3 py-1.5 text-xs text-text-muted">{{ t('tasks.noTransitions') }}</span>
                                    }
                                  </div>
                                }
                              </div>
                              <span class="text-xs shrink-0 {{ taskPriorityInfo(task.priority).color }}">
                                {{ taskPriorityInfo(task.priority).label }}
                              </span>
                              <!-- Edit button -->
                              <button (click)="editTask($event, task)"
                                class="p-1 text-text-secondary hover:text-accent shrink-0 opacity-0 group-hover:opacity-100 transition-opacity"
                                [title]="t('common.edit')">
                                <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                                  <path d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                                </svg>
                              </button>
                              <!-- Mark/bookmark button -->
                              <button (click)="toggleMarkTask(task.id); $event.stopPropagation()"
                                class="p-1 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity"
                                [class.text-ctp-yellow]="markedTaskIds().has(task.id)"
                                [class.opacity-100]="markedTaskIds().has(task.id)"
                                [class.text-text-secondary]="!markedTaskIds().has(task.id)"
                                [title]="markedTaskIds().has(task.id) ? t('goals.unmarkTask') : t('goals.markTask')">
                                <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                                  @if (markedTaskIds().has(task.id)) {
                                    <path d="M5 2h14a1 1 0 011 1v19.143a.5.5 0 01-.766.424L12 18.03l-7.234 4.536A.5.5 0 014 22.143V3a1 1 0 011-1z" />
                                  } @else {
                                    <path d="M5 2h14a1 1 0 011 1v19.143a.5.5 0 01-.766.424L12 18.03l-7.234 4.536A.5.5 0 014 22.143V3a1 1 0 011-1zm1 2v15.432l6-3.761 6 3.761V4H6z" />
                                  }
                                </svg>
                              </button>
                              <!-- Unlink button -->
                              <button (click)="unlinkSingleTask(task.id); $event.stopPropagation()"
                                class="p-1 text-text-secondary hover:text-ctp-red shrink-0 opacity-0 group-hover:opacity-100 transition-opacity"
                                [title]="t('goals.unlink')">
                                <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                                  <path d="M6 18L18 6M6 6l12 12" />
                                </svg>
                              </button>
                            </div>
                            <!-- Expanded detail -->
                            @if (expandedTaskIds().has(task.id)) {
                              <div class="px-3 pb-3 ml-6 space-y-1.5 border-t border-border pt-2.5 text-xs">
                                <div class="flex justify-between">
                                  <span class="text-text-muted">{{ t('tasks.created') }}</span>
                                  <span class="text-text-secondary">{{ task.created_at | date:'MMM d, y HH:mm' }}</span>
                                </div>
                                <div class="flex justify-between">
                                  <span class="text-text-muted">{{ t('tasks.agent') }}</span>
                                  <span class="text-text-secondary font-mono">{{ task.assigned_agent_id ? (task.assigned_agent_id | slice:0:8) + '...' : '—' }}</span>
                                </div>
                                @if (task.context?.['spec']) {
                                  <div class="mt-2">
                                    <span class="text-text-muted">{{ t('tasks.spec') }}</span>
                                    <p class="text-text-secondary mt-0.5 whitespace-pre-line line-clamp-4">{{ task.context['spec'] }}</p>
                                  </div>
                                }
                              </div>
                            }
                          </div>
                        }
                      </div>
                    }
                  </div>

                  <!-- Comments / Notes -->
                  <div class="pt-3 border-t border-border mb-3">
                    <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{{ t('goals.comments') }}</h3>
                    <div class="flex gap-2 mb-3">
                      <input type="text" [(ngModel)]="newComment" [placeholder]="t('goals.commentPlaceholder')"
                        class="flex-1 bg-surface text-text-primary text-xs rounded px-2 py-1.5 border border-border
                               focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary"
                        (keydown.enter)="postComment()" />
                      <button (click)="postComment()" [disabled]="!newComment.trim()"
                        class="px-3 py-1.5 bg-accent text-bg rounded-lg text-xs font-medium hover:opacity-90 disabled:opacity-30">
                        {{ t('goals.post') }}
                      </button>
                    </div>
                    @if (commentsLoading()) {
                      <p class="text-text-muted text-xs">{{ t('common.loading') }}</p>
                    } @else {
                      <div class="space-y-2 max-h-48 overflow-y-auto">
                        @for (comment of comments(); track comment.id) {
                          <div class="text-xs">
                            <div class="flex items-center gap-2 mb-0.5">
                              <span class="text-text-muted">{{ formatTime(comment.created_at) }}</span>
                              <span class="font-medium text-ctp-mauve">{{ comment.agent_id ? 'assistant' : 'human' }}</span>
                            </div>
                            <p class="text-text-primary break-words">{{ comment.content }}</p>
                          </div>
                        } @empty {
                          <p class="text-text-muted text-xs">{{ t('goals.noComments') }}</p>
                        }
                      </div>
                    }
                  </div>

                  <div class="text-xs text-text-secondary">
                    {{ t('goals.updatedAt') }}: {{ goal.updated_at | date:'medium' }}
                  </div>
                </div>
              }
            </div>
          }
        </div>
      }

      <!-- Create/Edit modal -->
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
        (submitCreate)="onCreateTaskForGoal($event)"
        (submitUpdate)="onUpdateTaskForGoal($event)"
        (closed)="closeTaskForm()" />
    </div>
  `,
})
export class GoalsPage {
  private api = inject(GoalsApiService);
  private tasksApi = inject(TasksApiService);
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

  // Comments
  comments = signal<SpGoalComment[]>([]);
  commentsLoading = signal(false);
  newComment = '';

  // Linked tasks
  linkedTasks = signal<SpTask[]>([]);
  linkedTasksLoading = signal(false);
  statsFilter = signal<string>('');

  // Task create form (for creating a task linked to the selected goal)
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

  // Accordion state for linked tasks
  expandedTaskIds = signal<Set<string>>(new Set());
  openTaskMenuId = signal<string | null>(null);
  editingTask = signal<SpTask | null>(null);

  filtered = computed(() => {
    const q = this.searchQuery().toLowerCase().trim();
    const list = q
      ? this.items().filter(
          g => g.title.toLowerCase().includes(q) || g.description.toLowerCase().includes(q),
        )
      : this.items();
    // Sort achieved goals to the bottom, preserving relative order within each group
    return [...list].sort((a, b) => {
      const aAchieved = a.status === 'achieved' ? 1 : 0;
      const bAchieved = b.status === 'achieved' ? 1 : 0;
      return aAchieved - bAchieved;
    });
  });

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
      return tasks.filter(t => !GoalsPage.NON_WORKING_STATES.has(t.state));
    }
    // blocked is not a task state — we can't reliably filter client-side,
    // so show all tasks when blocked filter is active (the highlight still indicates intent)
    if (filter === 'blocked') return tasks;
    return tasks.filter(t => t.state === filter);
  });

  constructor() {
    effect(() => {
      this.ctx.projectId();
      this.selected.set(null);
      this.loadGoals();
    });
  }

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
    // Also load stats for all goals so collapsed view can show working indicators
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
    if (this.selected()) {
      // Populate inline edit fields
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
      this.loadComments(goal.id);
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

  taskStateColor(state: string): string {
    return sharedTaskStateColor(state);
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
    // Only save if something actually changed
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

  // --- Todos (stored in goal.metadata.todos) ---

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
        // Update local goal metadata to keep in sync
        sel.metadata = metadata;
      },
    });
  }

  // --- Comments ---

  private loadComments(goalId: string): void {
    this.commentsLoading.set(true);
    this.api.listComments(goalId).subscribe({
      next: (comments) => {
        this.comments.set(comments);
        this.commentsLoading.set(false);
      },
      error: () => {
        this.comments.set([]);
        this.commentsLoading.set(false);
      },
    });
  }

  postComment(): void {
    const content = this.newComment.trim();
    const sel = this.selected();
    if (!content || !sel) return;
    this.api.createComment(sel.id, content).subscribe({
      next: () => {
        this.newComment = '';
        this.loadComments(sel.id);
      },
    });
  }

  formatTime(iso: string): string {
    return iso?.substring(11, 16) ?? '??:??';
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

  // --- Task Accordion ---

  @HostListener('document:click')
  closeTaskMenu(): void {
    this.openTaskMenuId.set(null);
  }

  toggleTaskExpand(taskId: string): void {
    const next = new Set(this.expandedTaskIds());
    if (next.has(taskId)) {
      next.delete(taskId);
    } else {
      next.add(taskId);
    }
    this.expandedTaskIds.set(next);
  }

  toggleTaskStateMenu(event: Event, taskId: string): void {
    event.stopPropagation();
    this.openTaskMenuId.set(this.openTaskMenuId() === taskId ? null : taskId);
  }

  onTaskTransition(event: Event, task: SpTask, target: string): void {
    event.stopPropagation();
    this.openTaskMenuId.set(null);
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

  editTask(event: Event, task: SpTask): void {
    event.stopPropagation();
    this.editingTask.set(task);
    this.showTaskForm.set(true);
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

  protected readonly getTaskTransitions = taskTransitions;

  taskPriorityInfo(priority: number): { label: string; color: string } {
    return TASK_PRIORITY_LABELS[priority] ?? { label: String(priority), color: 'text-text-secondary' };
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

  // --- Create Task for Goal ---

  openCreateTask(): void {
    this.editingTask.set(null);
    this.showTaskForm.set(true);
  }

  closeTaskForm(): void {
    this.showTaskForm.set(false);
  }

  onCreateTaskForGoal(req: CreateTaskRequest): void {
    const sel = this.selected();
    if (!sel) return;
    // Pass goal_id so the API atomically links the task to the goal
    req.goal_id = sel.id;
    this.tasksApi.create(req).subscribe({
      next: () => {
        this.closeTaskForm();
        this.loadLinkedTasks(sel.id);
        this.loadAllProgress([sel]);
        this.loadStatsAndChildren(sel.id);
      },
    });
  }

  // --- Marked tasks (goal-scoped, localStorage) ---

  sortedLinkedTasks = computed(() => {
    const tasks = this.linkedTasks();
    const marked = this.markedTaskIds();
    return [...tasks].sort((a, b) => {
      const aMarked = marked.has(a.id) ? 0 : 1;
      const bMarked = marked.has(b.id) ? 0 : 1;
      return aMarked - bMarked;
    });
  });

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
