import { Component, inject, signal, OnInit, DestroyRef, HostListener } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { forkJoin, timer, switchMap } from 'rxjs';
import { TeamApiService, SpRole, SpMember, SpRoleCreate, SpMemberCreate } from '../../core/services/team-api.service';
import { AgentsApiService, SpAgent, SpAgentRegistered, SpAgentTask } from '../../core/services/agents-api.service';
import { TasksApiService } from '../../core/services/tasks-api.service';
import { taskStateColor, taskTransitions } from '../../shared/ui-constants';
import { CreateAgentModalComponent } from '../agents/create-agent-modal';

type Tab = 'agents' | 'team';

@Component({
  selector: 'app-team',
  standalone: true,
  imports: [TranslocoModule, FormsModule, CreateAgentModalComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header with tab switcher -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <div class="flex items-center gap-1 bg-surface rounded-lg border border-border p-1">
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
        </div>

        @if (activeTab() === 'agents') {
          <button (click)="showCreateModal.set(true)"
            class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
            {{ t('agents.create') }}
          </button>
        } @else {
          <button (click)="showRoleForm()"
            class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
            {{ t('team.addRole') }}
          </button>
        }
      </div>

      @if (loading()) {
        <p class="text-text-secondary">{{ t('common.loading') }}</p>
      } @else if (error()) {
        <p class="text-error">{{ t('common.error') }}</p>
      } @else {

        <!-- ── AGENTS TAB ── -->
        @if (activeTab() === 'agents') {
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

        <!-- ── TEAM TAB ── -->
        @if (activeTab() === 'team') {
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
                              class="px-3 py-1.5 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
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
                              class="px-3 py-1.5 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50">
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

      @if (showCreateModal()) {
        <app-create-agent-modal
          (created)="onAgentCreated($event); showCreateModal.set(false)"
          (cancelled)="showCreateModal.set(false)" />
      }
    </div>
  `,
})
export class TeamPage implements OnInit {
  private teamApi = inject(TeamApiService);
  private agentsApi = inject(AgentsApiService);
  private tasksApi = inject(TasksApiService);
  private destroyRef = inject(DestroyRef);

  readonly allAuthorities = ['execute', 'delegate', 'review', 'create', 'decide', 'manage'];

  // Shared state
  activeTab = signal<Tab>('agents');
  roles = signal<SpRole[]>([]);
  members = signal<SpMember[]>([]);
  agents = signal<SpAgent[]>([]);
  loading = signal(true);
  error = signal(false);

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

  private loadTeamData(): void {
    forkJoin({
      roles: this.teamApi.getRoles(),
      members: this.teamApi.getMembers(),
    }).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
      next: ({ roles, members }) => {
        this.roles.set(roles);
        this.members.set(members);
        this.loading.set(false);
      },
      error: () => {
        this.loading.set(false);
        this.error.set(true);
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
