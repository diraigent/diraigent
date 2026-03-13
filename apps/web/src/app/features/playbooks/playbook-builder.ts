import { Component, inject, signal, OnInit, PLATFORM_ID } from '@angular/core';
import { isPlatformBrowser } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { Router, ActivatedRoute } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import {
  CdkDragDrop,
  CdkDrag,
  CdkDragHandle,
  CdkDropList,
  moveItemInArray,
} from '@angular/cdk/drag-drop';
import {
  PlaybooksApiService,
  SpPlaybookStep,
  GitStrategyId,
} from '../../core/services/playbooks-api.service';
import {
  StepTemplatesApiService,
  SpStepTemplate,
  SpCreateStepTemplate,
} from '../../core/services/step-templates-api.service';
import { ProjectContext } from '../../core/services/project-context.service';

@Component({
  selector: 'app-playbook-builder',
  standalone: true,
  imports: [FormsModule, TranslocoModule, CdkDrag, CdkDragHandle, CdkDropList],
  template: `
    <div class="p-3 sm:p-6 max-w-4xl mx-auto" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <div class="flex items-center gap-3">
          <button (click)="goBack()" class="p-2 text-text-secondary hover:text-text-primary rounded-lg hover:bg-surface">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
              <path d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <h1 class="text-2xl font-semibold text-text-primary">
            {{ isDefault ? t('playbooks.cloneTitle') : (editId ? t('playbooks.editTitle') : t('playbooks.createTitle')) }}
          </h1>
        </div>
        <div class="flex items-center gap-3">
          <button
            (click)="goBack()"
            class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
            {{ t('playbooks.cancel') }}
          </button>
          <button
            (click)="save()"
            [disabled]="!title.trim() || steps().length === 0 || saving()"
            class="px-5 py-2.5 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90
                   disabled:opacity-40 disabled:cursor-not-allowed">
            {{ saving() ? t('common.loading') : (isDefault ? t('playbooks.clone') : (editId ? t('playbooks.save') : t('playbooks.create'))) }}
          </button>
        </div>
      </div>

      <!-- Fork notice for default playbooks -->
      @if (isDefault) {
        <div class="mb-6 p-3 rounded-lg bg-accent/10 border border-accent/30 text-sm text-text-secondary">
          {{ t('playbooks.forkNotice') }}
        </div>
      }

      <!-- Title + Description + Tags -->
      <div class="grid grid-cols-1 gap-4 mb-8">
        <div>
          <label for="pb-title" class="block text-sm text-text-secondary mb-1">{{ t('playbooks.fieldTitle') }}</label>
          <input id="pb-title" type="text" [(ngModel)]="title" [placeholder]="t('playbooks.fieldTitle')"
            class="w-full bg-surface text-text-primary rounded-lg px-3 py-2.5 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent text-sm" />
        </div>
        <div>
          <label for="pb-trigger" class="block text-sm text-text-secondary mb-1">{{ t('playbooks.fieldTrigger') }}</label>
          <textarea id="pb-trigger" [(ngModel)]="trigger" rows="3" [placeholder]="t('playbooks.fieldTrigger')"
            class="w-full bg-surface text-text-primary rounded-lg px-3 py-2.5 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent text-sm resize-y"></textarea>
        </div>
        <div>
          <label for="pb-tags" class="block text-sm text-text-secondary mb-1">{{ t('playbooks.fieldTags') }}</label>
          <input id="pb-tags" type="text" [(ngModel)]="tags" [placeholder]="t('playbooks.tagsPlaceholder')"
            class="w-full max-w-md bg-surface text-text-primary rounded-lg px-3 py-2.5 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent text-sm" />
        </div>
        <div>
          <label for="pb-initial-state" class="block text-sm text-text-secondary mb-1">Initial task state</label>
          <select id="pb-initial-state" [(ngModel)]="initialState"
            class="w-full max-w-md bg-surface text-text-primary rounded-lg px-3 py-2.5 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent text-sm">
            <option value="ready">Ready — tasks are immediately queued</option>
            <option value="backlog">Backlog — tasks wait for manual promotion</option>
          </select>
        </div>
        <div>
          <div class="flex items-center gap-1.5 mb-1">
            <label for="pb-git-strategy" class="block text-sm text-text-secondary">Git strategy</label>
            <div class="relative">
              <button type="button" (click)="showGitStrategyInfo = !showGitStrategyInfo"
                class="w-4 h-4 rounded-full border border-text-muted text-text-muted text-[10px] font-bold
                       flex items-center justify-center hover:border-accent hover:text-accent transition-colors"
                title="Git strategy info">?</button>
              @if (showGitStrategyInfo) {
                <div class="absolute left-6 top-1/2 -translate-y-1/2 z-10 w-80 bg-surface border border-border
                            rounded-lg shadow-lg p-3 text-xs text-text-secondary">
                  <div class="flex items-center justify-between mb-2">
                    <span class="font-medium text-text-primary text-sm">Git strategies</span>
                    <button type="button" (click)="showGitStrategyInfo = false"
                      class="text-text-muted hover:text-text-primary">
                      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path d="M6 18L18 6M6 6l12 12"/></svg>
                    </button>
                  </div>
                  <dl class="space-y-2">
                    <div>
                      <dt class="font-medium text-text-primary">Merge to default branch</dt>
                      <dd>Branch from the default branch, merge back when done. Standard autonomous workflow.</dd>
                    </div>
                    <div>
                      <dt class="font-medium text-text-primary">Branch only (for PRs)</dt>
                      <dd>Branch from default, push branch to origin. No automatic merge. Use this for PR-based workflows.</dd>
                    </div>
                    <div>
                      <dt class="font-medium text-text-primary">Merge to target branch</dt>
                      <dd>Branch from and merge to a specified target branch (e.g. develop, staging). Requires a target branch name.</dd>
                    </div>
                    <div>
                      <dt class="font-medium text-text-primary">Feature branch (per goal)</dt>
                      <dd>Tasks branch from a goal branch (e.g. goal/&lt;slug&gt;) and merge back into it. The goal branch merges to default when the goal is completed.</dd>
                    </div>
                    <div>
                      <dt class="font-medium text-text-primary">No git</dt>
                      <dd>Plain directory, no git operations. For non-code tasks.</dd>
                    </div>
                  </dl>
                </div>
              }
            </div>
          </div>
          <select id="pb-git-strategy" [(ngModel)]="gitStrategy"
            class="w-full max-w-md bg-surface text-text-primary rounded-lg px-3 py-2.5 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent text-sm">
            <option value="merge_to_default">Merge to default branch</option>
            <option value="branch_only">Branch only (no merge, for PRs)</option>
            <option value="branch_to_target">Merge to target branch</option>
            <option value="feature_branch">Feature branch (per goal)</option>
            <option value="no_git">No git</option>
          </select>
        </div>
        @if (gitStrategy === 'branch_to_target') {
          <div>
            <label for="pb-git-target" class="block text-sm text-text-secondary mb-1">Target branch</label>
            <input id="pb-git-target" type="text" [(ngModel)]="gitTargetBranch" placeholder="develop"
              class="w-full max-w-md bg-surface text-text-primary rounded-lg px-3 py-2.5 border border-border
                     focus:outline-none focus:ring-1 focus:ring-accent text-sm" />
          </div>
        }
      </div>

      <!-- Steps section -->
      <div>
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-sm font-medium text-text-secondary uppercase tracking-wide">
            {{ t('playbooks.stepsTitle') }} ({{ steps().length }})
          </h2>
          <div class="flex items-center gap-2">
            <button (click)="addStep()" class="text-xs text-accent hover:underline">
              {{ t('playbooks.addStep') }}
            </button>
            <span class="text-text-muted text-xs">|</span>
            <div class="relative">
              <button (click)="toggleTemplatePicker()" class="text-xs text-accent hover:underline">
                Add from Template
              </button>
              @if (showTemplatePicker()) {
                <div (click)="showTemplatePicker.set(false)" (keydown.enter)="showTemplatePicker.set(false)" tabindex="0" role="button" aria-label="Close template picker" class="fixed inset-0 z-10"></div>
                <div class="absolute right-0 top-full mt-1 z-20 w-80 max-h-64 overflow-y-auto
                            bg-surface border border-border rounded-lg shadow-lg">
                  @if (templates().length === 0) {
                    <div class="p-4 text-xs text-text-muted text-center">No templates available</div>
                  } @else {
                    @for (tpl of templates(); track tpl.id) {
                      <button (click)="addStepFromTemplate(tpl)"
                        class="w-full text-left px-3 py-2.5 hover:bg-accent/10 border-b border-border
                               last:border-b-0 transition-colors">
                        <div class="text-sm font-medium text-text-primary">{{ tpl.name }}</div>
                        @if (tpl.description) {
                          <div class="text-xs text-text-muted mt-0.5 line-clamp-2">{{ tpl.description }}</div>
                        }
                        <div class="flex items-center gap-2 mt-1">
                          @if (tpl.model) {
                            <span class="text-[10px] px-1.5 py-0.5 rounded bg-accent/10 text-accent">{{ tpl.model }}</span>
                          }
                          @if (tpl.budget) {
                            <span class="text-[10px] text-text-muted">{{ '$' + tpl.budget }}</span>
                          }
                          @if (tpl.tenant_id === null) {
                            <span class="text-[10px] px-1.5 py-0.5 rounded bg-text-muted/10 text-text-muted">global</span>
                          }
                        </div>
                      </button>
                    }
                  }
                </div>
              }
            </div>
          </div>
        </div>

        <!-- Drag-drop step list -->
        <div
          cdkDropList
          [cdkDropListData]="steps()"
          (cdkDropListDropped)="dropStep($event)"
          class="space-y-3">
          @for (step of steps(); track $index; let i = $index) {
            <div cdkDrag [cdkDragDisabled]="isTouch()"
              class="bg-surface rounded-lg border border-border p-4 transition-colors hover:border-accent/50 cursor-grab active:cursor-grabbing">
              <div cdkDragPlaceholder class="bg-accent/10 rounded-lg border-2 border-dashed border-accent/30 h-32"></div>

              <!-- Step header -->
              <div class="flex items-center justify-between mb-3">
                <div class="flex items-center gap-2">
                  <!-- Drag handle (hidden on touch devices) -->
                  @if (!isTouch()) {
                    <span cdkDragHandle class="p-1 text-text-muted hover:text-text-secondary cursor-grab">
                      <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
                        <path d="M8 6a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm0 8a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm0 8a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm8-16a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm0 8a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm0 8a2 2 0 1 1 0-4 2 2 0 0 1 0 4z"/>
                      </svg>
                    </span>
                  }
                  <span class="w-6 h-6 bg-accent/20 text-accent rounded-full text-xs font-bold flex items-center justify-center shrink-0">
                    {{ i + 1 }}
                  </span>
                  @if (step.step_template_id) {
                    <span class="flex items-center gap-1 text-[10px] text-accent bg-accent/10 px-1.5 py-0.5 rounded">
                      <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                        <path d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1"/>
                      </svg>
                      {{ templateNames()[step.step_template_id] || 'template' }}
                    </span>
                  }
                </div>
                <div class="flex gap-1">
                  @if (i > 0) {
                    <button (click)="moveStep(i, -1)" class="p-1 text-text-secondary hover:text-text-primary" [title]="t('playbooks.moveUp')">
                      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path d="M5 15l7-7 7 7"/></svg>
                    </button>
                  }
                  @if (i < steps().length - 1) {
                    <button (click)="moveStep(i, 1)" class="p-1 text-text-secondary hover:text-text-primary" [title]="t('playbooks.moveDown')">
                      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path d="M19 9l-7 7-7-7"/></svg>
                    </button>
                  }
                  @if (step.step_template_id) {
                    <button (click)="detachTemplate(i)" class="p-1 text-text-secondary hover:text-ctp-yellow" title="Detach from template">
                      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                        <path d="M18.84 12.25l1.72-1.71h-.02a5.004 5.004 0 00-7.07-7.07l-1.72 1.71m-6.58 6.57L3.47 13.46a5.003 5.003 0 007.07 7.07l1.71-1.71M8 12h8"/>
                      </svg>
                    </button>
                  }
                  <button (click)="saveAsTemplate(i)" [disabled]="savingTemplate() === i" class="p-1 text-text-secondary hover:text-accent" title="Save as Template">
                    @if (savingTemplate() === i) {
                      <svg class="w-3.5 h-3.5 animate-spin" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                        <path d="M12 2v4m0 12v4m-7.071-3.929l2.828-2.828m8.486-8.486l2.828-2.828M2 12h4m12 0h4m-3.929 7.071l-2.828-2.828M7.757 7.757L4.929 4.929"/>
                      </svg>
                    } @else {
                      <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                        <path d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4"/>
                      </svg>
                    }
                  </button>
                  <button (click)="removeStep(i)" class="p-1 text-text-secondary hover:text-ctp-red" [title]="t('playbooks.removeStep')">
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path d="M6 18L18 6M6 6l12 12"/></svg>
                  </button>
                </div>
              </div>

              <!-- Step fields -->
              <div class="mb-3">
                <label [attr.for]="'pb-step-name-' + i" class="text-xs text-text-muted mb-0.5 block">{{ t('playbooks.stepName') }}</label>
                <input [id]="'pb-step-name-' + i" type="text" [ngModel]="step.name" (ngModelChange)="updateField(i, 'name', $event)"
                  [placeholder]="t('playbooks.stepName')"
                  class="w-full bg-bg text-text-primary text-sm rounded px-2.5 py-1.5 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent" />
              </div>

              <div class="mb-3">
                <label [attr.for]="'pb-step-desc-' + i" class="text-xs text-text-muted mb-0.5 block">{{ t('playbooks.stepDescription') }}</label>
                <textarea [id]="'pb-step-desc-' + i" [ngModel]="step.description" (ngModelChange)="updateField(i, 'description', $event)"
                  rows="10" [placeholder]="t('playbooks.stepDescription')"
                  class="w-full bg-bg text-text-primary text-xs rounded px-2.5 py-1.5 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
              </div>

              <div class="grid grid-cols-2 gap-3">
                <div>
                  <label [attr.for]="'pb-step-model-' + i" class="text-xs text-text-muted mb-0.5 block">{{ t('playbooks.model') }}</label>
                  <select [id]="'pb-step-model-' + i" [ngModel]="step.model ?? ''" (ngModelChange)="updateField(i, 'model', $event || undefined)"
                    class="w-full bg-bg text-text-primary text-xs rounded px-2.5 py-1.5 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent">
                    <option value="">{{ t('playbooks.modelDefault') }}</option>
                    <option value="claude-sonnet-4-6">Sonnet</option>
                    <option value="claude-opus-4-6">Opus</option>
                    <option value="claude-haiku-4-5-20251001">Haiku</option>
                  </select>
                </div>
                <div>
                  <label [attr.for]="'pb-step-budget-' + i" class="text-xs text-text-muted mb-0.5 block">Budget ($)</label>
                  <input [id]="'pb-step-budget-' + i" type="number" [ngModel]="step.budget ?? ''" (ngModelChange)="updateField(i, 'budget', $event || undefined)"
                    min="0.1" step="0.5" placeholder="default"
                    class="w-full bg-bg text-text-primary text-xs rounded px-2.5 py-1.5 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                </div>
                <div>
                  <label [attr.for]="'pb-step-tools-' + i" class="text-xs text-text-muted mb-0.5 block">Allowed tools</label>
                  <select [id]="'pb-step-tools-' + i" [ngModel]="step.allowed_tools ?? ''" (ngModelChange)="updateField(i, 'allowed_tools', $event || undefined)"
                    class="w-full bg-bg text-text-primary text-xs rounded px-2.5 py-1.5 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent">
                    <option value="">Default</option>
                    <option value="full">Full (read + write)</option>
                    <option value="readonly">Read-only</option>
                    <option value="merge">Merge (git only)</option>
                  </select>
                </div>
                <div>
                  <label [attr.for]="'pb-step-ctx-' + i" class="text-xs text-text-muted mb-0.5 block">Context level</label>
                  <select [id]="'pb-step-ctx-' + i" [ngModel]="step.context_level ?? ''" (ngModelChange)="updateField(i, 'context_level', $event || undefined)"
                    class="w-full bg-bg text-text-primary text-xs rounded px-2.5 py-1.5 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent">
                    <option value="">Default</option>
                    <option value="full">Full context</option>
                    <option value="minimal">Minimal</option>
                    <option value="dream">Dream (analysis)</option>
                  </select>
                </div>
                <div>
                  <label [attr.for]="'pb-step-oncomplete-' + i" class="text-xs text-text-muted mb-0.5 block">On complete</label>
                  <select [id]="'pb-step-oncomplete-' + i" [ngModel]="step.on_complete ?? ''" (ngModelChange)="updateField(i, 'on_complete', $event || undefined)"
                    class="w-full bg-bg text-text-primary text-xs rounded px-2.5 py-1.5 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent">
                    <option value="">Default (next)</option>
                    <option value="next">Next step</option>
                    <option value="done">Done</option>
                    <option value="human_review">Human review</option>
                  </select>
                </div>
                <div>
                  <label [attr.for]="'pb-step-timeout-' + i" class="text-xs text-text-muted mb-0.5 block">Timeout (min)</label>
                  <input [id]="'pb-step-timeout-' + i" type="number" [ngModel]="step.timeout_minutes ?? ''" (ngModelChange)="updateField(i, 'timeout_minutes', $event || undefined)"
                    min="1" step="1" placeholder="none"
                    class="w-full bg-bg text-text-primary text-xs rounded px-2.5 py-1.5 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                </div>
                <div>
                  <label [attr.for]="'pb-step-gitaction-' + i" class="text-xs text-text-muted mb-0.5 block">Git action</label>
                  <select [id]="'pb-step-gitaction-' + i" [ngModel]="step.git_action ?? 'none'" (ngModelChange)="updateField(i, 'git_action', $event === 'none' ? undefined : $event)"
                    class="w-full bg-bg text-text-primary text-xs rounded px-2.5 py-1.5 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent">
                    <option value="none">None</option>
                    <option value="merge">Merge to target</option>
                    <option value="push">Push branch</option>
                  </select>
                </div>
                <div>
                  <label [attr.for]="'pb-step-agent-' + i" class="text-xs text-text-muted mb-0.5 block">Agent name</label>
                  <input [id]="'pb-step-agent-' + i" type="text" [ngModel]="step.agent ?? ''" (ngModelChange)="updateField(i, 'agent', $event || undefined)"
                    placeholder="default"
                    class="w-full bg-bg text-text-primary text-xs rounded px-2.5 py-1.5 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                </div>
                <div>
                  <label [attr.for]="'pb-step-maxcycles-' + i" class="text-xs text-text-muted mb-0.5 block">{{ t('playbooks.maxCycles') }}</label>
                  <input [id]="'pb-step-maxcycles-' + i" type="number" [ngModel]="step.max_cycles ?? ''" (ngModelChange)="updateField(i, 'max_cycles', $event || undefined)"
                    min="0" step="1" placeholder="default"
                    class="w-full bg-bg text-text-primary text-xs rounded px-2.5 py-1.5 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                </div>
                <div>
                  <label [attr.for]="'pb-step-retriable-' + i" class="text-xs text-text-muted mb-0.5 block">{{ t('playbooks.retriable') }}</label>
                  <select [id]="'pb-step-retriable-' + i" [ngModel]="step.retriable === null || step.retriable === undefined ? '' : (step.retriable ? 'true' : 'false')" (ngModelChange)="updateField(i, 'retriable', $event === '' ? undefined : $event === 'true')"
                    class="w-full bg-bg text-text-primary text-xs rounded px-2.5 py-1.5 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent">
                    <option value="">{{ t('playbooks.modelDefault') }}</option>
                    <option value="true">Yes</option>
                    <option value="false">No</option>
                  </select>
                </div>
              </div>

              <!-- Advanced JSON fields -->
              <details class="mt-3">
                <summary class="text-xs text-text-muted cursor-pointer hover:text-text-secondary">Advanced (JSON)</summary>
                <div class="grid grid-cols-1 gap-3 mt-2">
                  <div>
                    <label [attr.for]="'pb-step-mcp-' + i" class="text-xs text-text-muted mb-0.5 block">MCP servers</label>
                    <textarea [id]="'pb-step-mcp-' + i" [ngModel]="toJson(step.mcp_servers)" (ngModelChange)="updateJsonField(i, 'mcp_servers', $event)"
                      rows="5" placeholder='{"mcpServers": {}}'
                      class="w-full bg-bg text-text-primary text-xs font-mono rounded px-2.5 py-1.5 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
                  </div>
                  <div>
                    <label [attr.for]="'pb-step-agents-' + i" class="text-xs text-text-muted mb-0.5 block">Sub-agents</label>
                    <textarea [id]="'pb-step-agents-' + i" [ngModel]="toJson(step.agents)" (ngModelChange)="updateJsonField(i, 'agents', $event)"
                      rows="5" placeholder='{"name": {"description": "...", "prompt": "..."}}'
                      class="w-full bg-bg text-text-primary text-xs font-mono rounded px-2.5 py-1.5 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
                  </div>
                  <div>
                    <label [attr.for]="'pb-step-settings-' + i" class="text-xs text-text-muted mb-0.5 block">Settings</label>
                    <textarea [id]="'pb-step-settings-' + i" [ngModel]="toJson(step.settings)" (ngModelChange)="updateJsonField(i, 'settings', $event)"
                      rows="5" placeholder="{}"
                      class="w-full bg-bg text-text-primary text-xs font-mono rounded px-2.5 py-1.5 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
                  </div>
                  <div>
                    <label [attr.for]="'pb-step-env-' + i" class="text-xs text-text-muted mb-0.5 block">Environment variables</label>
                    <textarea [id]="'pb-step-env-' + i" [ngModel]="toJson(step.env)" (ngModelChange)="updateJsonField(i, 'env', $event)"
                      rows="5" placeholder='{"KEY": "value"}'
                      class="w-full bg-bg text-text-primary text-xs font-mono rounded px-2.5 py-1.5 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
                  </div>
                </div>
              </details>
            </div>
          }
        </div>

        @if (steps().length === 0) {
          <div class="bg-surface/50 rounded-lg border border-dashed border-border p-8 text-center">
            <p class="text-sm text-text-muted mb-3">{{ t('playbooks.noSteps') }}</p>
            <div class="flex items-center justify-center gap-3">
              <button (click)="addStep()" class="px-4 py-2 bg-accent text-bg rounded-lg text-xs font-medium hover:opacity-90">
                {{ t('playbooks.addStep') }}
              </button>
              <div class="relative">
                <button (click)="toggleTemplatePicker()" class="px-4 py-2 bg-surface text-text-primary border border-border rounded-lg text-xs font-medium hover:border-accent/50">
                  Add from Template
                </button>
                @if (showTemplatePicker()) {
                  <div (click)="showTemplatePicker.set(false)" (keydown.enter)="showTemplatePicker.set(false)" tabindex="0" role="button" aria-label="Close template picker" class="fixed inset-0 z-10"></div>
                  <div class="absolute left-1/2 -translate-x-1/2 top-full mt-1 z-20 w-80 max-h-64 overflow-y-auto
                              bg-surface border border-border rounded-lg shadow-lg">
                    @if (templates().length === 0) {
                      <div class="p-4 text-xs text-text-muted text-center">No templates available</div>
                    } @else {
                      @for (tpl of templates(); track tpl.id) {
                        <button (click)="addStepFromTemplate(tpl)"
                          class="w-full text-left px-3 py-2.5 hover:bg-accent/10 border-b border-border
                                 last:border-b-0 transition-colors">
                          <div class="text-sm font-medium text-text-primary">{{ tpl.name }}</div>
                          @if (tpl.description) {
                            <div class="text-xs text-text-muted mt-0.5 line-clamp-2">{{ tpl.description }}</div>
                          }
                          <div class="flex items-center gap-2 mt-1">
                            @if (tpl.model) {
                              <span class="text-[10px] px-1.5 py-0.5 rounded bg-accent/10 text-accent">{{ tpl.model }}</span>
                            }
                            @if (tpl.budget) {
                              <span class="text-[10px] text-text-muted">{{ '$' + tpl.budget }}</span>
                            }
                          </div>
                        </button>
                      }
                    }
                  </div>
                }
              </div>
            </div>
          </div>
        }
      </div>
    </div>
  `,
  styles: [`
    .cdk-drag-animating {
      transition: transform 250ms cubic-bezier(0, 0, 0.2, 1);
    }
    .cdk-drop-list-dragging .cdk-drag:not(.cdk-drag-placeholder) {
      transition: transform 250ms cubic-bezier(0, 0, 0.2, 1);
    }
  `],
})
export class PlaybookBuilderPage implements OnInit {
  private api = inject(PlaybooksApiService);
  private templateApi = inject(StepTemplatesApiService);
  private ctx = inject(ProjectContext);
  private router = inject(Router);
  private route = inject(ActivatedRoute);
  private platformId = inject(PLATFORM_ID);

  /** True on touch-primary devices — disables CDK drag to preserve mobile scrolling. */
  isTouch = signal(false);
  steps = signal<SpPlaybookStep[]>([]);
  saving = signal(false);
  templates = signal<SpStepTemplate[]>([]);
  templateNames = signal<Record<string, string>>({});
  showTemplatePicker = signal(false);
  savingTemplate = signal<number | null>(null);

  title = '';
  trigger = '';
  tags = '';
  initialState: 'ready' | 'backlog' = 'ready';
  gitStrategy: GitStrategyId = 'merge_to_default';
  gitTargetBranch = '';
  showGitStrategyInfo = false;
  editId: string | null = null;
  /** True when editing a shared default playbook (tenant_id = null). Saving will fork it. */
  isDefault = false;

  ngOnInit(): void {
    if (isPlatformBrowser(this.platformId)) {
      this.isTouch.set(window.matchMedia('(pointer: coarse)').matches);
    }
    this.editId = this.route.snapshot.paramMap.get('id');
    if (this.editId) {
      this.loadPlaybook(this.editId);
    } else {
      this.steps.set([{ name: '', description: '', on_complete: 'next', step: 0 }]);
    }
    this.loadTemplates();
  }

  loadPlaybook(id: string): void {
    this.api.get(id).subscribe({
      next: (pb) => {
        this.title = pb.title;
        this.trigger = pb.trigger_description;
        this.tags = pb.tags.join(', ');
        this.initialState = pb.initial_state;
        this.gitStrategy = (pb.metadata?.['git_strategy'] as GitStrategyId) || 'merge_to_default';
        this.gitTargetBranch = (pb.metadata?.['git_target_branch'] as string) || '';
        this.steps.set(pb.steps.map(s => ({ ...s })));
        this.isDefault = pb.tenant_id === null;
      },
    });
  }

  addStep(): void {
    const current = this.steps();
this.steps.set([
      ...current,
      {
        name: '',
        description: '',
        on_complete: current.length > 0 ? 'done' : 'next',
        step: current.length,
      },
    ]);
  }

  removeStep(index: number): void {
    const updated = [...this.steps()];
    updated.splice(index, 1);
    this.steps.set(updated);
  }

  moveStep(index: number, direction: number): void {
    const target = index + direction;
    const updated = [...this.steps()];
    if (target < 0 || target >= updated.length) return;
    [updated[index], updated[target]] = [updated[target], updated[index]];
    this.steps.set(updated);
  }

  dropStep(event: CdkDragDrop<SpPlaybookStep[]>): void {
    const updated = [...this.steps()];
    moveItemInArray(updated, event.previousIndex, event.currentIndex);
    this.steps.set(updated);
  }

  updateField(index: number, field: keyof SpPlaybookStep, value: unknown): void {
    const updated = [...this.steps()];
    updated[index] = { ...updated[index], [field]: value };
    this.steps.set(updated);
  }

  toJson(value: unknown): string {
    if (value == null) return '';
    return JSON.stringify(value, null, 2);
  }

  updateJsonField(index: number, field: keyof SpPlaybookStep, raw: string): void {
    const trimmed = raw.trim();
    if (!trimmed) {
      this.updateField(index, field, undefined);
      return;
    }
    try {
      this.updateField(index, field, JSON.parse(trimmed));
    } catch {
      // Don't update while JSON is invalid — user is still typing
    }
  }

  private buildMetadata(): Record<string, unknown> {
    const meta: Record<string, unknown> = { git_strategy: this.gitStrategy };
    if (this.gitStrategy === 'branch_to_target' && this.gitTargetBranch.trim()) {
      meta['git_target_branch'] = this.gitTargetBranch.trim();
    }
    return meta;
  }

  save(): void {
    this.saving.set(true);
    const tagList = this.tags.split(',').map(t => t.trim()).filter(t => t.length > 0);
    const stepsData = this.steps().map(s => ({
      ...s,
      name: s.name.trim().toLowerCase().replace(/\s+/g, '_') || `step`,
    }));
    const metadata = this.buildMetadata();

    if (this.editId) {
      this.api.update(this.editId, {
        title: this.title,
        trigger_description: this.trigger,
        tags: tagList,
        steps: stepsData,
        initial_state: this.initialState,
        metadata,
      }).subscribe({
        next: (saved) => {
          // If we forked a default, the API returns a new playbook with a different id.
          // Navigate to the list so the user sees their new copy.
          this.router.navigate(['/playbooks']);
          void saved;
        },
        error: () => this.saving.set(false),
      });
    } else {
      this.api.create({
        title: this.title,
        trigger_description: this.trigger,
        tags: tagList,
        steps: stepsData,
        initial_state: this.initialState,
        metadata,
      }).subscribe({
        next: () => this.router.navigate(['/playbooks']),
        error: () => this.saving.set(false),
      });
    }
  }

  // ── Template operations ──

  loadTemplates(): void {
    const pid = this.ctx.projectId();
    if (!pid) return;
    this.templateApi.list().subscribe({
      next: (list) => {
        this.templates.set(list);
        const names: Record<string, string> = {};
        for (const t of list) names[t.id] = t.name;
        this.templateNames.set(names);
      },
    });
  }

  toggleTemplatePicker(): void {
    if (!this.showTemplatePicker() && this.templates().length === 0) {
      this.loadTemplates();
    }
    this.showTemplatePicker.update(v => !v);
  }

  addStepFromTemplate(tpl: SpStepTemplate): void {
    const current = this.steps();
    const step: SpPlaybookStep = {
      name: tpl.name,
      description: tpl.description ?? '',
      on_complete: tpl.on_complete ?? (current.length > 0 ? 'done' : 'next'),
      step: current.length,
      step_template_id: tpl.id,
      model: tpl.model ?? undefined,
      budget: tpl.budget ?? undefined,
      allowed_tools: tpl.allowed_tools ?? undefined,
      context_level: tpl.context_level ?? undefined,
      max_cycles: tpl.max_cycles ?? undefined,
      retriable: tpl.retriable ?? undefined,
      timeout_minutes: tpl.timeout_minutes ?? undefined,
      mcp_servers: tpl.mcp_servers ?? undefined,
      agents: tpl.agents ?? undefined,
      agent: tpl.agent ?? undefined,
      settings: tpl.settings ?? undefined,
      env: tpl.env ?? undefined,
    };
    this.steps.set([...current, step]);
    this.showTemplatePicker.set(false);
    // Ensure template name is in our lookup
    this.templateNames.update(names => ({ ...names, [tpl.id]: tpl.name }));
  }

  detachTemplate(index: number): void {
    const updated = [...this.steps()];
    updated[index] = { ...updated[index] };
    delete updated[index].step_template_id;
    this.steps.set(updated);
  }

  saveAsTemplate(index: number): void {
    const pid = this.ctx.projectId();
    if (!pid) return;
    const step = this.steps()[index];
    this.savingTemplate.set(index);
    const data: SpCreateStepTemplate = {
      name: step.name || 'Untitled Step',
      description: step.description || undefined,
      model: step.model,
      budget: step.budget,
      allowed_tools: step.allowed_tools,
      context_level: step.context_level,
      on_complete: step.on_complete || undefined,
      retriable: step.retriable,
      max_cycles: step.max_cycles,
      timeout_minutes: step.timeout_minutes,
      mcp_servers: step.mcp_servers,
      agents: step.agents,
      agent: step.agent,
      settings: step.settings,
      env: step.env,
    };
    this.templateApi.create(data).subscribe({
      next: (created) => {
        this.savingTemplate.set(null);
        // Link the step to the new template
        const updated = [...this.steps()];
        updated[index] = { ...updated[index], step_template_id: created.id };
        this.steps.set(updated);
        // Update lookups
        this.templateNames.update(names => ({ ...names, [created.id]: created.name }));
        this.templates.update(list => [...list, created]);
      },
      error: () => this.savingTemplate.set(null),
    });
  }

  goBack(): void {
    this.router.navigate(['/playbooks']);
  }
}
