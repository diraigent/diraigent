import { Component, HostListener, input, output, signal } from '@angular/core';
import { TranslocoModule } from '@jsverse/transloco';
import { FormsModule } from '@angular/forms';
import { DatePipe, SlicePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { SpTask, SpTaskUpdate, SpTaskComment, SpTaskDependencies, ChangedFileSummary, UpdateTaskRequest } from '../../../../core/services/tasks-api.service';
import { SpPlaybook } from '../../../../core/services/playbooks-api.service';
import { TaskBranchStatus } from '../../../../core/services/git-api.service';
import { SpVerification, VerificationStatus, VerificationKind } from '../../../../core/services/verifications-api.service';
import { taskStateColor, taskTransitions, DEFAULT_TASK_KINDS } from '../../../../shared/ui-constants';
import { TaskUpdatesComponent } from '../../components/task-updates/task-updates';
import { TaskCommentsComponent } from '../../components/task-comments/task-comments';
import { ChangedFilesComponent } from '../../components/changed-files/changed-files';

const VERIFICATION_STATUS_COLORS: Record<VerificationStatus, string> = {
  pass: 'bg-ctp-green/20 text-ctp-green',
  fail: 'bg-ctp-red/20 text-ctp-red',
  pending: 'bg-ctp-yellow/20 text-ctp-yellow',
  skipped: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

const VERIFICATION_KIND_COLORS: Record<VerificationKind, string> = {
  test: 'bg-ctp-blue/20 text-ctp-blue',
  acceptance: 'bg-ctp-teal/20 text-ctp-teal',
  sign_off: 'bg-ctp-mauve/20 text-ctp-mauve',
};


@Component({
  selector: 'app-task-detail',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, SlicePipe, RouterLink, TaskUpdatesComponent, TaskCommentsComponent, ChangedFilesComponent],
  template: `
    <div [class]="embedded() ? 'px-5 pb-5 pt-3 max-h-[70vh] overflow-y-auto' : 'bg-surface rounded-lg border border-border p-5 max-h-[calc(100vh-180px)] overflow-y-auto'" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3">
        @if (editingTitle()) {
          <div class="flex-1 mr-2">
            <input id="inline-title-edit" type="text" [(ngModel)]="editTitle"
              (blur)="saveTitleEdit()"
              (keydown.enter)="saveTitleEdit()"
              (keydown.escape)="cancelTitleEdit(); $event.stopPropagation()"
              class="w-full text-lg font-semibold text-text-primary bg-surface border border-border rounded px-2 py-1
                     focus:outline-none focus:ring-1 focus:ring-accent" />
          </div>
        } @else {
          <h2 (click)="startTitleEdit()" (keydown.enter)="startTitleEdit()"
              tabindex="0" role="button"
              class="text-lg font-semibold text-text-primary break-words cursor-pointer hover:text-accent/80 transition-colors"
              [title]="t('tasks.edit')">
            <span class="text-text-secondary font-normal mr-1">#{{ task().number }}</span>{{ task().title }}
          </h2>
        }
        <div class="flex gap-2 shrink-0">
          <button (click)="closed.emit()" class="text-text-muted hover:text-text-secondary text-sm">✕</button>
        </div>
      </div>

      <!-- Task UUID -->
      <div class="flex items-center gap-1.5 mb-3">
        <span class="text-text-muted text-[10px] uppercase tracking-wider">{{ t('tasks.taskId') }}</span>
        <button (click)="copyId()" class="group flex items-center gap-1 text-[11px] font-mono text-text-secondary hover:text-accent transition-colors cursor-pointer" [title]="t(copied() ? 'common.copied' : 'common.copy')">
          <span>{{ task().id }}</span>
          <svg class="w-3 h-3 opacity-0 group-hover:opacity-100 transition-opacity" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            @if (copied()) {
              <path d="M5 13l4 4L19 7" />
            } @else {
              <path d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
            }
          </svg>
        </button>
      </div>

      <!-- State + actions -->
      <div class="flex flex-wrap items-center gap-2 mb-4">
        <!-- Reverted badge -->
        @if (task().reverted_at) {
          <span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-ctp-peach/20 text-ctp-peach"
                [title]="t('tasks.revertedAt') + ': ' + (task().reverted_at | date:'medium')">
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
              <path d="M3 10h10a5 5 0 015 5v2M3 10l4-4M3 10l4 4" />
            </svg>
            {{ t('tasks.reverted') }}
          </span>
        }
        <!-- State badge with dropdown -->
        <div class="relative">
          <button (click)="toggleStateMenu($event)"
            class="px-2 py-0.5 rounded-full text-xs font-medium cursor-pointer hover:ring-1 hover:ring-accent transition-all {{ stateColor(task().state) }}"
            [title]="t('tasks.changeState')">
            {{ task().state }}
            <span class="ml-0.5 opacity-60">▾</span>
          </button>
          @if (stateMenuOpen()) {
            <div class="absolute z-50 mt-1 left-0 bg-surface border border-border rounded-lg shadow-lg py-1 min-w-[130px]">
              @for (target of transitions(); track target) {
                <button (click)="onStateMenuSelect($event, target)"
                  class="w-full text-left px-3 py-1.5 text-xs hover:bg-surface-hover transition-colors flex items-center gap-2 cursor-pointer">
                  <span class="w-2 h-2 rounded-full {{ stateColor(target) }}"></span>
                  <span class="text-text-primary">{{ target }}</span>
                </button>
              } @empty {
                <span class="px-3 py-1.5 text-xs text-text-muted block">{{ t('tasks.noTransitions') }}</span>
              }
            </div>
          }
        </div>

        <!-- Claim/Release -->
        @if (!task().assigned_agent_id && task().state === 'ready') {
          <button (click)="claimClick.emit()"
            class="px-2 py-0.5 text-xs font-medium bg-accent text-bg rounded-lg hover:opacity-90">
            {{ t('tasks.claim') }}
          </button>
        }
      </div>

      <!-- Meta info -->
      <div class="grid grid-cols-2 gap-x-4 gap-y-2 mb-4 text-sm">
        <div>
          <span class="text-text-muted text-xs">{{ t('tasks.kind') }}</span>
          <select (change)="onKindChange($event)"
            class="w-full bg-surface text-text-primary text-xs rounded px-2 py-1 border border-border mt-0.5">
            @for (k of kinds(); track k) {
              <option [value]="k" [selected]="k === task().kind">{{ k }}</option>
            }
          </select>
        </div>
        <div class="flex items-end">
          <label class="flex items-center gap-1.5 cursor-pointer select-none mt-0.5">
            <input type="checkbox" [checked]="task().urgent" (change)="onUrgentToggle()"
              class="w-3.5 h-3.5 rounded border-border bg-surface text-ctp-red focus:ring-ctp-red focus:ring-1" />
            <span class="text-xs" [class]="task().urgent ? 'text-ctp-red font-medium' : 'text-text-secondary'">{{ t('tasks.urgent') }}</span>
          </label>
        </div>
        <div>
          <span class="text-text-muted text-xs">{{ t('tasks.agent') }}</span>
          <p class="text-text-primary text-xs">{{ task().assigned_agent_id ? (task().assigned_agent_id! | slice:0:8) + '...' : '—' }}</p>
        </div>
        <div>
          <span class="text-text-muted text-xs">{{ t('tasks.created') }}</span>
          <p class="text-text-primary text-xs">{{ task().created_at | date:'medium' }}</p>
        </div>
        <div class="col-span-2">
          <span class="text-text-muted text-xs">{{ t('tasks.playbook') }}</span>
          <select (change)="onPlaybookChange($event)"
            class="w-full bg-surface text-text-primary text-xs rounded px-2 py-1 border border-border mt-0.5">
            <option value="" [selected]="!task().playbook_id">—</option>
            @for (pb of playbooks(); track pb.id) {
              <option [value]="pb.id" [selected]="pb.id === task().playbook_id">{{ pb.title }}</option>
            }
          </select>
          @if (currentPlaybook(); as pb) {
            <div class="flex items-center gap-0.5 mt-1.5 flex-wrap">
              @for (step of pb.steps; track step.step; let i = $index; let last = $last) {
                <button (click)="onStepClick(step.step)"
                  class="px-2 py-0.5 rounded text-[10px] font-medium transition-colors cursor-pointer
                    {{ step.step === task().playbook_step
                      ? 'bg-accent text-bg'
                      : step.step < (task().playbook_step ?? 0)
                        ? 'bg-ctp-green/15 text-ctp-green'
                        : 'bg-surface-hover text-text-muted hover:text-text-secondary' }}"
                  [title]="step.name + (step.model ? ' (' + step.model + ')' : '')">
                  {{ step.name }}
                </button>
                @if (!last) {
                  <span class="text-text-muted text-[10px]">›</span>
                }
              }
            </div>
          }
        </div>
      </div>

      <!-- Updates -->
      <div class="mb-4">
        <app-task-updates
          [updates]="updates()"
          [loading]="updatesLoading()"
          (post)="postUpdate.emit($event)" />
      </div>

      <!-- Comments -->
      <div class="mb-4">
        <app-task-comments
          [comments]="comments()"
          [loading]="commentsLoading()"
          (post)="postComment.emit($event)" />
      </div>

      <hr class="border-border mb-4" />

      <!-- Cost metrics -->
      @if (task().cost_usd > 0) {
        <div class="mb-4">
          <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">Cost</h3>
          <div class="grid grid-cols-3 gap-2">
            <div class="bg-bg rounded-lg px-3 py-2 border border-border text-center">
              <div class="text-sm font-semibold text-ctp-green">{{ '$' + task().cost_usd.toFixed(4) }}</div>
              <div class="text-[10px] text-text-muted mt-0.5">USD</div>
            </div>
            <div class="bg-bg rounded-lg px-3 py-2 border border-border text-center">
              <div class="text-sm font-semibold text-ctp-blue">{{ (task().input_tokens / 1000).toFixed(1) }}k</div>
              <div class="text-[10px] text-text-muted mt-0.5">in tokens</div>
            </div>
            <div class="bg-bg rounded-lg px-3 py-2 border border-border text-center">
              <div class="text-sm font-semibold text-ctp-mauve">{{ (task().output_tokens / 1000).toFixed(1) }}k</div>
              <div class="text-[10px] text-text-muted mt-0.5">out tokens</div>
            </div>
          </div>
        </div>
      }

      <!-- Git branch info -->
      @if (gitStatus()?.exists) {
        <div class="mb-4">
          <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">Branch</h3>
          <div class="flex items-center gap-2 bg-bg rounded-lg px-3 py-2 border border-border">
            <svg class="w-4 h-4 shrink-0 text-text-muted" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
              <path d="M6 3v12m0 0a3 3 0 103 3V9a3 3 0 10-3-3m0 12a3 3 0 103 3m6-3a3 3 0 10-3-3m0 0V9a3 3 0 10-3-3" />
            </svg>
            <code class="text-xs text-text-primary font-mono flex-1 truncate">{{ gitStatus()!.branch }}</code>
            @if (gitStatus()!.is_pushed) {
              <span class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-ctp-green/15 text-ctp-green"
                    title="Pushed to remote">
                <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M5 13l4 4L19 7" />
                </svg>
                pushed
              </span>
            } @else {
              <span class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-ctp-yellow/15 text-ctp-yellow"
                    title="Local only — not pushed to remote">
                <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                local
              </span>
              <button (click)="pushClick.emit(gitStatus()!.branch)"
                class="px-2 py-0.5 text-[10px] font-medium rounded bg-accent text-bg hover:opacity-90 transition-opacity cursor-pointer"
                [disabled]="pushing()">
                @if (pushing()) {
                  pushing...
                } @else {
                  push
                }
              </button>
            }
            @if (gitStatus()!.has_conflict) {
              <span class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-ctp-red/15 text-ctp-red"
                    title="This branch has merge conflicts with the default branch">
                <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M12 9v4m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
                </svg>
                conflict
              </span>
              <button (click)="resolveClick.emit()"
                class="px-2 py-0.5 text-[10px] font-medium rounded bg-ctp-red/15 text-ctp-red hover:bg-ctp-red/25 transition-colors cursor-pointer"
                [disabled]="resolving()"
                title="Rebase this branch onto the default branch to resolve conflicts">
                @if (resolving()) {
                  resolving...
                } @else {
                  resolve
                }
              </button>
            }
          </div>
          @if (gitStatus()!.is_pushed && gitStatus()!.ahead_remote > 0) {
            <p class="text-[10px] text-ctp-yellow mt-1 ml-6">{{ gitStatus()!.ahead_remote }} commit(s) ahead of remote</p>
          }
          @if (gitStatus()!.behind_default > 0 && !gitStatus()!.has_conflict) {
            <p class="text-[10px] text-text-muted mt-1 ml-6">{{ gitStatus()!.behind_default }} commit(s) behind default branch</p>
          }
          @if (gitStatus()!.last_commit) {
            <p class="text-[10px] text-text-muted mt-1 ml-6">
              <span class="font-mono">{{ gitStatus()!.last_commit }}</span>
              @if (gitStatus()!.last_commit_message) {
                — {{ gitStatus()!.last_commit_message }}
              }
            </p>
          }
        </div>
      }

      <!-- Originating Decision -->
      @if (task().decision) {
        <div class="mb-4">
          <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{{ t('tasks.originatingDecision') }}</h3>
          <div class="bg-bg rounded-lg p-3 border border-border flex flex-col gap-1">
            <div class="flex items-center gap-2">
              <span class="px-1.5 py-0.5 rounded-full text-[10px] font-medium bg-ctp-mauve/20 text-ctp-mauve shrink-0">{{ task().decision!.status }}</span>
              <span class="text-sm font-medium text-text-primary">{{ task().decision!.title }}</span>
            </div>
            @if (task().decision!.rationale_excerpt) {
              <p class="text-xs text-text-secondary line-clamp-3">{{ task().decision!.rationale_excerpt }}</p>
            }
          </div>
        </div>
      }

      <!-- Parent Task -->
      @if (parentTask()) {
        <div class="mb-4">
          <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{{ t('tasks.parentTask') }}</h3>
          <button (click)="navigateToTask.emit(parentTask()!.id)"
            class="flex items-center gap-2 bg-bg rounded-lg px-3 py-2 border border-border hover:border-accent/50 transition-colors cursor-pointer w-full text-left">
            <svg class="w-4 h-4 shrink-0 text-ctp-mauve" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
              <path d="M9 5l7 7-7 7" />
            </svg>
            <span class="px-1.5 py-0.5 rounded-full text-[10px] font-medium shrink-0 {{ stateColor(parentTask()!.state) }}">{{ parentTask()!.state }}</span>
            <span class="text-text-secondary text-xs">#{{ parentTask()!.number }}</span>
            <span class="text-sm text-text-primary truncate">{{ parentTask()!.title }}</span>
          </button>
        </div>
      }

      <!-- Subtasks -->
      @if (subtasks().length > 0) {
        <div class="mb-4">
          <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{{ t('tasks.subtasks') }}</h3>
          <div class="space-y-1">
            @for (child of subtasks(); track child.id) {
              <button (click)="navigateToTask.emit(child.id)"
                class="flex items-center gap-2 bg-bg rounded px-3 py-1.5 border border-border hover:border-accent/50 transition-colors cursor-pointer w-full text-left">
                <span class="px-1.5 py-0.5 rounded-full text-[10px] font-medium shrink-0 {{ stateColor(child.state) }}">{{ child.state }}</span>
                <span class="text-text-secondary text-xs">#{{ child.number }}</span>
                <span class="text-xs text-text-primary truncate flex-1">{{ child.title }}</span>
                @if (child.assigned_agent_id) {
                  <span class="text-[10px] text-text-muted font-mono shrink-0">{{ child.assigned_agent_id | slice:0:8 }}</span>
                }
              </button>
            }
          </div>
        </div>
      }

      <!-- Spec / Description -->
      <div class="mb-4">
        <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('tasks.spec') }}</h3>
        @if (editingSpec()) {
          <textarea id="inline-spec-edit" [(ngModel)]="editSpec" rows="6"
            (keydown.escape)="cancelSpecEdit(); $event.stopPropagation()"
            class="w-full text-sm text-text-primary bg-surface rounded-lg p-3 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent resize-y font-mono"></textarea>
          <div class="flex justify-end gap-2 mt-1">
            <button (click)="cancelSpecEdit()"
              class="px-2 py-1 text-xs text-text-secondary hover:text-text-primary">
              {{ t('tasks.cancel') }}
            </button>
            <button (click)="saveSpecEdit()"
              class="px-2 py-1 text-xs bg-accent text-bg rounded hover:opacity-90">
              {{ t('tasks.save') }}
            </button>
          </div>
        } @else {
          @if (spec()) {
            <pre (click)="startSpecEdit()" (keydown.enter)="startSpecEdit()"
              tabindex="0" role="button"
              class="text-sm text-text-primary whitespace-pre-wrap bg-bg rounded-lg p-3 border border-border max-h-60 overflow-y-auto cursor-pointer hover:border-accent/50 transition-colors">{{ spec() }}</pre>
          } @else {
            <p (click)="startSpecEdit()" (keydown.enter)="startSpecEdit()"
              tabindex="0" role="button"
              class="text-sm text-text-muted bg-bg rounded-lg p-3 border border-border border-dashed cursor-pointer hover:border-accent/50 transition-colors">
              {{ t('tasks.specField') }}...
            </p>
          }
        }
      </div>

      <!-- Files -->
      @if (files().length > 0) {
        <div class="mb-4">
          <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('tasks.files') }}</h3>
          <div class="space-y-0.5">
            @for (file of files(); track file) {
              <p class="text-xs text-ctp-green font-mono">{{ file }}</p>
            }
          </div>
        </div>
      }

      <!-- Test command -->
      @if (testCmd()) {
        <div class="mb-4">
          <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('tasks.testCmd') }}</h3>
          <code class="text-xs text-ctp-green font-mono bg-bg rounded px-2 py-1">{{ testCmd() }}</code>
        </div>
      }

      <!-- Verifications -->
      @if (verifications().length > 0) {
        <div class="mb-4">
          <div class="flex items-center justify-between mb-2">
            <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider">Verifications</h3>
            <a [routerLink]="['/verifications']" [queryParams]="{ task_id: task().id }"
              class="text-[10px] text-accent hover:underline">view all</a>
          </div>
          <div class="space-y-1">
            @for (v of verifications(); track v.id) {
              <div class="flex items-center gap-2 bg-bg rounded px-3 py-1.5 border border-border">
                <span class="px-1.5 py-0.5 rounded-full text-[10px] font-medium shrink-0 {{ verificationStatusColor(v.status) }}">{{ v.status }}</span>
                <span class="px-1.5 py-0.5 rounded-full text-[10px] font-medium shrink-0 {{ verificationKindColor(v.kind) }}">{{ v.kind }}</span>
                <span class="text-xs text-text-primary truncate">{{ v.title }}</span>
              </div>
            }
          </div>
        </div>
      } @else {
        <div class="mb-4">
          <div class="flex items-center justify-between">
            <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider">Verifications</h3>
            <a [routerLink]="['/verifications']" [queryParams]="{ task_id: task().id }"
              class="text-[10px] text-accent hover:underline">view all</a>
          </div>
          <p class="text-xs text-text-muted mt-1">No verifications yet</p>
        </div>
      }

      <!-- Dependencies: Depends on -->
      @if (dependencies().depends_on.length > 0) {
        <div class="mb-4">
          <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{{ t('tasks.dependsOn') }}</h3>
          <div class="space-y-1">
            @for (dep of dependencies().depends_on; track dep.depends_on) {
              <div class="flex items-center justify-between bg-bg rounded px-3 py-1.5 border border-border">
                <div class="flex items-center gap-2 min-w-0">
                  <span class="px-1.5 py-0.5 rounded-full text-[10px] font-medium shrink-0 {{ stateColor(dep.state) }}">{{ dep.state }}</span>
                  <span class="text-xs text-text-primary truncate">{{ dep.title }}</span>
                  <span class="text-[10px] text-text-muted font-mono shrink-0">{{ dep.depends_on | slice:0:8 }}</span>
                </div>
                <button (click)="removeDep.emit(dep.depends_on)"
                  class="text-ctp-red hover:text-ctp-red/80 text-xs shrink-0 ml-2">✕</button>
              </div>
            }
          </div>
        </div>
      }

      <!-- Dependencies: Blocks -->
      @if (dependencies().blocks.length > 0) {
        <div class="mb-4">
          <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{{ t('tasks.blocks') }}</h3>
          <div class="space-y-1">
            @for (dep of dependencies().blocks; track dep.task_id) {
              <div class="flex items-center gap-2 bg-bg rounded px-3 py-1.5 border border-border">
                <span class="px-1.5 py-0.5 rounded-full text-[10px] font-medium shrink-0 {{ stateColor(dep.state) }}">{{ dep.state }}</span>
                <span class="text-xs text-text-primary truncate">{{ dep.title }}</span>
                <span class="text-[10px] text-text-muted font-mono shrink-0">{{ dep.task_id | slice:0:8 }}</span>
              </div>
            }
          </div>
        </div>
      }

      <!-- Changed files with inline diff -->
      <div class="mb-4">
        <app-changed-files
          [taskId]="task().id"
          [files]="changedFiles()" />
      </div>

      <!-- Add dependency -->
      <div class="mb-4">
        <div class="flex gap-2">
          <input type="text" [(ngModel)]="depId" [placeholder]="t('tasks.depIdPlaceholder')"
            class="flex-1 bg-surface text-text-primary text-xs rounded px-2 py-1.5 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary"
            (keydown.enter)="addDep()" />
          <button (click)="addDep()" [disabled]="!depId.trim()"
            class="px-3 py-1.5 bg-accent text-bg rounded-lg text-xs font-medium hover:opacity-90 disabled:opacity-30">
            {{ t('tasks.addDep') }}
          </button>
        </div>
      </div>

      <!-- Footer -->
      <div class="flex items-center justify-between pt-3 mt-4 border-t border-border">
        <span class="text-xs text-text-muted">{{ t('tasks.updatedAt') }}: {{ task().updated_at | date:'medium' }}</span>
        <div class="flex items-center gap-3">
          @if (task().state === 'done' || task().state === 'cancelled') {
            @if (!confirmingRevert) {
              <button (click)="confirmingRevert = true" [disabled]="reverting()"
                class="text-xs text-ctp-yellow hover:text-ctp-yellow/80 transition-colors disabled:opacity-50">
                @if (reverting()) {
                  {{ t('tasks.reverting') }}
                } @else {
                  {{ t('tasks.revert') }}
                }
              </button>
            } @else {
              <div class="flex items-center gap-2">
                <span class="text-xs text-ctp-yellow">{{ t('tasks.revertConfirm') }}</span>
                <button (click)="revertClick.emit(); confirmingRevert = false"
                  class="px-2 py-0.5 text-xs rounded bg-ctp-yellow/20 text-ctp-yellow hover:bg-ctp-yellow/30">
                  {{ t('tasks.revertYes') }}
                </button>
                <button (click)="confirmingRevert = false"
                  class="px-2 py-0.5 text-xs rounded border border-border text-text-secondary hover:text-text-primary">
                  {{ t('tasks.cancel') }}
                </button>
              </div>
            }
          }
          @if (!confirmingDelete) {
            <button (click)="confirmingDelete = true"
              class="text-xs text-ctp-red hover:text-ctp-red/80 transition-colors">
              {{ t('tasks.delete') }}
            </button>
          } @else {
            <div class="flex items-center gap-2">
              <span class="text-xs text-ctp-red">{{ t('tasks.deleteConfirm') }}</span>
              <button (click)="deleteClick.emit(); confirmingDelete = false"
                class="px-2 py-0.5 text-xs rounded bg-ctp-red/20 text-ctp-red hover:bg-ctp-red/30">
                {{ t('tasks.deleteYes') }}
              </button>
              <button (click)="confirmingDelete = false"
                class="px-2 py-0.5 text-xs rounded border border-border text-text-secondary hover:text-text-primary">
                {{ t('tasks.cancel') }}
              </button>
            </div>
          }
        </div>
      </div>
    </div>
  `,
})
export class TaskDetailComponent {
  task = input.required<SpTask>();
  embedded = input(false);
  updates = input<SpTaskUpdate[]>([]);
  comments = input<SpTaskComment[]>([]);
  dependencies = input<SpTaskDependencies>({ depends_on: [], blocks: [] });
  verifications = input<SpVerification[]>([]);
  changedFiles = input<ChangedFileSummary[]>([]);
  gitStatus = input<TaskBranchStatus | null>(null);
  playbooks = input<SpPlaybook[]>([]);
  pushing = input(false);
  reverting = input(false);
  resolving = input(false);
  updatesLoading = input(false);
  commentsLoading = input(false);
  kinds = input<string[]>(DEFAULT_TASK_KINDS);
  parentTask = input<SpTask | null>(null);
  subtasks = input<SpTask[]>([]);
  closed = output<void>();
  transitionClick = output<string>();
  claimClick = output<void>();
  pushClick = output<string>();
  resolveClick = output<void>();
  revertClick = output<void>();
  postUpdate = output<{ kind: string; content: string }>();
  postComment = output<string>();
  addDepClick = output<string>();
  removeDep = output<string>();
  deleteClick = output<void>();
  playbookChange = output<string | null>();
  playbookStepChange = output<number>();
  inlineUpdate = output<UpdateTaskRequest>();
  navigateToTask = output<string>();

  depId = '';
  confirmingDelete = false;
  confirmingRevert = false;
  stateMenuOpen = signal(false);
  copied = signal(false);
  editingTitle = signal(false);
  editTitle = '';
  editingSpec = signal(false);
  editSpec = '';

  @HostListener('document:click')
  closeMenus(): void {
    this.stateMenuOpen.set(false);
  }

  toggleStateMenu(event: Event): void {
    event.stopPropagation();
    this.stateMenuOpen.set(!this.stateMenuOpen());
  }

  onStateMenuSelect(event: Event, target: string): void {
    event.stopPropagation();
    this.stateMenuOpen.set(false);
    this.transitionClick.emit(target);
  }

  transitions(): string[] {
    return taskTransitions(this.task().state);
  }

  spec(): string {
    const ctx = this.task().context;
    return (ctx?.['spec'] as string) ?? '';
  }

  files(): string[] {
    const ctx = this.task().context;
    return (ctx?.['files'] as string[]) ?? [];
  }

  testCmd(): string {
    const ctx = this.task().context;
    return (ctx?.['test_cmd'] as string) ?? '';
  }

  protected readonly stateColor = taskStateColor;

  verificationStatusColor(status: VerificationStatus): string {
    return VERIFICATION_STATUS_COLORS[status] ?? 'bg-surface-hover text-text-muted';
  }

  verificationKindColor(kind: VerificationKind): string {
    return VERIFICATION_KIND_COLORS[kind] ?? 'bg-surface-hover text-text-muted';
  }

  currentPlaybook(): SpPlaybook | undefined {
    const id = this.task().playbook_id;
    if (!id) return undefined;
    return this.playbooks().find(pb => pb.id === id);
  }

  onPlaybookChange(event: Event): void {
    const value = (event.target as HTMLSelectElement).value;
    this.playbookChange.emit(value || null);
  }

  onStepClick(step: number): void {
    if (step !== this.task().playbook_step) {
      this.playbookStepChange.emit(step);
    }
  }

  copyId(): void {
    navigator.clipboard.writeText(this.task().id).then(() => {
      this.copied.set(true);
      setTimeout(() => this.copied.set(false), 2000);
    });
  }

  startTitleEdit(): void {
    this.editTitle = this.task().title;
    this.editingTitle.set(true);
    setTimeout(() => {
      const el = document.getElementById('inline-title-edit') as HTMLInputElement | null;
      el?.focus();
      el?.select();
    });
  }

  saveTitleEdit(): void {
    if (!this.editingTitle()) return;
    const newTitle = this.editTitle.trim();
    this.editingTitle.set(false);
    if (newTitle && newTitle !== this.task().title) {
      this.inlineUpdate.emit({ title: newTitle });
    }
  }

  cancelTitleEdit(): void {
    this.editingTitle.set(false);
  }

  onKindChange(event: Event): void {
    const value = (event.target as HTMLSelectElement).value;
    if (value !== this.task().kind) {
      this.inlineUpdate.emit({ kind: value });
    }
  }

  onUrgentToggle(): void {
    this.inlineUpdate.emit({ urgent: !this.task().urgent });
  }

  startSpecEdit(): void {
    this.editSpec = this.spec();
    this.editingSpec.set(true);
    setTimeout(() => {
      const el = document.getElementById('inline-spec-edit') as HTMLTextAreaElement | null;
      el?.focus();
    });
  }

  saveSpecEdit(): void {
    const newSpec = this.editSpec.trim();
    this.editingSpec.set(false);
    const oldSpec = this.spec();
    if (newSpec !== oldSpec) {
      const context = { ...this.task().context };
      if (newSpec) {
        context['spec'] = newSpec;
      } else {
        delete context['spec'];
      }
      this.inlineUpdate.emit({ context });
    }
  }

  cancelSpecEdit(): void {
    this.editingSpec.set(false);
  }

  addDep(): void {
    const id = this.depId.trim();
    if (!id) return;
    this.addDepClick.emit(id);
    this.depId = '';
  }
}
