import { Component, inject, signal, OnInit, DestroyRef, HostListener } from '@angular/core';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { timer, switchMap } from 'rxjs';
import { SlicePipe } from '@angular/common';
import { AgentsApiService, SpAgent, SpAgentRegistered, SpAgentTask } from '../../core/services/agents-api.service';
import { TasksApiService } from '../../core/services/tasks-api.service';
import { taskStateColor, taskTransitions } from '../../shared/ui-constants';
import { CreateAgentModalComponent } from './create-agent-modal';

@Component({
  selector: 'app-agents',
  standalone: true,
  imports: [TranslocoModule, SlicePipe, CreateAgentModalComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.agents') }}</h1>
        <button (click)="showCreateModal.set(true)"
          class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
          {{ t('agents.create') }}
        </button>
      </div>

      @if (loading()) {
        <p class="text-text-secondary">{{ t('common.loading') }}</p>
      } @else if (error()) {
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
                                [class.bg-ctp-green]="agent.status === 'idle' || agent.status === 'working'"
                                [class.bg-ctp-peach]="agent.status === 'offline'"
                                [class.bg-ctp-red]="agent.status === 'revoked'"></span>
                          <span class="text-text-secondary">{{ agent.status }}</span>
                        </span>
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
                      <td colspan="4" class="px-4 py-8 text-center text-text-muted">{{ t('common.empty') }}</td>
                    </tr>
                  }
                </tbody>
              </table>
            </div>
          </div>

          <!-- Detail panel -->
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
                          [class.bg-ctp-green]="agent.status === 'idle' || agent.status === 'working'"
                          [class.bg-ctp-peach]="agent.status === 'offline'"
                          [class.bg-ctp-red]="agent.status === 'revoked'"></span>
                    <span class="text-text-primary capitalize">{{ agent.status }}</span>
                  </div>
                </div>

                <!-- Owner -->
                @if (agent.owner_id) {
                  <div class="mb-4">
                    <span class="text-xs text-text-muted uppercase tracking-wide">Owner</span>
                    <p class="mt-1 text-text-secondary text-sm font-mono">{{ agent.owner_id | slice:0:8 }}...</p>
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
                  <div class="flex items-center justify-between">
                    <span class="text-xs text-text-muted uppercase tracking-wide">{{ t('agents.capabilities') }}</span>
                    @if (!editingCaps()) {
                      <button (click)="startEditCaps(agent)"
                        class="text-xs text-accent hover:opacity-80 transition-opacity">
                        {{ t('common.edit') }}
                      </button>
                    }
                  </div>
                  @if (editingCaps()) {
                    <div class="mt-2 space-y-2">
                      <input #capsInput type="text" [value]="capsInputValue()"
                        (input)="capsInputValue.set(capsInput.value)"
                        placeholder="rust, typescript, sql"
                        class="w-full px-2 py-1 text-xs rounded border border-border bg-bg-subtle text-text-primary
                               focus:outline-none focus:border-accent" />
                      <div class="flex gap-2">
                        <button (click)="saveCaps(agent)"
                          class="flex-1 px-2 py-1 text-xs font-medium bg-accent text-bg rounded-lg hover:opacity-90">
                          {{ t('common.save') }}
                        </button>
                        <button (click)="cancelEditCaps()"
                          class="flex-1 px-2 py-1 text-xs border border-border rounded text-text-secondary hover:bg-surface-hover transition-colors">
                          {{ t('common.cancel') }}
                        </button>
                      </div>
                    </div>
                  } @else {
                    <div class="mt-1 flex flex-wrap gap-1">
                      @for (cap of agent.capabilities; track cap) {
                        <span class="px-2 py-0.5 text-xs rounded bg-bg-subtle text-text-secondary">{{ cap }}</span>
                      }
                      @if (agent.capabilities.length === 0) {
                        <span class="text-text-muted text-sm">—</span>
                      }
                    </div>
                  }
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

      @if (showCreateModal()) {
        <app-create-agent-modal
          (created)="onAgentCreated($event); showCreateModal.set(false)"
          (cancelled)="showCreateModal.set(false)" />
      }
    </div>
  `,
})
export class AgentsPage implements OnInit {
  private api = inject(AgentsApiService);
  private tasksApi = inject(TasksApiService);
  private destroyRef = inject(DestroyRef);

  agents = signal<SpAgent[]>([]);
  selectedAgent = signal<SpAgent | null>(null);
  agentTasks = signal<SpAgentTask[]>([]);
  loading = signal(true);
  tasksLoading = signal(false);
  error = signal(false);
  openMenuId = signal<string | null>(null);
  showCreateModal = signal(false);
  editingCaps = signal(false);
  capsInputValue = signal('');

  @HostListener('document:click')
  onDocumentClick(): void {
    this.openMenuId.set(null);
  }

  ngOnInit(): void {
    timer(0, 10_000)
      .pipe(
        switchMap(() => this.api.getAgents()),
        takeUntilDestroyed(this.destroyRef),
      )
      .subscribe({
        next: (agents) => {
          this.agents.set(agents);
          this.loading.set(false);
          this.error.set(false);
          // Update selected agent if it still exists
          const sel = this.selectedAgent();
          if (sel) {
            const updated = agents.find(a => a.id === sel.id);
            if (updated) this.selectedAgent.set(updated);
            else this.selectedAgent.set(null);
          }
        },
        error: () => {
          this.loading.set(false);
          this.error.set(true);
        },
      });
  }

  selectAgent(agent: SpAgent | null): void {
    this.selectedAgent.set(agent);
    this.agentTasks.set([]);
    if (agent) {
      this.tasksLoading.set(true);
      this.api.getAgentTasks(agent.id).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
        next: (tasks) => {
          this.agentTasks.set(tasks);
          this.tasksLoading.set(false);
        },
        error: () => this.tasksLoading.set(false),
      });
    }
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

  hasMetadata(agent: SpAgent): boolean {
    return Object.keys(agent.metadata).length > 0;
  }

  metadataEntries(agent: SpAgent): [string, string][] {
    return Object.entries(agent.metadata).map(([k, v]) => [k, String(v)]);
  }

  startEditCaps(agent: SpAgent): void {
    this.capsInputValue.set(agent.capabilities.join(', '));
    this.editingCaps.set(true);
  }

  cancelEditCaps(): void {
    this.editingCaps.set(false);
    this.capsInputValue.set('');
  }

  saveCaps(agent: SpAgent): void {
    const caps = this.capsInputValue()
      .split(',')
      .map(c => c.trim())
      .filter(c => c.length > 0);
    this.api.updateAgent(agent.id, { capabilities: caps }).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
      next: (updated) => {
        this.agents.update(agents => agents.map(a => a.id === updated.id ? updated : a));
        this.selectedAgent.set(updated);
        this.editingCaps.set(false);
        this.capsInputValue.set('');
      },
    });
  }

  revokeAgent(agent: SpAgent): void {
    this.api.updateAgent(agent.id, { status: 'revoked' }).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
      next: (updated) => {
        this.agents.update(agents => agents.map(a => a.id === updated.id ? updated : a));
        this.selectedAgent.set(updated);
      },
    });
  }

  onAgentCreated(_agent: SpAgentRegistered): void {
    // Refresh the agents list immediately
    this.api.getAgents().pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
      next: (agents) => this.agents.set(agents),
    });
  }

  protected readonly stateColor = taskStateColor;
  protected readonly getTransitions = taskTransitions;

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
}
