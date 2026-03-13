import { Component, inject, signal, computed } from '@angular/core';
import { DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { forkJoin } from 'rxjs';
import { TranslocoModule } from '@jsverse/transloco';
import {
  DecisionsApiService,
  SpDecision,
  SpDecisionAlternative,
  SpTaskSummaryForDecision,
  DecisionStatus,
} from '../../core/services/decisions-api.service';
import { TasksApiService, CreateTaskRequest } from '../../core/services/tasks-api.service';
import { PlaybooksApiService, SpPlaybook } from '../../core/services/playbooks-api.service';
import { DECISION_STATUS_COLORS } from '../../shared/ui-constants';
import { CrudFeatureBase } from '../../shared/crud-feature-base';
import { ModalWrapperComponent } from '../../shared/components/modal-wrapper/modal-wrapper';
import { FilterBarComponent } from '../../shared/components/filter-bar/filter-bar';
interface SpawnTaskItem {
  title: string;
  kind: string;
  urgent: boolean;
  spec: string;
}

const STATUSES: DecisionStatus[] = ['proposed', 'accepted', 'rejected', 'superseded', 'deprecated'];

@Component({
  selector: 'app-decisions',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, ModalWrapperComponent, FilterBarComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.decisions') }}</h1>
        <button (click)="openCreate()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
          {{ t('decisions.create') }}
        </button>
      </div>

      <!-- Filters -->
      <app-filter-bar
        [placeholder]="t('decisions.searchPlaceholder')"
        [query]="searchQuery()"
        (queryChange)="searchQuery.set($event)">
        <select
          [(ngModel)]="selectedStatus"
          (ngModelChange)="loadItems()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('decisions.allStatuses') }}</option>
          @for (s of statuses; track s) {
            <option [value]="s">{{ t('decisions.status.' + s) }}</option>
          }
        </select>
      </app-filter-bar>

      <!-- Content: accordion list -->
      @if (loading()) {
        <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
      } @else if (filtered().length === 0) {
        <p class="text-text-secondary text-sm">{{ t('common.empty') }}</p>
      } @else {
        <div class="space-y-2">
          @for (item of filtered(); track item.id) {
            <div class="rounded-lg border transition-colors"
              [class]="item.id === selected()?.id
                ? 'bg-accent/10 border-accent'
                : 'bg-surface border-border hover:border-accent/50'">
              <!-- Accordion header -->
              <button (click)="selectItem(item)" class="w-full text-left p-4">
                <div class="flex items-center gap-2">
                  <svg class="w-4 h-4 text-text-secondary shrink-0 transition-transform duration-200"
                    [class.rotate-90]="item.id === selected()?.id"
                    fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
                  </svg>
                  <span class="text-sm font-medium text-text-primary">{{ item.title }}</span>
                  <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ statusColor(item.status) }}">
                    {{ t('decisions.status.' + item.status) }}
                  </span>
                </div>
                @if (item.id !== selected()?.id && item.context) {
                  <p class="text-xs text-text-secondary line-clamp-2 mt-1 ml-6">{{ item.context }}</p>
                }
              </button>

              <!-- Expanded detail (inline) -->
              @if (item.id === selected()?.id) {
                <div class="px-4 pb-4 pt-0 border-t border-border/50 mt-0">
                  <!-- Actions row -->
                  <div class="flex items-center gap-3 mb-4 flex-wrap pt-3">
                    <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ statusColor(item.status) }}">
                      {{ t('decisions.status.' + item.status) }}
                    </span>
                    @if (item.status === 'proposed') {
                      <button (click)="updateStatus(item, 'accepted')"
                        class="px-3 py-1 text-xs font-medium bg-ctp-green/20 text-ctp-green rounded-lg hover:bg-ctp-green/30">
                        {{ t('decisions.accept') }}
                      </button>
                      <button (click)="updateStatus(item, 'rejected')"
                        class="px-3 py-1 text-xs font-medium bg-ctp-red/20 text-ctp-red rounded-lg hover:bg-ctp-red/30">
                        {{ t('decisions.reject') }}
                      </button>
                    }
                    @if (item.status === 'accepted') {
                      <button (click)="openSupersede(item)"
                        class="px-3 py-1 text-xs font-medium bg-ctp-yellow/20 text-ctp-yellow rounded-lg hover:bg-ctp-yellow/30">
                        {{ t('decisions.supersede') }}
                      </button>
                      <button (click)="updateStatus(item, 'deprecated')"
                        class="px-3 py-1 text-xs font-medium bg-ctp-overlay0/20 text-ctp-overlay0 rounded-lg hover:bg-ctp-overlay0/30">
                        {{ t('decisions.deprecate') }}
                      </button>
                    }
                    <div class="flex gap-2 ml-auto">
                      <button (click)="openSpawnTasks(item)"
                        class="px-3 py-1 text-xs font-medium bg-accent/20 text-accent rounded-lg hover:bg-accent/30">
                        {{ t('decisions.spawnTasks') }}
                      </button>
                      <button (click)="openEdit(item)" class="p-1.5 text-text-secondary hover:text-accent rounded" title="Edit">
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                          <path d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                        </svg>
                      </button>
                      <button (click)="deleteItem(item)" class="p-1.5 text-text-secondary hover:text-ctp-red rounded" title="Delete">
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                          <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      </button>
                    </div>
                  </div>

                  @if (item.superseded_by) {
                    <div class="text-xs text-ctp-yellow bg-ctp-yellow/10 rounded px-3 py-2 mb-4">
                      {{ t('decisions.supersededBy') }}: {{ item.superseded_by }}
                    </div>
                  }

                  <!-- Context -->
                  <div class="mb-4">
                    <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('decisions.fieldContext') }}</h3>
                    <p class="text-sm text-text-primary whitespace-pre-wrap">{{ item.context }}</p>
                  </div>

                  <!-- Decision -->
                  @if (item.decision) {
                    <div class="mb-4">
                      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('decisions.fieldDecision') }}</h3>
                      <p class="text-sm text-text-primary whitespace-pre-wrap">{{ item.decision }}</p>
                    </div>
                  }

                  <!-- Rationale -->
                  @if (item.rationale) {
                    <div class="mb-4">
                      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('decisions.fieldRationale') }}</h3>
                      <p class="text-sm text-text-primary whitespace-pre-wrap">{{ item.rationale }}</p>
                    </div>
                  }

                  <!-- Alternatives -->
                  @if (hasAlternatives(item)) {
                    <div class="mb-4">
                      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{{ t('decisions.fieldAlternatives') }}</h3>
                      @if (alternativesIsString(item)) {
                        <p class="text-sm text-text-primary whitespace-pre-wrap">{{ item.alternatives }}</p>
                      } @else {
                        <div class="space-y-2">
                          @for (alt of alternativesArray(item); track alt.name) {
                            <div class="bg-bg rounded-lg p-3 border border-border">
                              <p class="text-sm font-medium text-text-primary mb-1">{{ alt.name }}</p>
                              <div class="grid grid-cols-2 gap-2 text-xs">
                                <div>
                                  <span class="text-ctp-green">{{ t('decisions.pros') }}:</span>
                                  <span class="text-text-secondary ml-1">{{ alt.pros }}</span>
                                </div>
                                <div>
                                  <span class="text-ctp-red">{{ t('decisions.cons') }}:</span>
                                  <span class="text-text-secondary ml-1">{{ alt.cons }}</span>
                                </div>
                              </div>
                            </div>
                          }
                        </div>
                      }
                    </div>
                  }

                  <!-- Consequences -->
                  @if (item.consequences) {
                    <div class="mb-4">
                      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('decisions.fieldConsequences') }}</h3>
                      <p class="text-sm text-text-primary whitespace-pre-wrap">{{ item.consequences }}</p>
                    </div>
                  }

                  <!-- Linked Tasks -->
                  <div class="pt-3 border-t border-border mb-3">
                    <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{{ t('decisions.linkedTasks') }}</h3>
                    @if (linkedTasksLoading()) {
                      <p class="text-xs text-text-muted">{{ t('common.loading') }}</p>
                    } @else if (linkedTasks().length === 0) {
                      <p class="text-xs text-text-muted">{{ t('decisions.noLinkedTasks') }}</p>
                    } @else {
                      <div class="space-y-1">
                        @for (t2 of linkedTasks(); track t2.id) {
                          <div class="flex items-center gap-2 bg-bg rounded px-3 py-1.5 border border-border">
                            <span class="text-[10px] font-mono text-text-muted shrink-0">#{{ t2.number }}</span>
                            <span class="text-xs text-text-primary truncate flex-1">{{ t2.title }}</span>
                            <span class="px-1.5 py-0.5 rounded-full text-[10px] font-medium shrink-0"
                              [class]="taskStateClass(t2.state)">{{ t2.state }}</span>
                          </div>
                        }
                      </div>
                    }
                  </div>

                  <div class="text-xs text-text-secondary">
                    {{ t('decisions.updatedAt') }}: {{ item.updated_at | date:'medium' }}
                  </div>
                </div>
              }
            </div>
          }
        </div>
      }

      <!-- Create/Edit modal -->
      @if (showForm()) {
        <app-modal-wrapper maxWidth="max-w-2xl" [scrollable]="true" (closed)="closeForm()">
          <h2 class="text-lg font-semibold text-text-primary mb-4">
            {{ editing() ? t('decisions.editTitle') : t('decisions.createTitle') }}
          </h2>
          <div class="space-y-4">
            <div>
              <label for="dec-title" class="block text-sm text-text-secondary mb-1">{{ t('decisions.fieldTitle') }}</label>
              <input id="dec-title" type="text" [(ngModel)]="formTitle"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent" />
            </div>
            <div>
              <label for="dec-context" class="block text-sm text-text-secondary mb-1">{{ t('decisions.fieldContext') }}</label>
              <textarea id="dec-context" [(ngModel)]="formContext" rows="3"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
            </div>
            <div>
              <label for="dec-decision" class="block text-sm text-text-secondary mb-1">{{ t('decisions.fieldDecision') }}</label>
              <textarea id="dec-decision" [(ngModel)]="formDecision" rows="3"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
            </div>
            <div>
              <label for="dec-rationale" class="block text-sm text-text-secondary mb-1">{{ t('decisions.fieldRationale') }}</label>
              <textarea id="dec-rationale" [(ngModel)]="formRationale" rows="3"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
            </div>
            <div>
              <label for="dec-consequences" class="block text-sm text-text-secondary mb-1">{{ t('decisions.fieldConsequences') }}</label>
              <textarea id="dec-consequences" [(ngModel)]="formConsequences" rows="2"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
            </div>

            <!-- Alternatives -->
            <div>
              <div class="flex items-center justify-between mb-2">
                <span class="text-sm text-text-secondary">{{ t('decisions.fieldAlternatives') }}</span>
                <button (click)="addAlternative()" class="text-xs text-accent hover:underline">
                  + {{ t('decisions.addAlternative') }}
                </button>
              </div>
              @for (alt of formAlternatives; track $index; let i = $index) {
                <div class="bg-surface rounded-lg p-3 border border-border mb-2">
                  <div class="flex items-center justify-between mb-2">
                    <input type="text" [(ngModel)]="alt.name" [placeholder]="t('decisions.altName')"
                      class="flex-1 bg-bg text-text-primary text-sm rounded px-2 py-1 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent" />
                    <button (click)="removeAlternative(i)" class="ml-2 text-text-secondary hover:text-ctp-red text-xs">✕</button>
                  </div>
                  <div class="grid grid-cols-2 gap-2">
                    <input type="text" [(ngModel)]="alt.pros" [placeholder]="t('decisions.pros')"
                      class="bg-bg text-text-primary text-sm rounded px-2 py-1 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent" />
                    <input type="text" [(ngModel)]="alt.cons" [placeholder]="t('decisions.cons')"
                      class="bg-bg text-text-primary text-sm rounded px-2 py-1 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent" />
                  </div>
                </div>
              }
            </div>

            <div class="flex justify-end gap-3 pt-2">
              <button (click)="closeForm()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                {{ t('decisions.cancel') }}
              </button>
              <button (click)="submitForm()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
                {{ editing() ? t('decisions.save') : t('decisions.create') }}
              </button>
            </div>
          </div>
        </app-modal-wrapper>
      }

      <!-- Spawn Tasks modal -->
      @if (showSpawnTasks()) {
        <app-modal-wrapper maxWidth="max-w-2xl" [scrollable]="true" (closed)="closeSpawnTasks()">
          <h2 class="text-lg font-semibold text-text-primary mb-1">{{ t('decisions.spawnTasksTitle') }}</h2>
          <p class="text-sm text-text-secondary mb-4">{{ t('decisions.spawnTasksDescription') }}</p>

          <!-- Playbook selector -->
          <div class="mb-4">
            <label for="dec-spawn-playbook" class="block text-sm text-text-secondary mb-1">{{ t('decisions.spawnPlaybook') }}</label>
            <select id="dec-spawn-playbook" [(ngModel)]="spawnPlaybookId"
              class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                     focus:outline-none focus:ring-1 focus:ring-accent">
              <option value="">{{ t('decisions.spawnNoPlaybook') }}</option>
              @for (pb of spawnPlaybooks(); track pb.id) {
                <option [value]="pb.id">{{ pb.title }}</option>
              }
            </select>
          </div>

          <!-- Tasks list -->
          <div class="space-y-3 mb-4">
            @for (task of spawnTasks(); track $index; let i = $index) {
              <div class="bg-bg rounded-lg p-3 border border-border">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-xs font-semibold text-text-secondary uppercase tracking-wider">{{ t('decisions.spawnTask') }} {{ i + 1 }}</span>
                  @if (spawnTasks().length > 1) {
                    <button (click)="removeSpawnTask(i)" class="text-text-secondary hover:text-ctp-red text-xs">✕</button>
                  }
                </div>
                <div class="space-y-2">
                  <input type="text" [(ngModel)]="task.title" [placeholder]="t('decisions.spawnTaskTitle')"
                    class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent" />
                  <div class="flex gap-2">
                    <select [(ngModel)]="task.kind"
                      class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                             focus:outline-none focus:ring-1 focus:ring-accent flex-1">
                      <option value="feature">Feature</option>
                      <option value="bug">Bug</option>
                      <option value="chore">Chore</option>
                      <option value="spike">Spike</option>
                      <option value="refactor">Refactor</option>
                    </select>
                    <label class="flex items-center gap-1.5 cursor-pointer select-none px-3 py-2">
                      <input type="checkbox" [(ngModel)]="task.urgent"
                        class="w-4 h-4 rounded border-border bg-surface text-ctp-red focus:ring-ctp-red focus:ring-1" />
                      <span class="text-sm" [class]="task.urgent ? 'text-ctp-red font-medium' : 'text-text-secondary'">Urgent</span>
                    </label>
                  </div>
                  <textarea [(ngModel)]="task.spec" [placeholder]="t('decisions.spawnTaskSpec')" rows="3"
                    class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
                </div>
              </div>
            }
          </div>

          <div class="flex items-center justify-between">
            <button (click)="addSpawnTask()" class="text-xs text-accent hover:underline">
              + {{ t('decisions.spawnAddTask') }}
            </button>
            <div class="flex gap-3">
              <button (click)="closeSpawnTasks()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                {{ t('decisions.cancel') }}
              </button>
              <button (click)="confirmSpawnTasks()" [disabled]="spawnSubmitting()"
                class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50">
                {{ spawnSubmitting() ? t('common.saving') : t('decisions.spawnCreate') }}
              </button>
            </div>
          </div>
        </app-modal-wrapper>
      }

      <!-- Supersede modal -->
      @if (showSupersede()) {
        <app-modal-wrapper maxWidth="max-w-md" (closed)="closeSupersede()">
          <h2 class="text-lg font-semibold text-text-primary mb-4">{{ t('decisions.supersedeTitle') }}</h2>
          <p class="text-sm text-text-secondary mb-4">{{ t('decisions.supersedeDescription') }}</p>
          <select [(ngModel)]="supersedeTargetId"
            class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent mb-4">
            <option value="">{{ t('decisions.selectDecision') }}</option>
            @for (d of otherDecisions(); track d.id) {
              <option [value]="d.id">{{ d.title }}</option>
            }
          </select>
          <div class="flex justify-end gap-3">
            <button (click)="closeSupersede()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
              {{ t('decisions.cancel') }}
            </button>
            <button (click)="confirmSupersede()" [disabled]="!supersedeTargetId"
              class="px-4 py-2 bg-ctp-yellow/20 text-ctp-yellow rounded-lg text-sm font-medium hover:bg-ctp-yellow/30 disabled:opacity-50">
              {{ t('decisions.supersede') }}
            </button>
          </div>
        </app-modal-wrapper>
      }
    </div>
  `,
})
export class DecisionsPage extends CrudFeatureBase<SpDecision> {
  private api = inject(DecisionsApiService);
  private tasksApi = inject(TasksApiService);
  private playbooksApi = inject(PlaybooksApiService);

  readonly statuses = STATUSES;

  selectedStatus = '';

  showSupersede = signal(false);
  supersedeSourceId = '';
  supersedeTargetId = '';

  // Linked tasks state (tasks spawned from this decision)
  linkedTasks = signal<SpTaskSummaryForDecision[]>([]);
  linkedTasksLoading = signal(false);

  // Spawn tasks state
  showSpawnTasks = signal(false);
  spawnSubmitting = signal(false);
  spawnPlaybookId = '';
  spawnPlaybooks = signal<SpPlaybook[]>([]);
  spawnTasks = signal<SpawnTaskItem[]>([]);

  formTitle = '';
  formContext = '';
  formDecision = '';
  formRationale = '';
  formConsequences = '';
  formAlternatives: SpDecisionAlternative[] = [];

  filtered = computed(() => {
    const q = this.searchQuery().toLowerCase().trim();
    if (!q) return this.items();
    return this.items().filter(
      (item) => item.title.toLowerCase().includes(q) || item.context.toLowerCase().includes(q),
    );
  });

  otherDecisions = computed(() => {
    return this.items().filter((d) => d.id !== this.supersedeSourceId);
  });

  override loadItems(): void {
    this.loading.set(true);
    const status = this.selectedStatus as DecisionStatus | '';
    this.api.list(status || undefined).subscribe({
      next: (items) => this.refreshAfterMutation(items),
      error: () => this.loading.set(false),
    });
  }

  override selectItem(item: SpDecision): void {
    super.selectItem(item);
    this.loadLinkedTasks(item.id);
  }

  private loadLinkedTasks(decisionId: string): void {
    this.linkedTasksLoading.set(true);
    this.api.listLinkedTasks(decisionId).subscribe({
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

  taskStateClass(state: string): string {
    const map: Record<string, string> = {
      backlog: 'bg-ctp-overlay0/20 text-ctp-overlay0',
      ready: 'bg-ctp-blue/20 text-ctp-blue',
      working: 'bg-ctp-yellow/20 text-ctp-yellow',
      implement: 'bg-ctp-yellow/20 text-ctp-yellow',
      review: 'bg-ctp-mauve/20 text-ctp-mauve',
      done: 'bg-ctp-green/20 text-ctp-green',
      cancelled: 'bg-ctp-red/20 text-ctp-red',
    };
    return map[state] ?? 'bg-surface text-text-muted';
  }

  protected override resetForm(): void {
    this.formTitle = '';
    this.formContext = '';
    this.formDecision = '';
    this.formRationale = '';
    this.formConsequences = '';
    this.formAlternatives = [];
  }

  protected override fillForm(item: SpDecision): void {
    this.formTitle = item.title;
    this.formContext = item.context;
    this.formDecision = item.decision ?? '';
    this.formRationale = item.rationale ?? '';
    this.formConsequences = item.consequences ?? '';
    this.formAlternatives = Array.isArray(item.alternatives) ? item.alternatives.map((a) => ({ ...a })) : [];
  }

  /** True if alternatives is a non-empty array or non-empty string */
  hasAlternatives(item: SpDecision): boolean {
    const a = item.alternatives;
    if (!a) return false;
    if (typeof a === 'string') return a.length > 0;
    return Array.isArray(a) && a.length > 0;
  }

  /** True when alternatives was stored as a plain string (e.g. posted via agent-cli) */
  alternativesIsString(item: SpDecision): boolean {
    return typeof item.alternatives === 'string';
  }

  /** Returns alternatives as a typed array, or [] when stored as a string */
  alternativesArray(item: SpDecision): SpDecisionAlternative[] {
    return Array.isArray(item.alternatives) ? (item.alternatives as SpDecisionAlternative[]) : [];
  }

  statusColor(status: DecisionStatus): string {
    return DECISION_STATUS_COLORS[status] ?? '';
  }

  updateStatus(item: SpDecision, status: DecisionStatus): void {
    this.api.update(item.id, { status }).subscribe({
      next: () => this.loadItems(),
    });
  }

  addAlternative(): void {
    this.formAlternatives.push({ name: '', pros: '', cons: '' });
  }

  removeAlternative(index: number): void {
    this.formAlternatives.splice(index, 1);
  }

  submitForm(): void {
    const alts = this.formAlternatives.filter((a) => a.name.trim().length > 0);
    const existing = this.editing();
    if (existing) {
      this.api
        .update(existing.id, {
          title: this.formTitle,
          context: this.formContext,
          decision: this.formDecision,
          rationale: this.formRationale,
          consequences: this.formConsequences,
          alternatives: alts,
        })
        .subscribe({
          next: () => {
            this.closeForm();
            this.loadItems();
          },
        });
    } else {
      this.api
        .create({
          title: this.formTitle,
          context: this.formContext,
          decision: this.formDecision,
          rationale: this.formRationale,
          alternatives: alts,
          consequences: this.formConsequences || undefined,
        })
        .subscribe({
          next: () => {
            this.closeForm();
            this.loadItems();
          },
        });
    }
  }

  deleteItem(item: SpDecision): void {
    this.api.delete(item.id).subscribe({
      next: () => {
        this.selected.set(null);
        this.loadItems();
      },
    });
  }

  openSupersede(item: SpDecision): void {
    this.supersedeSourceId = item.id;
    this.supersedeTargetId = '';
    this.showSupersede.set(true);
  }

  closeSupersede(): void {
    this.showSupersede.set(false);
  }

  confirmSupersede(): void {
    if (!this.supersedeTargetId) return;
    this.api
      .update(this.supersedeSourceId, {
        status: 'superseded',
        superseded_by: this.supersedeTargetId,
      })
      .subscribe({
        next: () => {
          this.closeSupersede();
          this.loadItems();
        },
      });
  }

  // Spawn tasks

  openSpawnTasks(decision: SpDecision): void {
    const spec = this.buildDecisionSpec(decision);
    this.spawnTasks.set([{ title: `Implement: ${decision.title}`, kind: 'feature', urgent: false, spec }]);
    this.spawnPlaybookId = '';
    this.spawnSubmitting.set(false);
    this.playbooksApi.list().subscribe({
      next: (pbs) => this.spawnPlaybooks.set(pbs),
    });
    this.showSpawnTasks.set(true);
  }

  closeSpawnTasks(): void {
    this.showSpawnTasks.set(false);
  }

  addSpawnTask(): void {
    this.spawnTasks.update((tasks) => [...tasks, { title: '', kind: 'feature', urgent: false, spec: '' }]);
  }

  removeSpawnTask(index: number): void {
    this.spawnTasks.update((tasks) => tasks.filter((_, i) => i !== index));
  }

  confirmSpawnTasks(): void {
    const validTasks = this.spawnTasks().filter((t) => t.title.trim().length > 0);
    if (validTasks.length === 0) return;

    this.spawnSubmitting.set(true);
    const decisionId = this.selected()?.id;

    const requests = validTasks.map((t) => {
      const req: CreateTaskRequest = {
        title: t.title.trim(),
        kind: t.kind,
        urgent: t.urgent,
        context: t.spec.trim() ? { spec: t.spec.trim() } : {},
      };
      if (this.spawnPlaybookId) {
        req.playbook_id = this.spawnPlaybookId;
      }
      if (decisionId) {
        req.decision_id = decisionId;
      }
      return this.tasksApi.create(req);
    });

    forkJoin(requests).subscribe({
      next: () => {
        this.spawnSubmitting.set(false);
        this.closeSpawnTasks();
        if (decisionId) {
          this.loadLinkedTasks(decisionId);
        }
      },
      error: () => {
        this.spawnSubmitting.set(false);
      },
    });
  }

  private buildDecisionSpec(decision: SpDecision): string {
    const parts: string[] = [];
    if (decision.decision) {
      parts.push(`Decision: ${decision.decision}`);
    }
    if (decision.context) {
      parts.push(`Context: ${decision.context}`);
    }
    if (decision.rationale) {
      parts.push(`Rationale: ${decision.rationale}`);
    }
    if (decision.consequences) {
      parts.push(`Consequences: ${decision.consequences}`);
    }
    return parts.join('\n\n');
  }
}
