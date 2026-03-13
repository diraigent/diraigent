import { Component, inject, signal, effect, HostListener, ViewChild, ElementRef, DestroyRef } from '@angular/core';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { Subscription, timer, switchMap, forkJoin, of } from 'rxjs';
import { catchError } from 'rxjs/operators';
import { ProjectContext } from '../../core/services/project-context.service';
import { ChatService } from '../../core/services/chat.service';
import {
  TasksApiService,
  SpTask,
  SpTaskUpdate,
  SpTaskComment,
  SpTaskDependencies,
  ChangedFileSummary,
  TaskListFilters,
  UpdateTaskRequest,
} from '../../core/services/tasks-api.service';
import { GitApiService, TaskBranchStatus, BranchInfo, MainPushStatus } from '../../core/services/git-api.service';
import { VerificationsApiService, SpVerification } from '../../core/services/verifications-api.service';
import { PlaybooksApiService, SpPlaybook } from '../../core/services/playbooks-api.service';
import { DiraigentApiService } from '../../core/services/diraigent-api.service';
import { TaskListComponent } from './pages/task-list/task-list';
import { taskTransitions, deriveStatesFromPlaybooks, DEFAULT_TASK_KINDS } from '../../shared/ui-constants';
import { TaskFormComponent } from './components/task-form/task-form';

@Component({
  selector: 'app-tasks',
  standalone: true,
  imports: [TranslocoModule, TaskListComponent, TaskFormComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.tasks') }}</h1>
        <div class="flex items-center gap-3">
          @if (mainStatus(); as ms) {
            @if (ms.ahead > 0 && ms.behind > 0) {
              <!-- Diverged: local and remote have different commits — rebase + push needed -->
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
              <!-- Normal push: local is ahead, no conflict -->
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
            {{ t('tasks.create') }}
          </button>
        </div>
      </div>

      @if (error()) {
        <p class="text-error mb-4">{{ t('common.error') }}</p>
      }

      <app-task-list
        [tasks]="tasks()"
        [loading]="loading()"
        [selectedId]="selectedTask()?.id ?? null"
        [blockedIds]="blockedTaskIds()"
        [goalLinkedIds]="goalLinkedIds()"
        [flaggedIds]="flaggedIds()"
        [branchMap]="branchMap()"
        [states]="filterStates()"
        [kinds]="filterKinds()"
        [searchQuery]="searchQuery()"
        [stateFilter]="stateFilter()"
        [kindFilter]="kindFilter()"
        [hideDone]="hideDone()"
        [unlinked]="unlinked()"
        [total]="total()"
        [offset]="offset()"
        [limit]="limit()"
        [hasMore]="hasMore()"
        [detailUpdates]="taskUpdates()"
        [detailComments]="taskComments()"
        [detailDependencies]="taskDependencies()"
        [detailVerifications]="taskVerifications()"
        [detailChangedFiles]="taskChangedFiles()"
        [detailGitStatus]="taskGitStatus()"
        [detailPlaybooks]="playbooks()"
        [detailPushing]="pushing()"
        [detailReverting]="reverting()"
        [detailResolving]="resolving()"
        [detailLoading]="detailLoading()"
        [detailKinds]="filterKinds()"
        (taskSelect)="selectTask($event)"
        (stateChange)="onTransition($event.task, $event.target)"
        (searchChange)="onSearchChange($event)"
        (stateFilterChange)="onStateFilter($event)"
        (kindFilterChange)="onKindFilter($event)"
        (hideDoneChange)="onHideDoneChange($event)"
        (unlinkedChange)="onUnlinkedChange($event)"
        (pageChange)="onPageChange($event)"
        (bulkTransition)="onBulkTransition($event.taskIds, $event.state)"
        (bulkDelete)="onBulkDelete($event.taskIds)"
        (flagToggle)="onFlagToggle($event.task, $event.flagged)"
        (detailTransition)="onTransition(selectedTask()!, $event)"
        (detailClaim)="onClaim(selectedTask()!)"
        (detailRelease)="onRelease(selectedTask()!)"
        (detailPush)="onPush($event)"
        (detailResolve)="onResolve(selectedTask()!)"
        (detailRevert)="onRevert(selectedTask()!)"
        (detailPostUpdate)="onPostUpdate(selectedTask()!, $event)"
        (detailPostComment)="onPostComment(selectedTask()!, $event)"
        (detailAddDep)="onAddDep(selectedTask()!, $event)"
        (detailRemoveDep)="onRemoveDep(selectedTask()!, $event)"
        (detailDelete)="onDelete(selectedTask()!)"
        (detailPlaybookChange)="onPlaybookChange(selectedTask()!, $event)"
        (detailPlaybookStepChange)="onPlaybookStepChange(selectedTask()!, $event)"
        (detailInlineUpdate)="onInlineUpdate(selectedTask()!, $event)" />

      <!-- Create/Edit form -->
      <app-task-form
        [show]="showForm()"
        [editing]="editingTask()"
        (submitCreate)="onCreateTask($event)"
        (submitUpdate)="onUpdateTask($event)"
        (closed)="closeForm()" />

      <!-- Keyboard shortcuts hint -->
      <div class="fixed bottom-0 left-0 lg:left-64 right-0 bg-surface border-t border-border px-4 py-1.5
                  hidden sm:flex items-center gap-4 text-xs text-text-muted z-40 overflow-x-auto">
        <span><kbd class="px-1.5 py-0.5 rounded bg-surface-hover text-text-secondary font-mono">n</kbd> {{ t('shortcuts.newTask') }}</span>
        <span><kbd class="px-1.5 py-0.5 rounded bg-surface-hover text-text-secondary font-mono">j</kbd><kbd class="px-1.5 py-0.5 rounded bg-surface-hover text-text-secondary font-mono ml-0.5">k</kbd> {{ t('shortcuts.navigate') }}</span>
        <span><kbd class="px-1.5 py-0.5 rounded bg-surface-hover text-text-secondary font-mono">Enter</kbd> {{ t('shortcuts.open') }}</span>
        <span><kbd class="px-1.5 py-0.5 rounded bg-surface-hover text-text-secondary font-mono">s</kbd> {{ t('shortcuts.changeState') }}</span>
        <span><kbd class="px-1.5 py-0.5 rounded bg-surface-hover text-text-secondary font-mono">Esc</kbd> {{ t('shortcuts.close') }}</span>
      </div>
    </div>
  `,
})
export class TasksPage {
  private api = inject(TasksApiService);
  private git = inject(GitApiService);
  private verificationsApi = inject(VerificationsApiService);
  private playbooksApi = inject(PlaybooksApiService);
  private projectApi = inject(DiraigentApiService);
  private ctx = inject(ProjectContext);
  private chat = inject(ChatService);
  private el = inject(ElementRef);
  private destroyRef = inject(DestroyRef);
  private pollSub?: Subscription;
  private gitPollSub?: Subscription;
  private detailSub?: Subscription;
  private detailPollSub?: Subscription;
  private gitSub?: Subscription;

  @ViewChild(TaskListComponent) taskList?: TaskListComponent;

  tasks = signal<SpTask[]>([]);
  loading = signal(true);
  error = signal(false);
  total = signal(0);
  offset = signal(0);
  limit = signal(50);
  hasMore = signal(false);

  selectedTask = signal<SpTask | null>(null);
  taskUpdates = signal<SpTaskUpdate[]>([]);
  taskComments = signal<SpTaskComment[]>([]);
  taskDependencies = signal<SpTaskDependencies>({ depends_on: [], blocks: [] });
  taskVerifications = signal<SpVerification[]>([]);
  taskChangedFiles = signal<ChangedFileSummary[]>([]);
  taskGitStatus = signal<TaskBranchStatus | null>(null);
  pushing = signal(false);
  reverting = signal(false);
  resolving = signal(false);
  pushingMain = signal(false);
  resolvingMain = signal(false);
  releasing = signal(false);
  releaseMessage = signal('');
  mainStatus = signal<MainPushStatus | null>(null);
  blockedTaskIds = signal<Set<string>>(new Set());
  goalLinkedIds = signal<Set<string>>(new Set());
  flaggedIds = signal<Set<string>>(new Set());
  branchMap = signal<Map<string, BranchInfo>>(new Map());
  detailLoading = signal(false);
  playbooks = signal<SpPlaybook[]>([]);

  /** Dynamic states derived from playbooks. */
  filterStates = signal<string[]>(['backlog', 'ready', 'working', 'implement', 'review', 'merge', 'human_review', 'done', 'cancelled']);
  /** Dynamic kinds loaded from the project's package. */
  filterKinds = signal<string[]>(DEFAULT_TASK_KINDS);

  searchQuery = signal('');
  stateFilter = signal('');
  kindFilter = signal('');
  hideDone = signal(true);
  unlinked = signal(false);

  showForm = signal(false);
  editingTask = signal<SpTask | null>(null);

  private hideDoneKey(): string {
    return `diraigent_hide_done_${this.ctx.projectId()}`;
  }

  constructor() {
    // Restart polling and seed hideDone from localStorage whenever the project changes.
    effect(() => {
      const pid = this.ctx.projectId();
      const stored = localStorage.getItem(`diraigent_hide_done_${pid}`);
      // Default true until project metadata tells us otherwise.
      this.hideDone.set(stored !== null ? stored === 'true' : true);
      this.startPolling();
      // Load playbooks for the detail panel dropdown + derive filter states
      this.playbooksApi.list().pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
        next: (pbs) => {
          this.playbooks.set(pbs);
          this.filterStates.set(deriveStatesFromPlaybooks(pbs));
        },
      });
      // Load task kinds from the project's package
      if (pid) {
        this.projectApi.getProject(pid).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
          next: (proj) => {
            if (proj.package?.id) {
              this.projectApi.getPackage(proj.package.id).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
                next: (pkg) => {
                  if (pkg.allowed_task_kinds.length > 0) {
                    this.filterKinds.set(pkg.allowed_task_kinds);
                  }
                },
              });
            }
          },
        });
      }
    });

    // Refine the hideDone default once project data arrives (only when no stored preference).
    // Team-review projects (metadata.team_review = true) show done tasks by default.
    effect(() => {
      const pid = this.ctx.projectId();
      const project = this.ctx.project();
      if (project && localStorage.getItem(`diraigent_hide_done_${pid}`) === null) {
        const isTeamReview = project.metadata?.['team_review'] === true;
        this.hideDone.set(!isTeamReview);
      }
    });
  }

  @HostListener('document:keydown', ['$event'])
  handleKeyboard(event: KeyboardEvent): void {
    // Skip when typing in inputs, textareas, selects, or contenteditable
    const tag = (event.target as HTMLElement)?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
    if ((event.target as HTMLElement)?.isContentEditable) return;

    // Skip when form modal is open
    if (this.showForm()) return;

    switch (event.key) {
      case 'n':
        event.preventDefault();
        this.openCreate();
        break;

      case 'j':
        event.preventDefault();
        this.navigateList(1);
        break;

      case 'k':
        event.preventDefault();
        this.navigateList(-1);
        break;

      case 'Enter': {
        const sel = this.selectedTask();
        if (!sel) {
          // If nothing selected, select the first task
          this.navigateList(1);
        }
        // Detail panel is already shown when a task is selected
        break;
      }

      case 's': {
        event.preventDefault();
        const task = this.selectedTask();
        if (task) {
          this.cycleState(task);
        }
        break;
      }

      case 'Escape':
        event.preventDefault();
        if (this.selectedTask()) {
          this.selectTask(null);
        }
        break;
    }
  }

  private navigateList(direction: number): void {
    const sorted = this.taskList?.sortedTasks() ?? this.tasks();
    if (sorted.length === 0) return;

    const currentId = this.selectedTask()?.id;
    const currentIndex = currentId ? sorted.findIndex(t => t.id === currentId) : -1;

    let nextIndex: number;
    if (currentIndex === -1) {
      // Nothing selected: go to first (j) or last (k)
      nextIndex = direction > 0 ? 0 : sorted.length - 1;
    } else {
      nextIndex = currentIndex + direction;
      if (nextIndex < 0) nextIndex = 0;
      if (nextIndex >= sorted.length) nextIndex = sorted.length - 1;
    }

    const nextTask = sorted[nextIndex];
    if (nextTask && nextTask.id !== currentId) {
      this.selectedTask.set(nextTask);
      this.taskUpdates.set([]);
      this.taskComments.set([]);
      this.taskDependencies.set({ depends_on: [], blocks: [] });
      this.taskVerifications.set([]);
      this.taskChangedFiles.set([]);
      this.taskGitStatus.set(null);
      this.loadTaskDetail(nextTask.id);
      this.loadGitStatus(nextTask.id);
      this.startDetailPolling(nextTask.id);
      // Scroll selected row into view
      requestAnimationFrame(() => {
        const row = this.el.nativeElement.querySelector(`[data-task-id="${nextTask.id}"]`);
        row?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      });
    }
  }

  private cycleState(task: SpTask): void {
    const transitions = taskTransitions(task.state);
    if (transitions.length === 0) return;
    // Cycle to the first valid transition (most common action)
    this.onTransition(task, transitions[0]);
  }

  private startPolling(): void {
    this.pollSub?.unsubscribe();
    this.gitPollSub?.unsubscribe();
    this.stopDetailPolling();
    this.selectedTask.set(null);
    this.loading.set(true);

    // Fast poll: task list and blocked/flagged IDs (4 requests, completes quickly)
    this.pollSub = timer(0, 10_000)
      .pipe(
        switchMap(() => forkJoin({
          tasks: this.api.list(this.buildFilters()),
          blocked: this.api.listBlockedIds().pipe(catchError(() => of([] as string[]))),
          goalLinked: this.api.listGoalLinkedIds().pipe(catchError(() => of([] as string[]))),
          flagged: this.api.listFlaggedIds().pipe(catchError(() => of([] as string[]))),
        })),
        takeUntilDestroyed(this.destroyRef),
      )
      .subscribe({
        next: ({ tasks: res, blocked, goalLinked, flagged }) => {
          this.tasks.set(res.data);
          this.total.set(res.total);
          this.hasMore.set(res.has_more);
          this.blockedTaskIds.set(new Set(blocked));
          this.goalLinkedIds.set(new Set(goalLinked));
          this.flaggedIds.set(new Set(flagged));
          this.loading.set(false);
          this.error.set(false);
          // Update selected task header data if still in list
          const sel = this.selectedTask();
          if (sel) {
            const updated = res.data.find(t => t.id === sel.id);
            if (updated) {
              this.selectedTask.set(updated);
            }
          }
        },
        error: () => {
          this.loading.set(false);
          this.error.set(true);
        },
      });

    // Separate slow poll: git data (WS-based, can take up to 10s)
    // Runs independently so it doesn't block the fast task list poll
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

  selectTask(task: SpTask | null): void {
    if (task && this.selectedTask()?.id === task.id) {
      this.selectedTask.set(null);
      this.stopDetailPolling();
      return;
    }
    this.selectedTask.set(task);
    this.taskUpdates.set([]);
    this.taskComments.set([]);
    this.taskDependencies.set({ depends_on: [], blocks: [] });
    this.taskVerifications.set([]);
    this.taskChangedFiles.set([]);
    this.taskGitStatus.set(null);
    if (task) {
      this.loadTaskDetail(task.id);
      this.loadGitStatus(task.id);
      this.startDetailPolling(task.id);
    } else {
      this.stopDetailPolling();
    }
  }

  // -- Filters & Pagination --

  onSearchChange(query: string): void {
    this.searchQuery.set(query);
    this.offset.set(0);
    this.reload();
  }

  onStateFilter(state: string): void {
    this.stateFilter.set(state);
    this.offset.set(0);
    this.reload();
  }

  onKindFilter(kind: string): void {
    this.kindFilter.set(kind);
    this.offset.set(0);
    this.reload();
  }

  onHideDoneChange(hide: boolean): void {
    this.hideDone.set(hide);
    localStorage.setItem(this.hideDoneKey(), String(hide));
    this.offset.set(0);
    this.reload();
  }

  onUnlinkedChange(unlinked: boolean): void {
    this.unlinked.set(unlinked);
    this.offset.set(0);
    this.reload();
  }

  onPageChange(newOffset: number): void {
    this.offset.set(Math.max(0, newOffset));
    this.reload();
  }

  // -- Task Actions --

  onTransition(task: SpTask, targetState: string): void {
    this.api.transition(task.id, targetState).subscribe({
      next: () => this.reload(),
    });
  }

  onClaim(_task: SpTask): void {
    // For web UI, we don't have an agent_id — this action is for humans via UI.
    // Claim requires an agent_id; in web context this would need selection.
    // For now, skip or show a delegate action.
  }

  onRelease(task: SpTask): void {
    this.api.release(task.id).subscribe({
      next: () => this.reload(),
    });
  }

  onPush(branch: string): void {
    this.pushing.set(true);
    this.git.pushBranch(branch).subscribe({
      next: (res) => {
        this.pushing.set(false);
        if (res.success) {
          // Refresh git status
          const sel = this.selectedTask();
          if (sel) this.loadGitStatus(sel.id);
        }
      },
      error: () => this.pushing.set(false),
    });
  }

  onRevert(task: SpTask): void {
    this.reverting.set(true);
    this.git.revertTask(task.id).subscribe({
      next: () => {
        this.reverting.set(false);
        this.reload();
      },
      error: () => this.reverting.set(false),
    });
  }

  onResolve(task: SpTask): void {
    this.resolving.set(true);
    this.git.resolveTaskBranch(task.id).subscribe({
      next: () => {
        this.resolving.set(false);
        // Refresh git status to reflect the resolved state
        this.loadGitStatus(task.id);
      },
      error: (err: unknown) => {
        this.resolving.set(false);
        // Automatic rebase failed — ask the AI assistant to help
        const errorMsg = err instanceof Error ? err.message : String(err);
        const branch = this.taskGitStatus()?.branch ?? `agent/task-${task.id.substring(0, 12)}`;
        const prompt =
          `There is a merge conflict on branch ${branch}. ` +
          `The automatic rebase failed with: ${errorMsg}. ` +
          `Can you resolve this merge conflict by rebasing the branch onto the default branch?`;
        this.chat.openWithMessage(prompt);
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
        this.releaseMessage.set(res.message);
        // Refresh main status after release
        this.git.mainStatus().pipe(catchError(() => of(null as MainPushStatus | null))).subscribe({
          next: (ms) => this.mainStatus.set(ms),
        });
      },
      error: (err: unknown) => {
        this.releasing.set(false);
        const errorMsg = err instanceof Error ? err.message : String(err);
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
      error: (err: unknown) => {
        this.resolvingMain.set(false);
        // Automatic resolution failed — ask the AI assistant to help
        const detail = ms ? ` (local is ${ms.ahead} commit(s) ahead, ${ms.behind} behind remote)` : '';
        const errorMsg = err instanceof Error ? err.message : String(err);
        const prompt =
          `There is a merge conflict on the main branch${detail}. ` +
          `The automatic rebase+push failed with: ${errorMsg}. ` +
          `Can you resolve this merge conflict and push main to the remote?`;
        this.chat.openWithMessage(prompt);
      },
    });
  }

  onPostUpdate(task: SpTask, event: { kind: string; content: string }): void {
    this.api.createUpdate(task.id, { kind: event.kind, content: event.content }).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onPostComment(task: SpTask, content: string): void {
    this.api.createComment(task.id, { content }).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onAddDep(task: SpTask, depId: string): void {
    this.api.addDependency(task.id, depId).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onRemoveDep(task: SpTask, depId: string): void {
    this.api.removeDependency(task.id, depId).subscribe({
      next: () => this.loadTaskDetail(task.id),
    });
  }

  onPlaybookChange(task: SpTask, playbookId: string | null): void {
    this.api.update(task.id, { playbook_id: playbookId, playbook_step: playbookId ? 0 : null }).subscribe({
      next: () => this.reload(),
    });
  }

  onFlagToggle(task: SpTask, flagged: boolean): void {
    // Optimistic update — toggle immediately in the local task list
    this.tasks.set(this.tasks().map(t => t.id === task.id ? { ...t, flagged } : t));

    // Also update the selected task if it's the one being flagged
    const sel = this.selectedTask();
    if (sel && sel.id === task.id) {
      this.selectedTask.set({ ...sel, flagged });
    }

    // Update flaggedIds set
    const nextFlagged = new Set(this.flaggedIds());
    if (flagged) { nextFlagged.add(task.id); } else { nextFlagged.delete(task.id); }
    this.flaggedIds.set(nextFlagged);

    // Persist to server — revert on failure
    this.api.update(task.id, { flagged }).subscribe({
      error: () => this.reload(),
    });
  }

  onInlineUpdate(task: SpTask, data: UpdateTaskRequest): void {
    this.api.update(task.id, data).subscribe({
      next: () => this.reload(),
    });
  }

  onPlaybookStepChange(task: SpTask, step: number): void {
    this.api.update(task.id, { playbook_step: step }).subscribe({
      next: () => this.reload(),
    });
  }

  onDelete(task: SpTask): void {
    this.api.delete(task.id).subscribe({
      next: () => {
        this.selectedTask.set(null);
        this.reload();
      },
    });
  }

  // -- Bulk Actions --

  onBulkTransition(taskIds: string[], state: string): void {
    this.api.bulkTransition(taskIds, state).subscribe({
      next: () => this.reload(),
      error: () => this.reload(),
    });
  }

  onBulkDelete(taskIds: string[]): void {
    if (!confirm(`Delete ${taskIds.length} task(s)? This cannot be undone.`)) return;
    this.api.bulkDelete(taskIds).subscribe({
      next: () => {
        // Clear detail panel if selected task was deleted
        const sel = this.selectedTask();
        if (sel && taskIds.includes(sel.id)) {
          this.selectedTask.set(null);
        }
        this.reload();
      },
      error: () => this.reload(),
    });
  }

  // -- Form --

  openCreate(): void {
    this.editingTask.set(null);
    this.showForm.set(true);
  }

  openEdit(task: SpTask): void {
    this.editingTask.set(task);
    this.showForm.set(true);
  }

  closeForm(): void {
    this.showForm.set(false);
    this.editingTask.set(null);
  }

  onCreateTask(req: import('../../core/services/tasks-api.service').CreateTaskRequest): void {
    this.api.create(req).subscribe({
      next: () => {
        this.closeForm();
        this.reload();
      },
    });
  }

  onUpdateTask(event: { id: string; data: import('../../core/services/tasks-api.service').UpdateTaskRequest }): void {
    this.api.update(event.id, event.data).subscribe({
      next: () => {
        this.closeForm();
        this.reload();
      },
    });
  }

  // -- Private --

  private buildFilters(): TaskListFilters {
    let hideDoneBefore: string | undefined;
    const sf = this.stateFilter();
    // When "Hiding done" is active, exclude done/cancelled tasks older than
    // done_retention_days (from project metadata, default 1 day) — unless
    // the user explicitly picked 'done' or 'cancelled' in the state dropdown.
    if (this.hideDone() && sf !== 'done' && sf !== 'cancelled') {
      const retentionDays = (this.ctx.project()?.metadata?.['done_retention_days'] as number) ?? 1;
      hideDoneBefore = new Date(Date.now() - retentionDays * 24 * 60 * 60 * 1000).toISOString();
    }
    return {
      state: sf || undefined,
      kind: this.kindFilter() || undefined,
      search: this.searchQuery() || undefined,
      limit: this.limit(),
      offset: this.offset(),
      hide_done_before: hideDoneBefore,
      unlinked: this.unlinked() || undefined,
    };
  }

  private reload(): void {
    forkJoin({
      tasks: this.api.list(this.buildFilters()),
      blocked: this.api.listBlockedIds().pipe(catchError(() => of([] as string[]))),
      goalLinked: this.api.listGoalLinkedIds().pipe(catchError(() => of([] as string[]))),
      flagged: this.api.listFlaggedIds().pipe(catchError(() => of([] as string[]))),
    }).subscribe({
      next: ({ tasks: res, blocked, goalLinked, flagged }) => {
        this.tasks.set(res.data);
        this.total.set(res.total);
        this.hasMore.set(res.has_more);
        this.blockedTaskIds.set(new Set(blocked));
        this.goalLinkedIds.set(new Set(goalLinked));
        this.flaggedIds.set(new Set(flagged));
        this.error.set(false);
        const sel = this.selectedTask();
        if (sel) {
          const updated = res.data.find(t => t.id === sel.id);
          if (updated) this.selectedTask.set(updated);
        }
      },
      error: () => this.error.set(true),
    });
  }

  private startDetailPolling(taskId: string): void {
    this.stopDetailPolling();
    this.detailPollSub = timer(10_000, 10_000)
      .pipe(
        switchMap(() => this.fetchTaskDetail(taskId)),
        takeUntilDestroyed(this.destroyRef),
      )
      .subscribe({
        next: ({ updates, comments, dependencies, changedFiles, verifications }) => {
          // Only update if this task is still selected
          if (this.selectedTask()?.id === taskId) {
            this.taskUpdates.set(updates);
            this.taskComments.set(comments);
            this.taskDependencies.set(dependencies);
            this.taskChangedFiles.set(changedFiles);
            this.taskVerifications.set(verifications.data);
          }
        },
      });
  }

  private stopDetailPolling(): void {
    this.detailPollSub?.unsubscribe();
    this.detailPollSub = undefined;
  }

  private loadTaskDetail(taskId: string): void {
    this.detailLoading.set(true);
    this.detailSub?.unsubscribe();
    this.detailSub = this.fetchTaskDetail(taskId).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
      next: ({ updates, comments, dependencies, changedFiles, verifications }) => {
        this.taskUpdates.set(updates);
        this.taskComments.set(comments);
        this.taskDependencies.set(dependencies);
        this.taskChangedFiles.set(changedFiles);
        this.taskVerifications.set(verifications.data);
        this.detailLoading.set(false);
      },
      error: () => this.detailLoading.set(false),
    });
  }

  private fetchTaskDetail(taskId: string) {
    return forkJoin({
      updates: this.api.listUpdates(taskId).pipe(
        catchError((err) => { console.warn('[tasks] listUpdates failed:', err.status, err.message); return of([] as SpTaskUpdate[]); }),
      ),
      comments: this.api.listComments(taskId).pipe(
        catchError((err) => { console.warn('[tasks] listComments failed:', err.status, err.message); return of([] as SpTaskComment[]); }),
      ),
      dependencies: this.api.listDependencies(taskId).pipe(
        catchError((err) => { console.warn('[tasks] listDependencies failed:', err.status, err.message); return of({ depends_on: [], blocks: [] } as SpTaskDependencies); }),
      ),
      changedFiles: this.api.listChangedFiles(taskId).pipe(
        catchError((err) => { console.warn('[tasks] listChangedFiles failed:', err.status, err.message); return of([] as ChangedFileSummary[]); }),
      ),
      verifications: this.verificationsApi.list({ task_id: taskId, limit: 20 }).pipe(
        catchError((err) => { console.warn('[tasks] listVerifications failed:', err.status, err.message); return of({ data: [] as SpVerification[], total: 0, limit: 20, offset: 0, has_more: false }); }),
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
        if (this.selectedTask()?.id === taskId) {
          this.taskGitStatus.set(status);
        }
      },
    });
  }
}
