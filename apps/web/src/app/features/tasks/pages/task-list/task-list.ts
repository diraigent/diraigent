import { Component, input, output, signal, computed, HostListener, effect } from '@angular/core';
import { DatePipe, SlicePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { SpTask, SpTaskUpdate, SpTaskComment, SpTaskDependencies, ChangedFileSummary, UpdateTaskRequest } from '../../../../core/services/tasks-api.service';
import { BranchInfo, TaskBranchStatus } from '../../../../core/services/git-api.service';
import { SpVerification } from '../../../../core/services/verifications-api.service';
import { SpPlaybook } from '../../../../core/services/playbooks-api.service';
import { taskStateColor, taskTransitions } from '../../../../shared/ui-constants';
import { TaskDetailComponent } from '../task-detail/task-detail';

type SortField = 'title' | 'kind' | 'state' | 'urgent' | 'created_at' | 'assigned_agent_id';
type SortDir = 'asc' | 'desc';

@Component({
  selector: 'app-task-list',
  standalone: true,
  imports: [TranslocoModule, FormsModule, SlicePipe, DatePipe, TaskDetailComponent],
  template: `
    <div *transloco="let t">
      @if (!compact()) {
      <!-- Filters -->
      <div class="flex flex-wrap gap-3 mb-4">
        <input
          type="text"
          [placeholder]="t('tasks.searchPlaceholder')"
          [ngModel]="searchQuery()"
          (ngModelChange)="searchChange.emit($event)"
          class="flex-1 min-w-[200px] bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
        <select
          [ngModel]="stateFilter()"
          (ngModelChange)="stateFilterChange.emit($event)"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('tasks.allStates') }}</option>
          @for (s of states(); track s) {
            <option [value]="s">{{ s }}</option>
          }
        </select>
        <select
          [ngModel]="kindFilter()"
          (ngModelChange)="kindFilterChange.emit($event)"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('tasks.allKinds') }}</option>
          @for (k of kinds(); track k) {
            <option [value]="k">{{ k }}</option>
          }
        </select>
        <button (click)="hideDoneChange.emit(!hideDone())"
          class="text-sm px-3 py-2 rounded-lg border transition-colors cursor-pointer"
          [class]="hideDone()
            ? 'bg-ctp-green/10 border-ctp-green/30 text-ctp-green hover:bg-ctp-green/20'
            : 'bg-surface border-border text-text-secondary hover:bg-surface-hover'">
          @if (hideDone()) {
            {{ t('tasks.hidingDone') }}
          } @else {
            {{ t('tasks.showingDone') }}
          }
        </button>
        <button (click)="unlinkedChange.emit(!unlinked())"
          class="text-sm px-3 py-2 rounded-lg border transition-colors cursor-pointer"
          [class]="unlinked()
            ? 'bg-ctp-peach/10 border-ctp-peach/30 text-ctp-peach hover:bg-ctp-peach/20'
            : 'bg-surface border-border text-text-secondary hover:bg-surface-hover'">
          @if (unlinked()) {
            {{ t('tasks.unlinkedOnly') }}
          } @else {
            {{ t('tasks.allTasks') }}
          }
        </button>
        <button (click)="hierarchyView.set(!hierarchyView())"
          class="text-sm px-3 py-2 rounded-lg border transition-colors cursor-pointer"
          [class]="hierarchyView()
            ? 'bg-ctp-teal/10 border-ctp-teal/30 text-ctp-teal hover:bg-ctp-teal/20'
            : 'bg-surface border-border text-text-secondary hover:bg-surface-hover'"
          [title]="t('tasks.hierarchyToggle')">
          @if (hierarchyView()) {
            {{ t('tasks.hierarchyView') }}
          } @else {
            {{ t('tasks.flatView') }}
          }
        </button>
      </div>
      }

      <!-- Bulk action toolbar -->
      @if (!compact() && selectedIds().size > 0) {
        <div class="flex flex-wrap items-center gap-2 sm:gap-3 mb-3 px-3 sm:px-4 py-2.5 bg-accent/10 border border-accent/30 rounded-lg text-sm">
          <span class="text-text-primary font-medium">
            {{ selectedIds().size }} {{ t('tasks.bulk.selected') }}
          </span>
          <div class="flex-1"></div>

          <!-- Bulk transition -->
          <div class="relative">
            <button (click)="toggleBulkTransitionMenu($event)"
              class="px-3 py-1.5 rounded bg-surface border border-border hover:bg-surface-hover text-text-primary text-xs font-medium cursor-pointer">
              {{ t('tasks.bulk.transition') }}
            </button>
            @if (bulkTransitionOpen()) {
              <div class="absolute z-50 mt-1 right-0 bg-surface border border-border rounded-lg shadow-lg py-1 min-w-[140px]">
                @for (target of bulkTransitionTargets; track target) {
                  <button (click)="onBulkTransition($event, target)"
                    class="w-full text-left px-3 py-1.5 text-xs hover:bg-surface-hover transition-colors flex items-center gap-2 cursor-pointer">
                    <span class="w-2 h-2 rounded-full {{ stateColor(target) }}"></span>
                    <span class="text-text-primary">{{ target }}</span>
                  </button>
                }
              </div>
            }
          </div>

          <!-- Bulk delete -->
          <button (click)="onBulkDelete($event)"
            class="px-3 py-1.5 rounded bg-ctp-red/10 border border-ctp-red/30 text-ctp-red text-xs font-medium hover:bg-ctp-red/20 cursor-pointer">
            {{ t('tasks.bulk.delete') }}
          </button>

          <!-- Clear selection -->
          <button (click)="clearSelection()"
            class="px-3 py-1.5 rounded bg-surface border border-border hover:bg-surface-hover text-text-secondary text-xs cursor-pointer">
            {{ t('tasks.bulk.clear') }}
          </button>
        </div>
      }

      @if (!compact()) {
      <!-- Sort & select-all bar -->
      <div class="flex items-center gap-2 px-1 mb-2">
        <input type="checkbox"
          [checked]="allSelected()"
          [indeterminate]="someSelected()"
          (change)="toggleSelectAll()"
          class="accent-accent cursor-pointer" />
        <span class="text-text-muted text-xs">{{ t('tasks.bulk.selected') }}</span>
        <div class="flex-1"></div>
        <!-- Mobile sort dropdown -->
        <select [ngModel]="sortField()" (ngModelChange)="onSortFieldChange($event)"
          class="md:hidden bg-surface text-text-secondary text-xs rounded px-2 py-1 border border-border focus:outline-none">
          <option value="created_at">{{ t('tasks.created') }}</option>
          <option value="title">{{ t('tasks.title') }}</option>
          <option value="state">{{ t('tasks.state') }}</option>
          <option value="urgent">{{ t('tasks.urgent') }}</option>
          <option value="kind">{{ t('tasks.kind') }}</option>
        </select>
        <!-- Desktop sort buttons -->
        <div class="hidden md:flex items-center gap-1 text-xs">
          @for (col of sortColumns; track col.field) {
            <button (click)="toggleSort(col.field)"
              class="px-2 py-1 rounded transition-colors cursor-pointer select-none"
              [class]="sortField() === col.field
                ? 'bg-accent/10 text-accent font-medium'
                : 'text-text-secondary hover:text-text-primary hover:bg-surface-hover'">
              {{ col.label }} {{ sortIndicator(col.field) }}
            </button>
          }
        </div>
        <button (click)="sortDir.set(sortDir() === 'asc' ? 'desc' : 'asc')"
          class="text-text-secondary text-xs px-1.5 py-1 rounded border border-border bg-surface hover:bg-surface-hover cursor-pointer">
          {{ sortDir() === 'asc' ? '▲' : '▼' }}
        </button>
      </div>
      }

      <!-- Accordion task list -->
      <div class="space-y-1">
        @for (task of sortedTasks(); track task.id) {
          <div class="bg-surface rounded-lg border border-border overflow-hidden transition-colors"
               [class.ring-1]="selectedId() === task.id"
               [class.ring-accent]="selectedId() === task.id"
               [class.bg-accent/5]="selectedIds().has(task.id) && selectedId() !== task.id"
               [class.ml-6]="hierarchyView() && task.parent_id && isChildVisible(task)"
               [attr.data-task-id]="task.id">
            <!-- Accordion header — always visible -->
            <div class="flex items-center gap-2 md:gap-3 px-3 py-2.5 cursor-pointer" tabindex="0" role="button"
                 (click)="taskSelect.emit(task)" (keydown.enter)="taskSelect.emit(task)">
              @if (!compact()) {
              <input type="checkbox"
                [checked]="selectedIds().has(task.id)"
                (change)="toggleSelect(task.id)"
                (click)="$event.stopPropagation()"
                class="accent-accent cursor-pointer shrink-0" />
              }
              <!-- Title area -->
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-1.5 flex-wrap">
                  @if (task.reverted_at) {
                    <span class="inline-flex items-center px-1 py-0.5 rounded text-[10px] font-medium bg-ctp-peach/15 text-ctp-peach" [title]="t('tasks.reverted')">
                      <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                        <path d="M3 10h10a5 5 0 015 5v2M3 10l4-4M3 10l4 4" />
                      </svg>
                    </span>
                  }
                  @if (blockedIds().has(task.id)) {
                    <span class="inline-flex items-center px-1 py-0.5 rounded text-[10px] font-medium bg-ctp-red/15 text-ctp-red" [title]="t('tasks.blocked')">
                      <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                        <path d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                      </svg>
                    </span>
                  }
                  @if (hierarchyView() && hasChildren(task.id)) {
                    <button class="inline-flex items-center px-1 py-0.5 rounded text-[10px] font-medium bg-ctp-teal/15 text-ctp-teal cursor-pointer hover:bg-ctp-teal/25"
                            (click)="toggleParentCollapse($event, task.id)"
                            [title]="collapsedParents().has(task.id) ? t('tasks.expandChildren') : t('tasks.collapseChildren')">
                      <svg class="w-3 h-3 transition-transform" [class.rotate-90]="!collapsedParents().has(task.id)" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                        <path d="M9 5l7 7-7 7" />
                      </svg>
                    </button>
                  }
                  @if (task.parent_id) {
                    <span class="inline-flex items-center px-1 py-0.5 rounded text-[10px] font-medium bg-ctp-teal/15 text-ctp-teal" [title]="t('tasks.hasParent')">
                      <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                        <path d="M9 5l7 7-7 7" />
                      </svg>
                    </span>
                  }
                  @if (goalLinkedIds().has(task.id)) {
                    <span class="inline-flex items-center px-1 py-0.5 rounded text-[10px] font-medium bg-ctp-mauve/15 text-ctp-mauve" [title]="t('tasks.linkedToGoal')">
                      <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                        <path d="M13 10V3L4 14h7v7l9-11h-7z" />
                      </svg>
                    </span>
                  }
                  @if (getBranch(task.id); as branch) {
                    @if (branch.is_pushed) {
                      <span class="inline-flex items-center px-1 py-0.5 rounded text-[10px] font-medium bg-ctp-green/15 text-ctp-green" title="Pushed">
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                          <path d="M5 13l4 4L19 7" />
                        </svg>
                      </span>
                    } @else {
                      <span class="inline-flex items-center px-1 py-0.5 rounded text-[10px] font-medium bg-ctp-yellow/15 text-ctp-yellow" title="Local">
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                          <path d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
                        </svg>
                      </span>
                    }
                  }
                  <span class="text-text-secondary text-xs">#{{ task.number }}</span>
                  @if (task.reverted_at) {
                    <span class="text-ctp-peach text-xs" [title]="t('tasks.reverted')">↩</span>
                  }
                  <button class="shrink-0 p-0 border-0 bg-transparent cursor-pointer transition-colors hover:text-ctp-yellow"
                          [class.text-ctp-yellow]="task.flagged"
                          [class.text-text-muted]="!task.flagged"
                          [title]="task.flagged ? t('tasks.unflag') : t('tasks.flag')"
                          (click)="onFlagToggle($event, task)">
                    @if (task.flagged) {
                      <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                        <path d="M5 2a1 1 0 00-1 1v18a1 1 0 001.65.76L12 16.27l6.35 5.49A1 1 0 0020 21V3a1 1 0 00-1-1H5z" />
                      </svg>
                    } @else {
                      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                        <path d="M5 3a1 1 0 011-1h12a1 1 0 011 1v18l-7-4.5L5 21V3z" />
                      </svg>
                    }
                  </button>
                  <span class="text-text-primary text-sm font-medium truncate">{{ task.title }}</span>
                </div>
                <!-- Mobile-only: badges below title -->
                <div class="flex items-center gap-2 mt-1 flex-wrap md:hidden">
                  <button class="px-2 py-0.5 rounded-full text-xs font-medium cursor-pointer hover:ring-1 hover:ring-accent {{ stateColor(task.state) }}"
                        (click)="toggleStateMenu($event, task.id)">
                    {{ task.state }}
                  </button>
                  @if (task.urgent) {
                    <span class="text-xs text-ctp-red font-medium">Urgent</span>
                  }
                  <span class="text-text-secondary text-xs">{{ task.kind }}</span>
                </div>
              </div>
              <!-- Desktop-only: inline meta fields -->
              <span class="hidden md:inline-block text-text-secondary text-xs w-20 text-center shrink-0">{{ task.kind }}</span>
              <div class="hidden md:block relative shrink-0">
                <button class="px-2 py-0.5 rounded-full text-xs font-medium cursor-pointer hover:ring-1 hover:ring-accent {{ stateColor(task.state) }}"
                      (click)="toggleStateMenu($event, task.id)">
                  {{ task.state }}
                </button>
                @if (openMenuId() === task.id) {
                  <div class="absolute z-50 mt-1 right-0 bg-surface border border-border rounded-lg shadow-lg py-1 min-w-[120px]">
                    @for (target of getTransitions(task.state); track target) {
                      <button (click)="onTransition($event, task, target)"
                        class="w-full text-left px-3 py-1.5 text-xs hover:bg-surface-hover transition-colors flex items-center gap-2 cursor-pointer">
                        <span class="w-2 h-2 rounded-full {{ stateColor(target) }}"></span>
                        <span class="text-text-primary">{{ target }}</span>
                      </button>
                    } @empty {
                      <span class="px-3 py-1.5 text-xs text-text-muted">{{ t('tasks.noTransitions') }}</span>
                    }
                  </div>
                }
              </div>
              <span class="hidden md:inline-block text-xs w-20 text-center shrink-0">
                @if (task.urgent) {
                  <span class="text-ctp-red font-medium">Urgent</span>
                }
              </span>
              <span class="hidden lg:inline-block text-text-muted text-xs w-32 text-right shrink-0 whitespace-nowrap">
                {{ task.created_at | date:'MMM d, HH:mm' }}
              </span>
              <span class="hidden lg:inline-block text-text-muted text-xs w-20 text-right shrink-0 truncate font-mono">
                {{ task.assigned_agent_id ? (task.assigned_agent_id | slice:0:8) : '—' }}
              </span>
              <!-- Expand chevron -->
              <button class="shrink-0 p-1 text-text-muted hover:text-text-secondary rounded transition-colors cursor-pointer"
                      (click)="toggleExpand($event, task.id)">
                <svg class="w-4 h-4 transition-transform duration-150" [class.rotate-180]="expandedIds().has(task.id) || selectedId() === task.id"
                     fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M19 9l-7 7-7-7" />
                </svg>
              </button>
            </div>

            <!-- State transition menu (mobile) -->
            @if (openMenuId() === task.id) {
              <div class="md:hidden mx-3 mb-2 bg-surface border border-border rounded-lg shadow-lg py-1">
                @for (target of getTransitions(task.state); track target) {
                  <button (click)="onTransition($event, task, target)"
                    class="w-full text-left px-3 py-2 text-xs hover:bg-surface-hover transition-colors flex items-center gap-2 cursor-pointer">
                    <span class="w-2 h-2 rounded-full {{ stateColor(target) }}"></span>
                    <span class="text-text-primary">{{ target }}</span>
                  </button>
                } @empty {
                  <span class="px-3 py-2 text-xs text-text-muted">{{ t('tasks.noTransitions') }}</span>
                }
              </div>
            }

            <!-- Expanded details -->
            @if (selectedId() === task.id) {
              <div class="border-t border-border">
                <app-task-detail
                  [embedded]="true"
                  [task]="task"
                  [updates]="detailUpdates()"
                  [comments]="detailComments()"
                  [dependencies]="detailDependencies()"
                  [verifications]="detailVerifications()"
                  [changedFiles]="detailChangedFiles()"
                  [gitStatus]="detailGitStatus()"
                  [playbooks]="detailPlaybooks()"
                  [pushing]="detailPushing()"
                  [reverting]="detailReverting()"
                  [resolving]="detailResolving()"
                  [updatesLoading]="detailLoading()"
                  [commentsLoading]="detailLoading()"
                  [kinds]="detailKinds()"
                  [parentTask]="detailParentTask()"
                  [subtasks]="detailSubtasks()"
                  [planName]="detailPlanName()"
                  (closed)="onDetailClosed(task)"
                  (transitionClick)="detailTransition.emit($event)"
                  (claimClick)="detailClaim.emit()"
                  (releaseClick)="detailRelease.emit()"
                  (pushClick)="detailPush.emit($event)"
                  (resolveClick)="detailResolve.emit()"
                  (revertClick)="detailRevert.emit()"
                  (postUpdate)="detailPostUpdate.emit($event)"
                  (postComment)="detailPostComment.emit($event)"
                  (addDepClick)="detailAddDep.emit($event)"
                  (removeDep)="detailRemoveDep.emit($event)"
                  (deleteClick)="detailDelete.emit()"
                  (playbookChange)="detailPlaybookChange.emit($event)"
                  (playbookStepChange)="detailPlaybookStepChange.emit($event)"
                  (inlineUpdate)="detailInlineUpdate.emit($event)"
                  (navigateToTask)="detailNavigateToTask.emit($event)"
                  (navigateToPlan)="detailNavigateToPlan.emit($event)" />
              </div>
            } @else if (expandedIds().has(task.id)) {
              <div class="px-3 pb-3 ml-7 space-y-1.5 border-t border-border pt-2.5 text-xs">
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
                    <p class="text-text-secondary mt-0.5 whitespace-pre-line line-clamp-3">{{ task.context['spec'] }}</p>
                  </div>
                }
                @if (getBranch(task.id); as branch) {
                  <div class="flex justify-between items-center">
                    <span class="text-text-muted">Branch</span>
                    @if (branch.is_pushed) {
                      <span class="inline-flex items-center gap-1 text-ctp-green text-xs">
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path d="M5 13l4 4L19 7" /></svg>
                        Pushed
                      </span>
                    } @else {
                      <span class="inline-flex items-center gap-1 text-ctp-yellow text-xs">
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" /></svg>
                        Local
                      </span>
                    }
                  </div>
                }
              </div>
            }
          </div>
        } @empty {
          <div class="px-4 py-8 text-center text-text-muted bg-surface rounded-lg border border-border">
            @if (loading()) {
              {{ t('common.loading') }}
            } @else {
              {{ t('common.empty') }}
            }
          </div>
        }
      </div>

      <!-- Pagination -->
      @if (!compact() && total() > 0) {
        <div class="flex items-center justify-between mt-3 text-sm text-text-secondary">
          <span>{{ t('tasks.showing') }} {{ offset() + 1 }}–{{ Math.min(offset() + limit(), total()) }} {{ t('tasks.of') }} {{ total() }}</span>
          <div class="flex gap-2">
            <button
              [disabled]="offset() === 0"
              (click)="pageChange.emit(offset() - limit())"
              class="px-3 py-1 rounded bg-surface border border-border hover:bg-surface-hover disabled:opacity-30 disabled:cursor-not-allowed">
              {{ t('tasks.prev') }}
            </button>
            <button
              [disabled]="!hasMore()"
              (click)="pageChange.emit(offset() + limit())"
              class="px-3 py-1 rounded bg-surface border border-border hover:bg-surface-hover disabled:opacity-30 disabled:cursor-not-allowed">
              {{ t('tasks.next') }}
            </button>
          </div>
        </div>
      }
    </div>
  `,
})
export class TaskListComponent {
  tasks = input.required<SpTask[]>();
  compact = input(false);
  loading = input(false);
  selectedId = input<string | null>(null);
  blockedIds = input<Set<string>>(new Set());
  goalLinkedIds = input<Set<string>>(new Set());
  flaggedIds = input<Set<string>>(new Set());
  branchMap = input<Map<string, BranchInfo>>(new Map());
  searchQuery = input('');
  stateFilter = input('');
  kindFilter = input('');
  hideDone = input(true);
  unlinked = input(false);
  total = input(0);
  offset = input(0);
  limit = input(50);
  hasMore = input(false);

  // Detail data inputs (for selected task)
  detailUpdates = input<SpTaskUpdate[]>([]);
  detailComments = input<SpTaskComment[]>([]);
  detailDependencies = input<SpTaskDependencies>({ depends_on: [], blocks: [] });
  detailVerifications = input<SpVerification[]>([]);
  detailChangedFiles = input<ChangedFileSummary[]>([]);
  detailGitStatus = input<TaskBranchStatus | null>(null);
  detailPlaybooks = input<SpPlaybook[]>([]);
  detailPushing = input(false);
  detailReverting = input(false);
  detailResolving = input(false);
  detailLoading = input(false);
  detailKinds = input<string[]>([]);
  detailParentTask = input<SpTask | null>(null);
  detailSubtasks = input<SpTask[]>([]);
  detailPlanName = input<string | null>(null);

  taskSelect = output<SpTask>();
  stateChange = output<{ task: SpTask; target: string }>();
  searchChange = output<string>();
  stateFilterChange = output<string>();
  kindFilterChange = output<string>();
  hideDoneChange = output<boolean>();
  unlinkedChange = output<boolean>();
  pageChange = output<number>();
  bulkTransition = output<{ taskIds: string[]; state: string }>();
  bulkDelete = output<{ taskIds: string[] }>();
  flagToggle = output<{ task: SpTask; flagged: boolean }>();

  // Detail event outputs (bubbled from TaskDetailComponent)
  detailTransition = output<string>();
  detailClaim = output<void>();
  detailRelease = output<void>();
  detailPush = output<string>();
  detailResolve = output<void>();
  detailRevert = output<void>();
  detailPostUpdate = output<{ kind: string; content: string }>();
  detailPostComment = output<string>();
  detailAddDep = output<string>();
  detailRemoveDep = output<string>();
  detailDelete = output<void>();
  detailPlaybookChange = output<string | null>();
  detailPlaybookStepChange = output<number>();
  detailInlineUpdate = output<UpdateTaskRequest>();
  detailNavigateToTask = output<string>();
  detailNavigateToPlan = output<string>();

  readonly Math = Math;
  states = input<string[]>(['backlog', 'ready', 'working', 'implement', 'review', 'merge', 'human_review', 'done', 'cancelled']);
  kinds = input<string[]>(['feature', 'bug', 'refactor', 'docs', 'test', 'research', 'chore', 'spike']);
  readonly bulkTransitionTargets = ['backlog', 'ready', 'done', 'cancelled'];
  openMenuId = signal<string | null>(null);
  sortField = signal<SortField>('created_at');
  sortDir = signal<SortDir>('desc');
  selectedIds = signal<Set<string>>(new Set());
  bulkTransitionOpen = signal(false);
  expandedIds = signal<Set<string>>(new Set());
  hierarchyView = signal(false);
  collapsedParents = signal<Set<string>>(new Set());
  readonly sortColumns: { field: SortField; label: string }[] = [
    { field: 'title', label: 'Title' },
    { field: 'kind', label: 'Kind' },
    { field: 'state', label: 'State' },
    { field: 'urgent', label: 'Urgent' },
    { field: 'created_at', label: 'Created' },
    { field: 'assigned_agent_id', label: 'Agent' },
  ];

  constructor() {
    // Clear selection when filters or page change (but NOT on polling refresh)
    effect(() => {
      this.searchQuery();
      this.stateFilter();
      this.kindFilter();
      this.hideDone();
      this.offset();
      this.selectedIds.set(new Set());
    });
  }

  sortedTasks = computed(() => {
    const list = [...this.tasks()];
    const field = this.sortField();
    const dir = this.sortDir();
    list.sort((a, b) => {
      const av = a[field];
      const bv = b[field];
      if (av == null && bv == null) return 0;
      if (av == null) return 1;
      if (bv == null) return -1;
      const cmp = typeof av === 'number' ? av - (bv as unknown as number)
        : typeof av === 'boolean' ? Number(bv) - Number(av)
        : String(av).localeCompare(String(bv));
      return dir === 'asc' ? cmp : -cmp;
    });

    if (!this.hierarchyView()) return list;

    // Build hierarchy: group children under parents
    const collapsed = this.collapsedParents();
    const childMap = new Map<string, SpTask[]>();
    const roots: SpTask[] = [];
    for (const task of list) {
      if (task.parent_id && list.some(t => t.id === task.parent_id)) {
        const siblings = childMap.get(task.parent_id) ?? [];
        siblings.push(task);
        childMap.set(task.parent_id, siblings);
      } else {
        roots.push(task);
      }
    }
    // Flatten: parent followed by children
    const result: SpTask[] = [];
    for (const root of roots) {
      result.push(root);
      if (!collapsed.has(root.id)) {
        const children = childMap.get(root.id);
        if (children) result.push(...children);
      }
    }
    return result;
  });

  allSelected = computed(() => {
    const tasks = this.tasks();
    const sel = this.selectedIds();
    return tasks.length > 0 && tasks.every(t => sel.has(t.id));
  });

  someSelected = computed(() => {
    const tasks = this.tasks();
    const sel = this.selectedIds();
    const count = tasks.filter(t => sel.has(t.id)).length;
    return count > 0 && count < tasks.length;
  });

  @HostListener('document:click')
  closeMenu(): void {
    this.openMenuId.set(null);
    this.bulkTransitionOpen.set(false);
  }

  // Selection

  toggleSelect(taskId: string): void {
    const next = new Set(this.selectedIds());
    if (next.has(taskId)) {
      next.delete(taskId);
    } else {
      next.add(taskId);
    }
    this.selectedIds.set(next);
  }

  toggleSelectAll(): void {
    if (this.allSelected()) {
      this.selectedIds.set(new Set());
    } else {
      this.selectedIds.set(new Set(this.tasks().map(t => t.id)));
    }
  }

  clearSelection(): void {
    this.selectedIds.set(new Set());
  }

  // Bulk actions

  toggleBulkTransitionMenu(event: Event): void {
    event.stopPropagation();
    this.bulkTransitionOpen.set(!this.bulkTransitionOpen());
  }

  onBulkTransition(event: Event, target: string): void {
    event.stopPropagation();
    this.bulkTransitionOpen.set(false);
    const taskIds = [...this.selectedIds()];
    if (taskIds.length > 0) {
      this.bulkTransition.emit({ taskIds, state: target });
      this.clearSelection();
    }
  }

  onBulkDelete(event: Event): void {
    event.stopPropagation();
    const taskIds = [...this.selectedIds()];
    if (taskIds.length > 0) {
      this.bulkDelete.emit({ taskIds });
      this.clearSelection();
    }
  }

  // Flag toggle

  onFlagToggle(event: Event, task: SpTask): void {
    event.stopPropagation();
    this.flagToggle.emit({ task, flagged: !task.flagged });
  }

  // Accordion expand/collapse

  toggleExpand(event: Event, taskId: string): void {
    event.stopPropagation();
    const next = new Set(this.expandedIds());
    if (next.has(taskId)) {
      next.delete(taskId);
    } else {
      next.add(taskId);
    }
    this.expandedIds.set(next);
  }

  onDetailClosed(task: SpTask): void {
    const next = new Set(this.expandedIds());
    next.delete(task.id);
    this.expandedIds.set(next);
    this.taskSelect.emit(task);
  }

  onSortFieldChange(field: string): void {
    this.sortField.set(field as SortField);
  }

  // Sort & state menu

  toggleSort(field: SortField): void {
    if (this.sortField() === field) {
      this.sortDir.set(this.sortDir() === 'asc' ? 'desc' : 'asc');
    } else {
      this.sortField.set(field);
      this.sortDir.set('asc');
    }
  }

  sortIndicator(field: SortField): string {
    if (this.sortField() !== field) return '';
    return this.sortDir() === 'asc' ? '▲' : '▼';
  }

  toggleStateMenu(event: Event, taskId: string): void {
    event.stopPropagation();
    this.openMenuId.set(this.openMenuId() === taskId ? null : taskId);
  }

  onTransition(event: Event, task: SpTask, target: string): void {
    event.stopPropagation();
    this.openMenuId.set(null);
    this.stateChange.emit({ task, target });
  }

  protected readonly stateColor = taskStateColor;
  protected readonly getTransitions = taskTransitions;

  getBranch(taskId: string): BranchInfo | undefined {
    // Task ID prefix is the first 12 chars
    const prefix = taskId.substring(0, 12);
    return this.branchMap().get(prefix);
  }

  // Hierarchy helpers

  hasChildren(taskId: string): boolean {
    return this.tasks().some(t => t.parent_id === taskId);
  }

  isChildVisible(task: SpTask): boolean {
    return !!task.parent_id && this.tasks().some(t => t.id === task.parent_id);
  }

  toggleParentCollapse(event: Event, taskId: string): void {
    event.stopPropagation();
    const next = new Set(this.collapsedParents());
    if (next.has(taskId)) {
      next.delete(taskId);
    } else {
      next.add(taskId);
    }
    this.collapsedParents.set(next);
  }
}
