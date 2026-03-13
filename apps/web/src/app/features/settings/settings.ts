import { Component, inject, signal, effect, OnDestroy, OnInit, DestroyRef, HostListener } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { forkJoin, timer, switchMap } from 'rxjs';
import { DiraigentApiService, DgProject, DgProjectUpdate, DgPackage, DgGitMode } from '../../core/services/diraigent-api.service';
import { ProjectContext } from '../../core/services/project-context.service';
import { TeamApiService, SpRole, SpMember, SpRoleCreate, SpMemberCreate } from '../../core/services/team-api.service';
import { AgentsApiService, SpAgent, SpAgentRegistered, SpAgentTask } from '../../core/services/agents-api.service';
import { TasksApiService } from '../../core/services/tasks-api.service';
import { taskStateColor, taskTransitions, INTEGRATION_KIND_COLORS } from '../../shared/ui-constants';
import { IntegrationsApiService, Integration, IntegrationKind } from '../../core/services/integrations-api.service';
import { CreateAgentModalComponent } from '../agents/create-agent-modal';

type SettingsTab = 'general' | 'agents' | 'team' | 'integrations';

@Component({
  selector: 'app-settings',
  standalone: true,
  imports: [TranslocoModule, FormsModule, CreateAgentModalComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <h1 class="text-2xl font-semibold text-text-primary mb-3 sm:mb-6">{{ t('settings.title') }}</h1>

      @if (loading()) {
        <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
      } @else if (!project()) {
        <p class="text-text-secondary text-sm">{{ t('settings.noProject') }}</p>
      } @else {
        <!-- Tab bar -->
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center gap-1 bg-surface rounded-lg border border-border p-1">
            <button
              (click)="activeTab.set('general')"
              class="px-4 py-1.5 rounded-md text-sm font-medium transition-colors"
              [class.bg-bg]="activeTab() === 'general'"
              [class.text-text-primary]="activeTab() === 'general'"
              [class.shadow-sm]="activeTab() === 'general'"
              [class.text-text-muted]="activeTab() !== 'general'"
              [class.hover:text-text-secondary]="activeTab() !== 'general'">
              {{ t('settings.general') }}
            </button>
            <button
              (click)="activeTab.set('agents')"
              class="px-4 py-1.5 rounded-md text-sm font-medium transition-colors"
              [class.bg-bg]="activeTab() === 'agents'"
              [class.text-text-primary]="activeTab() === 'agents'"
              [class.shadow-sm]="activeTab() === 'agents'"
              [class.text-text-muted]="activeTab() !== 'agents'"
              [class.hover:text-text-secondary]="activeTab() !== 'agents'">
              {{ t('nav.agents') }}
            </button>
            <button
              (click)="activeTab.set('team')"
              class="px-4 py-1.5 rounded-md text-sm font-medium transition-colors"
              [class.bg-bg]="activeTab() === 'team'"
              [class.text-text-primary]="activeTab() === 'team'"
              [class.shadow-sm]="activeTab() === 'team'"
              [class.text-text-muted]="activeTab() !== 'team'"
              [class.hover:text-text-secondary]="activeTab() !== 'team'">
              {{ t('nav.team') }}
            </button>
            <button
              (click)="activeTab.set('integrations')"
              class="px-4 py-1.5 rounded-md text-sm font-medium transition-colors"
              [class.bg-bg]="activeTab() === 'integrations'"
              [class.text-text-primary]="activeTab() === 'integrations'"
              [class.shadow-sm]="activeTab() === 'integrations'"
              [class.text-text-muted]="activeTab() !== 'integrations'"
              [class.hover:text-text-secondary]="activeTab() !== 'integrations'">
              {{ t('nav.integrations') }}
            </button>
          </div>

          @if (activeTab() === 'agents') {
            <button (click)="showCreateModal.set(true)"
              class="px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg hover:opacity-90 transition-opacity">
              {{ t('agents.create') }}
            </button>
          } @else if (activeTab() === 'team') {
            <button (click)="showRoleForm()"
              class="px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg hover:opacity-90 transition-opacity">
              + {{ t('team.addRole') }}
            </button>
          } @else if (activeTab() === 'integrations') {
            <button (click)="navigateToNewIntegration()"
              class="px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg hover:opacity-90 transition-opacity">
              {{ t('integrations.create') }}
            </button>
          }
        </div>

        <!-- ── GENERAL TAB ── -->
        @if (activeTab() === 'general') {
          <div class="max-w-4xl">
            <!-- Project Settings -->
            <section class="mb-8">
              <h2 class="text-lg font-medium text-text-primary mb-4">{{ t('settings.general') }}</h2>
              <div class="bg-surface rounded-lg border border-border p-6 space-y-4">
                <!-- Name -->
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.name') }}</span>
                  <input type="text" [(ngModel)]="formName"
                    class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                </label>

                <!-- Slug (read-only) -->
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.slug') }}</span>
                  <input type="text" [value]="project()!.slug" disabled
                    class="w-full bg-bg-subtle text-text-secondary text-sm rounded-lg px-3 py-2 border border-border opacity-60" />
                </label>

                <!-- Description -->
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.description') }}</span>
                  <textarea [(ngModel)]="formDescription" rows="3"
                    class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
                </label>

                <!-- Repo URL -->
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.repoUrl') }}</span>
                  <input type="text" [(ngModel)]="formRepoUrl" [placeholder]="t('settings.repoUrlPlaceholder')"
                    class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                </label>

                <!-- Git Mode -->
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.gitMode') }}</span>
                  <select [(ngModel)]="formGitMode"
                    class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent">
                    <option value="standalone">{{ t('settings.gitModeStandalone') }}</option>
                    <option value="monorepo">{{ t('settings.gitModeMonorepo') }}</option>
                    <option value="none">{{ t('settings.gitModeNone') }}</option>
                  </select>
                </label>

                <!-- Git Root -->
                @if (formGitMode !== 'none') {
                  <label class="block">
                    <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.gitRoot') }}</span>
                    <input type="text" [(ngModel)]="formGitRoot"
                      [placeholder]="formGitMode === 'monorepo' ? t('settings.gitRootPlaceholderMonorepo') : t('settings.gitRootPlaceholderStandalone')"
                      class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent" />
                    <span class="block text-xs text-text-secondary mt-1">{{ t('settings.gitRootHint') }}</span>
                    <span class="block text-xs text-text-secondary mt-1">PROJECTS_PATH: <code class="font-mono bg-bg-subtle px-1 rounded">{{ projectsPath() ?? t('settings.notConfigured') }}</code></span>
                  </label>
                }

                <!-- Project Root -->
                @if (formGitMode === 'monorepo') {
                  <label class="block">
                    <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.projectRoot') }}</span>
                    <input type="text" [(ngModel)]="formProjectRoot"
                      [placeholder]="t('settings.projectRootPlaceholder')"
                      class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent" />
                    <span class="block text-xs text-text-secondary mt-1">{{ t('settings.projectRootHint') }}</span>
                  </label>
                }

                <!-- Auto Push -->
                @if (formGitMode !== 'none') {
                  <label class="flex items-center gap-3">
                    <input type="checkbox" [(ngModel)]="formAutoPush"
                      class="w-4 h-4 rounded border-border text-accent focus:ring-accent bg-bg-subtle" />
                    <span class="text-sm font-medium text-text-secondary">{{ t('settings.autoPush') }}</span>
                  </label>
                  <span class="block text-xs text-text-secondary mt-1 ml-7">{{ t('settings.autoPushHint') }}</span>
                }

                <!-- Resolved paths (read-only info) -->
                <div class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.resolvedPath') }}</span>
                  @if (project()!.resolved_path) {
                    <p class="text-xs font-mono text-text-secondary bg-bg-subtle rounded px-3 py-2 border border-border break-all">{{ project()!.resolved_path }}</p>
                  } @else {
                    <p class="text-xs text-text-secondary opacity-60 bg-bg-subtle rounded px-3 py-2 border border-border">{{ t('settings.notConfigured') }}</p>
                  }
                  <span class="block text-xs text-text-secondary mt-1">{{ t('settings.resolvedPathHint') }}</span>
                </div>

                @if (formGitMode !== 'none') {
                  <div class="block">
                    <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.gitResolvedPath') }}</span>
                    @if (project()!.git_resolved_path) {
                      <p class="text-xs font-mono text-text-secondary bg-bg-subtle rounded px-3 py-2 border border-border break-all">{{ project()!.git_resolved_path }}</p>
                    } @else {
                      <p class="text-xs text-text-secondary opacity-60 bg-bg-subtle rounded px-3 py-2 border border-border">{{ t('settings.notConfigured') }}</p>
                    }
                    <span class="block text-xs text-text-secondary mt-1">{{ t('settings.gitResolvedPathHint') }}</span>
                  </div>
                }

                <!-- Default Branch -->
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.defaultBranch') }}</span>
                  <input type="text" [(ngModel)]="formDefaultBranch"
                    class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                </label>

                <!-- Service Name -->
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.serviceName') }}</span>
                  <input type="text" [(ngModel)]="formServiceName" [placeholder]="t('settings.serviceNamePlaceholder')"
                    class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                  <span class="block text-xs text-text-secondary mt-1">{{ t('settings.serviceNameHint') }}</span>
                </label>

                <!-- Package -->
                <div class="block">
                  <label for="sett-package" class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.package') }}</label>
                  @if (loadingPackages()) {
                    <p class="text-xs text-text-secondary">{{ t('common.loading') }}</p>
                  } @else {
                    <select id="sett-package" [(ngModel)]="formPackageSlug"
                      class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent">
                      @for (pkg of packages(); track pkg.id) {
                        <option [value]="pkg.slug">{{ pkg.name }}</option>
                      }
                    </select>
                  }
                  <span class="block text-xs text-text-secondary mt-1">{{ t('settings.packageHint') }}</span>
                </div>

                <!-- Observation Retention -->
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('settings.observationRetentionDays') }}</span>
                  <input type="number" [(ngModel)]="formObservationRetentionDays" min="1" max="365"
                    class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                  <span class="block text-xs text-text-secondary mt-1">{{ t('settings.observationRetentionDaysHint') }}</span>
                </label>

                <!-- Save button -->
                <div class="flex items-center gap-3 pt-2">
                  <button (click)="saveProject()"
                    [disabled]="savingProject()"
                    class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50">
                    @if (savingProject()) {
                      {{ t('settings.saving') }}
                    } @else {
                      {{ t('settings.save') }}
                    }
                  </button>
                  @if (projectSaved()) {
                    <span class="text-sm text-ctp-green">{{ t('settings.saved') }}</span>
                  }
                </div>
              </div>
            </section>

            <!-- CLAUDE.md Editor -->
            <section class="mb-8">
              <div class="flex items-center justify-between mb-4">
                <h2 class="text-lg font-medium text-text-primary">CLAUDE.md</h2>
                @if (!claudeMdExists()) {
                  <span class="text-xs text-text-secondary bg-surface px-2 py-1 rounded">{{ t('settings.claudeMdNew') }}</span>
                }
              </div>
              <div class="bg-surface rounded-lg border border-border p-6 space-y-4">
                <p class="text-sm text-text-secondary">{{ t('settings.claudeMdDescription') }}</p>
                <textarea
                  [(ngModel)]="formClaudeMd"
                  rows="20"
                  class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent resize-y font-mono"
                  spellcheck="false"></textarea>

                <div class="flex items-center gap-3">
                  <button (click)="saveClaudeMd()"
                    [disabled]="savingClaudeMd()"
                    class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50">
                    @if (savingClaudeMd()) {
                      {{ t('settings.saving') }}
                    } @else {
                      {{ t('settings.save') }}
                    }
                  </button>
                  @if (claudeMdSaved()) {
                    <span class="text-sm text-ctp-green">{{ t('settings.saved') }}</span>
                  }
                </div>
              </div>
            </section>

            <!-- Danger Zone -->
            <section>
              <h2 class="text-lg font-medium text-ctp-red mb-4">{{ t('settings.dangerZone') }}</h2>
              <div class="bg-surface rounded-lg border border-ctp-red/30 p-6">
                <div class="flex items-start justify-between gap-4">
                  <div>
                    <p class="text-sm font-medium text-text-primary">{{ t('settings.deleteProject') }}</p>
                    <p class="text-sm text-text-secondary mt-1">{{ t('settings.deleteProjectHint') }}</p>
                  </div>
                  @if (!confirmingDelete()) {
                    <button (click)="startDeleteConfirm()"
                      class="shrink-0 px-4 py-2 bg-ctp-red text-white rounded-lg text-sm font-medium hover:opacity-90">
                      {{ t('settings.deleteProject') }}
                    </button>
                  }
                </div>

                @if (confirmingDelete()) {
                  <div class="mt-4 pt-4 border-t border-ctp-red/30">
                    <p class="text-sm text-ctp-red font-medium mb-3">{{ t('settings.deleteConfirm') }}</p>
                    <div class="flex items-center gap-3">
                      <button (click)="deleteProject()"
                        [disabled]="deletingProject()"
                        class="px-4 py-2 bg-ctp-red text-white rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50">
                        @if (deletingProject()) {
                          {{ t('settings.deleting') }}
                        } @else {
                          {{ t('settings.deleteYes') }}
                        }
                      </button>
                      <button (click)="cancelDelete()"
                        [disabled]="deletingProject()"
                        class="px-4 py-2 bg-surface text-text-secondary rounded-lg text-sm font-medium border border-border hover:border-accent disabled:opacity-50">
                        {{ t('settings.deleteNo') }}
                      </button>
                    </div>
                  </div>
                }
              </div>
            </section>
          </div>
        }

        <!-- ── AGENTS TAB ── -->
        @if (activeTab() === 'agents') {
          @if (teamLoading()) {
            <p class="text-text-secondary">{{ t('common.loading') }}</p>
          } @else if (teamError()) {
            <p class="text-error">{{ t('common.error') }}</p>
          } @else {
            <div class="flex flex-col lg:flex-row gap-4 lg:gap-6">
              <!-- Agent list -->
              <div class="flex-1 min-w-0">
                <div class="bg-surface rounded-lg border border-border overflow-hidden">
                  <table class="w-full text-sm">
                    <thead>
                      <tr class="border-b border-border text-text-secondary text-left">
                        <th class="px-4 py-3 font-medium">{{ t('agents.name') }}</th>
                        <th class="px-4 py-3 font-medium">{{ t('agents.status') }}</th>
                        <th class="px-4 py-3 font-medium">{{ t('team.role') }}</th>
                        <th class="px-4 py-3 font-medium">{{ t('agents.capabilities') }}</th>
                        <th class="px-4 py-3 font-medium">{{ t('agents.lastSeen') }}</th>
                      </tr>
                    </thead>
                    <tbody>
                      @for (agent of agents(); track agent.id) {
                        <tr class="border-b border-border last:border-b-0 cursor-pointer transition-colors"
                            [class.bg-surface-hover]="selectedAgent()?.id === agent.id"
                            (click)="selectAgent(agent)">
                          <td class="px-4 py-3 text-text-primary font-medium">{{ agent.name }}</td>
                          <td class="px-4 py-3">
                            <span class="inline-flex items-center gap-1.5">
                              <span class="w-2 h-2 rounded-full"
                                    [class.bg-ctp-green]="agent.status === 'idle'"
                                    [class.bg-ctp-yellow]="agent.status === 'working'"
                                    [class.bg-ctp-red]="agent.status === 'offline'"
                                    [class.bg-ctp-maroon]="agent.status === 'revoked'"></span>
                              <span class="text-text-secondary">{{ agent.status }}</span>
                            </span>
                          </td>
                          <td class="px-4 py-3 text-text-secondary text-xs">
                            {{ agentRoleName(agent.id) }}
                          </td>
                          <td class="px-4 py-3">
                            <div class="flex flex-wrap gap-1">
                              @for (cap of agent.capabilities.slice(0, 3); track cap) {
                                <span class="px-1.5 py-0.5 text-xs rounded bg-bg-subtle text-text-secondary">{{ cap }}</span>
                              }
                              @if (agent.capabilities.length > 3) {
                                <span class="px-1.5 py-0.5 text-xs rounded bg-bg-subtle text-text-muted">+{{ agent.capabilities.length - 3 }}</span>
                              }
                            </div>
                          </td>
                          <td class="px-4 py-3 text-text-muted text-xs">{{ formatTime(agent.last_seen_at) }}</td>
                        </tr>
                      } @empty {
                        <tr>
                          <td colspan="5" class="px-4 py-8 text-center text-text-muted">{{ t('common.empty') }}</td>
                        </tr>
                      }
                    </tbody>
                  </table>
                </div>
              </div>

              <!-- Agent detail panel -->
              @if (selectedAgent(); as agent) {
                <div class="w-80 shrink-0">
                  <div class="bg-surface rounded-lg border border-border p-4">
                    <div class="flex items-center justify-between mb-4">
                      <h2 class="text-lg font-semibold text-text-primary">{{ agent.name }}</h2>
                      <button (click)="selectAgent(null)" class="text-text-muted hover:text-text-secondary text-sm">✕</button>
                    </div>

                    <!-- Status -->
                    <div class="mb-4">
                      <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('agents.status') }}</span>
                      <div class="mt-1 flex items-center gap-2">
                        <span class="w-2.5 h-2.5 rounded-full"
                              [class.bg-ctp-green]="agent.status === 'idle'"
                              [class.bg-ctp-yellow]="agent.status === 'working'"
                              [class.bg-ctp-red]="agent.status === 'offline'"
                              [class.bg-ctp-maroon]="agent.status === 'revoked'"></span>
                        <span class="text-text-primary capitalize">{{ agent.status }}</span>
                      </div>
                    </div>

                    <!-- Role membership -->
                    @if (agentRoleName(agent.id)) {
                      <div class="mb-4">
                        <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('team.role') }}</span>
                        <p class="mt-1 text-text-secondary text-sm">{{ agentRoleName(agent.id) }}</p>
                      </div>
                    }

                    <!-- Last seen -->
                    @if (agent.last_seen_at) {
                      <div class="mb-4">
                        <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('agents.lastSeen') }}</span>
                        <p class="mt-1 text-text-secondary text-sm">{{ formatTime(agent.last_seen_at) }}</p>
                      </div>
                    }

                    <!-- Capabilities -->
                    <div class="mb-4">
                      <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('agents.capabilities') }}</span>
                      <div class="mt-1 flex flex-wrap gap-1">
                        @for (cap of agent.capabilities; track cap) {
                          <span class="px-2 py-0.5 text-xs rounded bg-bg-subtle text-text-secondary">{{ cap }}</span>
                        }
                        @if (agent.capabilities.length === 0) {
                          <span class="text-text-muted text-sm">—</span>
                        }
                      </div>
                    </div>

                    <!-- Metadata -->
                    @if (hasMetadata(agent)) {
                      <div class="mb-4">
                        <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('agents.metadata') }}</span>
                        <div class="mt-1 space-y-1">
                          @for (entry of metadataEntries(agent); track entry[0]) {
                            <div class="flex justify-between text-sm">
                              <span class="text-text-muted">{{ entry[0] }}</span>
                              <span class="text-text-secondary">{{ entry[1] }}</span>
                            </div>
                          }
                        </div>
                      </div>
                    }

                    <!-- Revoke -->
                    @if (agent.status !== 'revoked') {
                      <div class="mb-4">
                        <button (click)="revokeAgent(agent)"
                          class="w-full px-3 py-1.5 text-xs font-medium text-ctp-red border border-ctp-red/30 rounded-lg
                                 hover:bg-ctp-red/10 transition-colors">
                          {{ t('agents.revoke') }}
                        </button>
                      </div>
                    }

                    <!-- Tasks -->
                    <div>
                      <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('agents.assignedTasks') }}</span>
                      @if (tasksLoading()) {
                        <p class="mt-1 text-text-muted text-sm">{{ t('common.loading') }}</p>
                      } @else {
                        <div class="mt-1 space-y-1.5">
                          @for (task of agentTasks(); track task.id) {
                            <div class="p-2 rounded bg-bg-subtle text-sm">
                              <div class="text-text-primary">{{ task.title }}</div>
                              <div class="flex items-center gap-2 mt-0.5">
                                <div class="relative">
                                  <button class="text-xs px-1.5 py-0.5 rounded-full font-medium cursor-pointer hover:ring-1 hover:ring-accent {{ stateColor(task.state) }}"
                                          (click)="toggleStateMenu($event, task.id)">
                                    {{ task.state }}
                                  </button>
                                  @if (openMenuId() === task.id) {
                                    <div class="absolute z-50 mt-1 left-0 bg-surface border border-border rounded-lg shadow-lg py-1 min-w-[120px]">
                                      @for (target of getTransitions(task.state); track target) {
                                        <button (click)="onTransition($event, task, target)"
                                          class="w-full text-left px-3 py-1.5 text-xs hover:bg-surface-hover transition-colors flex items-center gap-2 cursor-pointer">
                                          <span class="w-2 h-2 rounded-full {{ stateColor(target) }}"></span>
                                          <span class="text-text-primary">{{ target }}</span>
                                        </button>
                                      } @empty {
                                        <span class="px-3 py-1.5 text-xs text-text-muted block">No transitions</span>
                                      }
                                    </div>
                                  }
                                </div>
                                <span class="text-xs text-text-muted">{{ task.kind }}</span>
                              </div>
                            </div>
                          } @empty {
                            <p class="text-text-muted text-sm">{{ t('agents.noTasks') }}</p>
                          }
                        </div>
                      }
                    </div>
                  </div>
                </div>
              }
            </div>
          }
        }

        <!-- ── INTEGRATIONS TAB ── -->
        @if (activeTab() === 'integrations') {
          @if (integrationsLoading()) {
            <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
          } @else if (integrationsError()) {
            <p class="text-ctp-red text-sm">{{ t('common.error') }}</p>
          } @else if (integrations().length === 0) {
            <div class="text-center py-12">
              <svg class="w-12 h-12 mx-auto text-text-secondary mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
                      d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1"/>
              </svg>
              <p class="text-text-secondary mb-4">{{ t('integrations.empty') }}</p>
              <button (click)="navigateToNewIntegration()"
                class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 transition-opacity">
                {{ t('integrations.create') }}
              </button>
            </div>
          } @else {
            <div class="grid gap-4 max-w-4xl">
              @for (integration of integrations(); track integration.id) {
                <div class="bg-surface border border-border rounded-lg p-4 hover:border-accent/50 transition-colors cursor-pointer"
                     role="button" tabindex="0"
                     (click)="navigateToIntegration(integration.id)"
                     (keydown.enter)="navigateToIntegration(integration.id)">
                  <div class="flex items-center justify-between">
                    <div class="flex items-center gap-3">
                      <div class="w-10 h-10 rounded-lg bg-bg-subtle flex items-center justify-center text-text-secondary">
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1"/>
                        </svg>
                      </div>
                      <div>
                        <div class="flex items-center gap-2">
                          <span class="font-medium text-text-primary">{{ integration.name }}</span>
                          <span class="text-xs px-2 py-0.5 rounded-full {{ integrationKindColor(integration.kind) }}">
                            {{ integration.kind }}
                          </span>
                        </div>
                        <p class="text-sm text-text-secondary">{{ integration.provider }} · {{ integration.base_url }}</p>
                      </div>
                    </div>
                    <div class="flex items-center gap-3">
                      <span class="text-xs px-2 py-1 rounded-full"
                            [class]="integration.enabled ? 'bg-ctp-green/20 text-ctp-green' : 'bg-ctp-overlay0/20 text-ctp-overlay0'">
                        {{ integration.enabled ? t('integrations.enabled') : t('integrations.disabled') }}
                      </span>
                      <button (click)="toggleIntegrationEnabled($event, integration)"
                              class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors"
                              [class]="integration.enabled ? 'bg-accent' : 'bg-ctp-overlay0'"
                              [attr.aria-label]="integration.enabled ? t('integrations.disable') : t('integrations.enable')">
                        <span class="inline-block h-4 w-4 rounded-full bg-white transition-transform"
                              [class]="integration.enabled ? 'translate-x-6' : 'translate-x-1'"></span>
                      </button>
                    </div>
                  </div>
                  @if (integration.capabilities.length > 0) {
                    <div class="mt-2 flex flex-wrap gap-1">
                      @for (cap of integration.capabilities; track cap) {
                        <span class="text-xs px-1.5 py-0.5 rounded bg-bg-subtle text-text-secondary">{{ cap }}</span>
                      }
                    </div>
                  }
                </div>
              }
            </div>
          }
        }

        <!-- ── TEAM TAB ── -->
        @if (activeTab() === 'team') {
          @if (teamLoading()) {
            <p class="text-text-secondary">{{ t('common.loading') }}</p>
          } @else if (teamError()) {
            <p class="text-error">{{ t('common.error') }}</p>
          } @else {
            <div class="flex gap-4 h-[calc(100vh-12rem)]">
              <!-- Panel 1: Roles -->
              <div class="w-64 shrink-0 flex flex-col bg-surface rounded-lg border border-border">
                <div class="px-4 py-3 border-b border-border">
                  <span class="text-sm font-semibold text-text-primary">{{ t('team.roles') }}</span>
                </div>
                <div class="flex-1 overflow-y-auto">
                  @for (role of roles(); track role.id) {
                    <div class="px-4 py-2.5 cursor-pointer border-b border-border last:border-b-0 transition-colors"
                         [class.bg-surface-hover]="selectedRole()?.id === role.id"
                         tabindex="0" role="button"
                         (click)="selectRole(role)" (keydown.enter)="selectRole(role)">
                      <div class="text-sm text-text-primary font-medium">{{ role.name }}</div>
                      <div class="text-xs text-text-muted mt-0.5">
                        {{ membersForRole(role.id).length }} {{ t('team.members') }} · {{ role.authorities.length }} {{ t('team.authorities') }}
                      </div>
                    </div>
                  } @empty {
                    <div class="px-4 py-6 text-center text-text-muted text-sm">{{ t('common.empty') }}</div>
                  }
                </div>
              </div>

              <!-- Panel 2: Members -->
              <div class="w-72 shrink-0 flex flex-col bg-surface rounded-lg border border-border">
                <div class="px-4 py-3 border-b border-border flex items-center justify-between">
                  <span class="text-sm font-semibold text-text-primary">{{ t('team.members') }}</span>
                  @if (selectedRole()) {
                    <button (click)="showMemberForm()" class="text-accent hover:text-interactive text-sm">+ {{ t('team.addMember') }}</button>
                  }
                </div>
                <div class="flex-1 overflow-y-auto">
                  @if (!selectedRole()) {
                    <div class="px-4 py-6 text-center text-text-muted text-sm">{{ t('team.selectRole') }}</div>
                  } @else {
                    @for (member of filteredMembers(); track member.id) {
                      <div class="px-4 py-2.5 cursor-pointer border-b border-border last:border-b-0 transition-colors"
                           [class.bg-surface-hover]="selectedMember()?.id === member.id"
                           tabindex="0" role="button"
                           (click)="selectMember(member)" (keydown.enter)="selectMember(member)">
                        <div class="text-sm text-text-primary font-medium">{{ agentName(member.agent_id) }}</div>
                        <div class="flex items-center gap-1.5 mt-0.5">
                          <span class="w-1.5 h-1.5 rounded-full"
                                [class.bg-ctp-green]="agentStatus(member.agent_id) === 'idle'"
                                [class.bg-ctp-yellow]="agentStatus(member.agent_id) === 'working'"
                                [class.bg-ctp-red]="agentStatus(member.agent_id) === 'offline'"
                                [class.bg-ctp-maroon]="agentStatus(member.agent_id) === 'revoked'"></span>
                          <span class="text-xs text-text-muted">{{ agentStatus(member.agent_id) }}</span>
                        </div>
                      </div>
                    } @empty {
                      <div class="px-4 py-6 text-center text-text-muted text-sm">{{ t('team.noMembers') }}</div>
                    }
                  }
                </div>
              </div>

              <!-- Panel 3: Detail -->
              <div class="flex-1 min-w-0 bg-surface rounded-lg border border-border overflow-y-auto">
                <!-- Role form -->
                @if (editingRole()) {
                  <div class="p-4">
                    <h3 class="text-lg font-semibold text-text-primary mb-4">
                      {{ roleFormId() ? t('team.editRole') : t('team.createRole') }}
                    </h3>
                    <div class="space-y-3">
                      <div>
                        <label for="team-role-name" class="block text-xs text-text-muted mb-1">{{ t('team.roleName') }}</label>
                        <input id="team-role-name" [ngModel]="roleForm().name" (ngModelChange)="updateRoleForm('name', $event)"
                               class="w-full px-3 py-1.5 rounded bg-bg text-text-primary border border-border text-sm
                                      focus:outline-none focus:ring-1 focus:ring-accent" />
                      </div>
                      <div>
                        <label for="team-role-desc" class="block text-xs text-text-muted mb-1">{{ t('team.description') }}</label>
                        <input id="team-role-desc" [ngModel]="roleForm().description" (ngModelChange)="updateRoleForm('description', $event)"
                               class="w-full px-3 py-1.5 rounded bg-bg text-text-primary border border-border text-sm
                                      focus:outline-none focus:ring-1 focus:ring-accent" />
                      </div>
                      <div>
                        <span class="block text-xs text-text-muted mb-1">{{ t('team.authorities') }}</span>
                        <div class="flex flex-wrap gap-1.5">
                          @for (auth of allAuthorities; track auth) {
                            <label class="inline-flex items-center gap-1 text-sm text-text-secondary cursor-pointer">
                              <input type="checkbox" [checked]="roleForm().authorities.includes(auth)"
                                     (change)="toggleAuthority(auth)"
                                     class="rounded border-border text-accent focus:ring-accent" />
                              {{ auth }}
                            </label>
                          }
                        </div>
                      </div>
                      <div class="flex gap-2 pt-2">
                        <button (click)="saveRole()"
                                class="px-3 py-1.5 rounded bg-accent text-white text-sm hover:opacity-90 transition-opacity">
                          {{ t('team.save') }}
                        </button>
                        <button (click)="cancelRoleForm()"
                                class="px-3 py-1.5 rounded bg-surface-hover text-text-secondary text-sm hover:text-text-primary transition-colors">
                          {{ t('team.cancel') }}
                        </button>
                        @if (roleFormId()) {
                          <button (click)="deleteRole()"
                                  class="ml-auto px-3 py-1.5 rounded text-error text-sm hover:bg-error hover:text-white transition-colors">
                            {{ t('team.delete') }}
                          </button>
                        }
                      </div>
                    </div>
                  </div>
                }
                <!-- Member form -->
                @else if (addingMember()) {
                  <div class="p-4">
                    <h3 class="text-lg font-semibold text-text-primary mb-4">{{ t('team.addMember') }}</h3>
                    <div class="space-y-3">
                      <div>
                        <label for="team-agent" class="block text-xs text-text-muted mb-1">{{ t('team.selectAgent') }}</label>
                        <select id="team-agent" [ngModel]="memberAgentId()" (ngModelChange)="memberAgentId.set($event)"
                                class="w-full px-3 py-1.5 rounded bg-bg text-text-primary border border-border text-sm
                                       focus:outline-none focus:ring-1 focus:ring-accent">
                          <option value="">—</option>
                          @for (agent of availableAgents(); track agent.id) {
                            <option [value]="agent.id">{{ agent.name }}</option>
                          }
                        </select>
                      </div>
                      <div class="flex gap-2 pt-2">
                        <button (click)="saveMember()"
                                [disabled]="!memberAgentId()"
                                class="px-3 py-1.5 rounded bg-accent text-white text-sm hover:opacity-90 transition-opacity disabled:opacity-50">
                          {{ t('team.save') }}
                        </button>
                        <button (click)="addingMember.set(false)"
                                class="px-3 py-1.5 rounded bg-surface-hover text-text-secondary text-sm hover:text-text-primary transition-colors">
                          {{ t('team.cancel') }}
                        </button>
                      </div>
                    </div>
                  </div>
                }
                <!-- Member detail -->
                @else if (selectedMember(); as member) {
                  <div class="p-4">
                    <div class="flex items-center justify-between mb-4">
                      <h3 class="text-lg font-semibold text-text-primary">{{ agentName(member.agent_id) }}</h3>
                      <button (click)="removeMember(member)"
                              class="text-error text-sm hover:underline">{{ t('team.remove') }}</button>
                    </div>

                    <div class="space-y-3">
                      <div>
                        <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('team.role') }}</span>
                        <p class="mt-0.5 text-text-primary text-sm">{{ roleName(member.role_id) }}</p>
                      </div>
                      <div>
                        <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('agents.status') }}</span>
                        <div class="mt-0.5 flex items-center gap-2">
                          <span class="w-2 h-2 rounded-full"
                                [class.bg-ctp-green]="agentStatus(member.agent_id) === 'idle'"
                                [class.bg-ctp-yellow]="agentStatus(member.agent_id) === 'working'"
                                [class.bg-ctp-red]="agentStatus(member.agent_id) === 'offline'"
                                [class.bg-ctp-maroon]="agentStatus(member.agent_id) === 'revoked'"></span>
                          <span class="text-text-secondary text-sm capitalize">{{ agentStatus(member.agent_id) }}</span>
                        </div>
                      </div>
                      <div>
                        <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('team.joined') }}</span>
                        <p class="mt-0.5 text-text-secondary text-sm">{{ formatDate(member.joined_at) }}</p>
                      </div>

                      <!-- Change role -->
                      <div>
                        <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('team.changeRole') }}</span>
                        <div class="mt-1 flex items-center gap-2">
                          <select [ngModel]="member.role_id" (ngModelChange)="changeMemberRole(member, $event)"
                                  class="flex-1 px-3 py-1.5 rounded bg-bg text-text-primary border border-border text-sm
                                         focus:outline-none focus:ring-1 focus:ring-accent">
                            @for (role of roles(); track role.id) {
                              <option [value]="role.id">{{ role.name }}</option>
                            }
                          </select>
                        </div>
                      </div>
                    </div>
                  </div>
                }
                <!-- Selected role detail -->
                @else if (selectedRole(); as role) {
                  <div class="p-4">
                    <div class="flex items-center justify-between mb-4">
                      <h3 class="text-lg font-semibold text-text-primary">{{ role.name }}</h3>
                      <button (click)="editRole(role)" class="text-accent text-sm hover:underline">{{ t('team.edit') }}</button>
                    </div>
                    @if (role.description) {
                      <p class="text-text-secondary text-sm mb-4">{{ role.description }}</p>
                    }
                    <div class="mb-3">
                      <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('team.authorities') }}</span>
                      <div class="mt-1 flex flex-wrap gap-1">
                        @for (auth of role.authorities; track auth) {
                          <span class="px-2 py-0.5 text-xs rounded bg-bg-subtle text-text-secondary">{{ auth }}</span>
                        }
                      </div>
                    </div>
                    @if (role.knowledge_scope.length > 0) {
                      <div class="mb-3">
                        <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('team.knowledgeScope') }}</span>
                        <div class="mt-1 flex flex-wrap gap-1">
                          @for (scope of role.knowledge_scope; track scope) {
                            <span class="px-2 py-0.5 text-xs rounded bg-bg-subtle text-text-secondary">{{ scope }}</span>
                          }
                        </div>
                      </div>
                    }
                    <!-- Members in this role -->
                    <div>
                      <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('team.members') }}</span>
                      <div class="mt-2 space-y-1.5">
                        @for (member of membersForRole(role.id); track member.id) {
                          <div class="flex items-center gap-2 p-2 rounded bg-bg-subtle">
                            <span class="w-1.5 h-1.5 rounded-full shrink-0"
                                  [class.bg-ctp-green]="agentStatus(member.agent_id) === 'idle'"
                                  [class.bg-ctp-yellow]="agentStatus(member.agent_id) === 'working'"
                                  [class.bg-ctp-red]="agentStatus(member.agent_id) === 'offline'"
                                  [class.bg-ctp-maroon]="agentStatus(member.agent_id) === 'revoked'"></span>
                            <span class="text-sm text-text-primary">{{ agentName(member.agent_id) }}</span>
                            <span class="ml-auto text-xs text-text-muted">{{ agentStatus(member.agent_id) }}</span>
                          </div>
                        } @empty {
                          <p class="text-text-muted text-sm">{{ t('team.noMembers') }}</p>
                        }
                      </div>
                    </div>
                  </div>
                } @else {
                  <div class="flex items-center justify-center h-full text-text-muted text-sm">
                    {{ t('team.selectRoleOrMember') }}
                  </div>
                }
              </div>
            </div>
          }
        }
      }

      @if (showCreateModal()) {
        <app-create-agent-modal
          (created)="onAgentCreated($event); showCreateModal.set(false)"
          (cancelled)="showCreateModal.set(false)" />
      }
    </div>
  `,
})
export class SettingsPage implements OnInit, OnDestroy {
  private api = inject(DiraigentApiService);
  private ctx = inject(ProjectContext);
  private router = inject(Router);
  private teamApi = inject(TeamApiService);
  private agentsApi = inject(AgentsApiService);
  private tasksApi = inject(TasksApiService);
  private integrationsApi = inject(IntegrationsApiService);
  private destroyRef = inject(DestroyRef);

  // Tab state
  activeTab = signal<SettingsTab>('general');

  // General tab state
  loading = signal(true);
  project = signal<DgProject | null>(null);
  packages = signal<DgPackage[]>([]);
  loadingPackages = signal(false);
  projectsPath = signal<string | null>(null);

  // Project form fields
  formName = '';
  formDescription = '';
  formRepoUrl = '';
  formDefaultBranch = 'main';
  formServiceName = '';
  formPackageSlug = 'software-dev';
  formGitMode: DgGitMode = 'standalone';
  formGitRoot = '';
  formProjectRoot = '';
  formAutoPush = true;
  formUploadLogs = false;
  formDoneRetentionDays = 1;
  formObservationRetentionDays = 30;
  savingProject = signal(false);
  projectSaved = signal(false);

  // CLAUDE.md
  formClaudeMd = '';
  claudeMdExists = signal(false);
  savingClaudeMd = signal(false);
  claudeMdSaved = signal(false);

  // Delete project
  confirmingDelete = signal(false);
  deletingProject = signal(false);

  // Team/Agent shared state
  readonly allAuthorities = ['execute', 'delegate', 'review', 'create', 'decide', 'manage'];
  roles = signal<SpRole[]>([]);
  members = signal<SpMember[]>([]);
  agents = signal<SpAgent[]>([]);
  teamLoading = signal(true);
  teamError = signal(false);

  // Agent tab state
  selectedAgent = signal<SpAgent | null>(null);
  agentTasks = signal<SpAgentTask[]>([]);
  tasksLoading = signal(false);
  openMenuId = signal<string | null>(null);
  showCreateModal = signal(false);

  // Team tab state
  selectedRole = signal<SpRole | null>(null);
  selectedMember = signal<SpMember | null>(null);
  editingRole = signal(false);
  roleFormId = signal<string | null>(null);
  roleForm = signal<SpRoleCreate>({ name: '', authorities: [] });
  addingMember = signal(false);
  memberAgentId = signal('');

  // Integrations tab state
  integrations = signal<Integration[]>([]);
  integrationsLoading = signal(false);
  integrationsError = signal(false);

  private savedTimer: ReturnType<typeof setTimeout> | null = null;
  private claudeSavedTimer: ReturnType<typeof setTimeout> | null = null;

  private loadEffect = effect(() => {
    const pid = this.ctx.projectId();
    if (pid) {
      this.loadProject(pid);
      this.loadClaudeMd(pid);
      this.loadPackages();
      this.loadSettings();
      this.loadIntegrations(pid);
    } else {
      this.loading.set(false);
      this.project.set(null);
    }
  });

  @HostListener('document:click')
  onDocumentClick(): void {
    this.openMenuId.set(null);
  }

  ngOnInit(): void {
    // Poll agents every 10s
    timer(0, 10_000)
      .pipe(
        switchMap(() => this.agentsApi.getAgents()),
        takeUntilDestroyed(this.destroyRef),
      )
      .subscribe({
        next: (agents) => {
          this.agents.set(agents);
          // Update selected agent if it still exists
          const sel = this.selectedAgent();
          if (sel) {
            const updated = agents.find(a => a.id === sel.id);
            if (updated) this.selectedAgent.set(updated);
            else this.selectedAgent.set(null);
          }
        },
      });

    // Load team data
    this.loadTeamData();
  }

  ngOnDestroy(): void {
    if (this.savedTimer) clearTimeout(this.savedTimer);
    if (this.claudeSavedTimer) clearTimeout(this.claudeSavedTimer);
  }

  // ── General tab methods ──

  private loadProject(projectId: string): void {
    this.loading.set(true);
    this.api.getProject(projectId).subscribe({
      next: p => {
        this.project.set(p);
        this.formName = p.name;
        this.formDescription = p.description ?? '';
        this.formRepoUrl = p.repo_url ?? '';
        this.formDefaultBranch = p.default_branch ?? 'main';
        this.formServiceName = p.service_name ?? '';
        this.formPackageSlug = p.package?.slug ?? 'software-dev';
        this.formGitMode = p.git_mode ?? 'standalone';
        this.formGitRoot = p.git_root ?? '';
        this.formProjectRoot = p.project_root ?? '';
        this.formAutoPush = (p.metadata?.['auto_push'] as boolean) ?? false;
        this.formUploadLogs = (p.metadata?.['upload_logs'] as boolean) ?? false;
        this.formDoneRetentionDays = (p.metadata?.['done_retention_days'] as number) ?? 1;
        this.formObservationRetentionDays = (p.metadata?.['observation_retention_days'] as number) ?? 30;
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });
  }

  private loadPackages(): void {
    this.loadingPackages.set(true);
    this.api.getPackages().subscribe({
      next: pkgs => {
        this.packages.set(pkgs);
        this.loadingPackages.set(false);
      },
      error: () => this.loadingPackages.set(false),
    });
  }

  private loadSettings(): void {
    this.api.getSettings().subscribe({
      next: settings => this.projectsPath.set(settings.projects_path),
      error: () => { /* settings fetch is best-effort */ },
    });
  }

  private loadClaudeMd(projectId: string): void {
    this.api.getClaudeMd(projectId).subscribe({
      next: res => {
        this.formClaudeMd = res.content;
        this.claudeMdExists.set(res.exists);
      },
      error: () => {
        this.formClaudeMd = '';
        this.claudeMdExists.set(false);
      },
    });
  }

  saveProject(): void {
    const pid = this.ctx.projectId();
    if (!pid) return;

    this.savingProject.set(true);
    this.projectSaved.set(false);

    const update: DgProjectUpdate = {
      name: this.formName,
      description: this.formDescription,
      repo_url: this.formRepoUrl || null,
      default_branch: this.formDefaultBranch,
      service_name: this.formServiceName || null,
      package_slug: this.formPackageSlug || null,
      git_mode: this.formGitMode,
      git_root: this.formGitRoot || null,
      project_root: this.formProjectRoot || null,
      metadata: { ...(this.project()?.metadata || {}), auto_push: this.formAutoPush, upload_logs: this.formUploadLogs, done_retention_days: this.formDoneRetentionDays, observation_retention_days: this.formObservationRetentionDays },
    };

    this.api.updateProject(pid, update).subscribe({
      next: p => {
        this.project.set(p);
        this.savingProject.set(false);
        this.projectSaved.set(true);
        if (this.savedTimer) clearTimeout(this.savedTimer);
        this.savedTimer = setTimeout(() => this.projectSaved.set(false), 3000);
      },
      error: () => this.savingProject.set(false),
    });
  }

  saveClaudeMd(): void {
    const pid = this.ctx.projectId();
    if (!pid) return;

    this.savingClaudeMd.set(true);
    this.claudeMdSaved.set(false);

    this.api.updateClaudeMd(pid, this.formClaudeMd).subscribe({
      next: res => {
        this.claudeMdExists.set(res.exists);
        this.savingClaudeMd.set(false);
        this.claudeMdSaved.set(true);
        if (this.claudeSavedTimer) clearTimeout(this.claudeSavedTimer);
        this.claudeSavedTimer = setTimeout(() => this.claudeMdSaved.set(false), 3000);
      },
      error: () => this.savingClaudeMd.set(false),
    });
  }

  startDeleteConfirm(): void {
    this.confirmingDelete.set(true);
  }

  cancelDelete(): void {
    this.confirmingDelete.set(false);
  }

  deleteProject(): void {
    const pid = this.ctx.projectId();
    if (!pid) return;

    this.deletingProject.set(true);
    this.api.deleteProject(pid).subscribe({
      next: () => {
        this.ctx.clear();
        this.router.navigate(['/']);
      },
      error: () => this.deletingProject.set(false),
    });
  }

  // ── Integration methods ──

  private loadIntegrations(projectId: string): void {
    this.integrationsLoading.set(true);
    this.integrationsError.set(false);
    this.integrationsApi.list(projectId).subscribe({
      next: (data) => {
        this.integrations.set(data);
        this.integrationsLoading.set(false);
      },
      error: () => {
        this.integrationsError.set(true);
        this.integrationsLoading.set(false);
      },
    });
  }

  toggleIntegrationEnabled(event: Event, integration: Integration): void {
    event.stopPropagation();
    const newEnabled = !integration.enabled;
    this.integrationsApi.update(integration.id, { enabled: newEnabled }).subscribe({
      next: (updated) => {
        this.integrations.update(list =>
          list.map(i => (i.id === updated.id ? updated : i)),
        );
      },
    });
  }

  integrationKindColor(kind: IntegrationKind): string {
    return INTEGRATION_KIND_COLORS[kind] ?? INTEGRATION_KIND_COLORS['custom'];
  }

  navigateToIntegration(id: string): void {
    this.router.navigate(['/integrations', id]);
  }

  navigateToNewIntegration(): void {
    this.router.navigate(['/integrations', 'new']);
  }

  // ── Team data loading ──

  private loadTeamData(): void {
    forkJoin({
      roles: this.teamApi.getRoles(),
      members: this.teamApi.getMembers(),
    }).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
      next: ({ roles, members }) => {
        this.roles.set(roles);
        this.members.set(members);
        this.teamLoading.set(false);
      },
      error: () => {
        this.teamLoading.set(false);
        this.teamError.set(true);
      },
    });
  }

  // ── Agent helpers ──

  agentRoleName(agentId: string): string {
    const member = this.members().find(m => m.agent_id === agentId);
    if (!member) return '';
    return this.roles().find(r => r.id === member.role_id)?.name ?? '';
  }

  selectAgent(agent: SpAgent | null): void {
    this.selectedAgent.set(agent);
    this.agentTasks.set([]);
    if (agent) {
      this.tasksLoading.set(true);
      this.agentsApi.getAgentTasks(agent.id).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
        next: (tasks) => {
          this.agentTasks.set(tasks);
          this.tasksLoading.set(false);
        },
        error: () => this.tasksLoading.set(false),
      });
    }
  }

  hasMetadata(agent: SpAgent): boolean {
    return Object.keys(agent.metadata).length > 0;
  }

  metadataEntries(agent: SpAgent): [string, string][] {
    return Object.entries(agent.metadata).map(([k, v]) => [k, String(v)]);
  }

  revokeAgent(agent: SpAgent): void {
    this.agentsApi.updateAgent(agent.id, { status: 'revoked' }).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
      next: (updated) => {
        this.agents.update(agents => agents.map(a => a.id === updated.id ? updated : a));
        this.selectedAgent.set(updated);
      },
    });
  }

  onAgentCreated(_agent: SpAgentRegistered): void {
    this.agentsApi.getAgents().pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
      next: (agents) => this.agents.set(agents),
    });
    this.loadTeamData();
  }

  toggleStateMenu(event: Event, taskId: string): void {
    event.stopPropagation();
    this.openMenuId.set(this.openMenuId() === taskId ? null : taskId);
  }

  onTransition(event: Event, task: SpAgentTask, target: string): void {
    event.stopPropagation();
    this.openMenuId.set(null);
    this.tasksApi.transition(task.id, target).subscribe({
      next: (updated) => {
        this.agentTasks.update(tasks =>
          tasks.map(t => t.id === updated.id ? { ...t, state: updated.state } : t),
        );
      },
    });
  }

  formatTime(iso: string | null): string {
    if (!iso) return '—';
    const date = new Date(iso);
    const now = Date.now();
    const diffMs = now - date.getTime();
    const diffMin = Math.floor(diffMs / 60000);
    if (diffMin < 1) return 'just now';
    if (diffMin < 60) return `${diffMin}m ago`;
    const diffHr = Math.floor(diffMin / 60);
    if (diffHr < 24) return `${diffHr}h ago`;
    return date.toLocaleDateString();
  }

  protected readonly stateColor = taskStateColor;
  protected readonly getTransitions = taskTransitions;

  // ── Team helpers ──

  selectRole(role: SpRole): void {
    this.selectedRole.set(role);
    this.selectedMember.set(null);
    this.editingRole.set(false);
    this.addingMember.set(false);
  }

  selectMember(member: SpMember): void {
    this.selectedMember.set(member);
    this.editingRole.set(false);
    this.addingMember.set(false);
  }

  filteredMembers(): SpMember[] {
    const role = this.selectedRole();
    if (!role) return [];
    return this.members().filter(m => m.role_id === role.id);
  }

  membersForRole(roleId: string): SpMember[] {
    return this.members().filter(m => m.role_id === roleId);
  }

  agentName(agentId: string): string {
    return this.agents().find(a => a.id === agentId)?.name ?? agentId.slice(0, 8);
  }

  agentStatus(agentId: string): string {
    return this.agents().find(a => a.id === agentId)?.status ?? 'offline';
  }

  roleName(roleId: string): string {
    return this.roles().find(r => r.id === roleId)?.name ?? roleId.slice(0, 8);
  }

  availableAgents(): SpAgent[] {
    const memberAgentIds = new Set(this.members().map(m => m.agent_id));
    return this.agents().filter(a => !memberAgentIds.has(a.id));
  }

  showRoleForm(): void {
    this.roleFormId.set(null);
    this.roleForm.set({ name: '', authorities: [] });
    this.editingRole.set(true);
    this.addingMember.set(false);
    this.selectedMember.set(null);
  }

  editRole(role: SpRole): void {
    this.roleFormId.set(role.id);
    this.roleForm.set({
      name: role.name,
      description: role.description ?? undefined,
      authorities: [...role.authorities],
      required_capabilities: [...role.required_capabilities],
      knowledge_scope: [...role.knowledge_scope],
    });
    this.editingRole.set(true);
  }

  updateRoleForm(field: string, value: string): void {
    this.roleForm.update(f => ({ ...f, [field]: value }));
  }

  toggleAuthority(auth: string): void {
    this.roleForm.update(f => {
      const auths = f.authorities.includes(auth)
        ? f.authorities.filter(a => a !== auth)
        : [...f.authorities, auth];
      return { ...f, authorities: auths };
    });
  }

  cancelRoleForm(): void {
    this.editingRole.set(false);
  }

  saveRole(): void {
    const form = this.roleForm();
    if (!form.name) return;
    const id = this.roleFormId();
    const op = id
      ? this.teamApi.updateRole(id, form)
      : this.teamApi.createRole(form);
    op.subscribe({
      next: () => {
        this.editingRole.set(false);
        this.loadTeamData();
      },
    });
  }

  deleteRole(): void {
    const id = this.roleFormId();
    if (!id) return;
    this.teamApi.deleteRole(id).subscribe({
      next: () => {
        this.editingRole.set(false);
        this.selectedRole.set(null);
        this.loadTeamData();
      },
    });
  }

  showMemberForm(): void {
    this.memberAgentId.set('');
    this.addingMember.set(true);
    this.editingRole.set(false);
    this.selectedMember.set(null);
  }

  saveMember(): void {
    const agentId = this.memberAgentId();
    const role = this.selectedRole();
    if (!agentId || !role) return;
    const body: SpMemberCreate = { agent_id: agentId, role_id: role.id };
    this.teamApi.createMember(body).subscribe({
      next: () => {
        this.addingMember.set(false);
        this.loadTeamData();
      },
    });
  }

  changeMemberRole(member: SpMember, roleId: string): void {
    this.teamApi.updateMember(member.id, { role_id: roleId }).subscribe({
      next: () => this.loadTeamData(),
    });
  }

  removeMember(member: SpMember): void {
    this.teamApi.deleteMember(member.id).subscribe({
      next: () => {
        this.selectedMember.set(null);
        this.loadTeamData();
      },
    });
  }

  formatDate(iso: string): string {
    return new Date(iso).toLocaleDateString();
  }
}
