import { Component, inject, signal, computed, effect } from '@angular/core';
import { DatePipe, JsonPipe, SlicePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import { forkJoin } from 'rxjs';
import { ProjectContext } from '../../core/services/project-context.service';
import {
  ObservationsApiService,
  SpObservation,
  ObservationKind,
  ObservationSeverity,
  ObservationStatus,
  SpObservationCreate,
  CleanupObservationsResult,
} from '../../core/services/observations-api.service';
import { TasksApiService } from '../../core/services/tasks-api.service';
import { OBSERVATION_KIND_COLORS, OBSERVATION_SEVERITY_COLORS } from '../../shared/ui-constants';
import { FilterBarComponent } from '../../shared/components/filter-bar/filter-bar';
import { ModalWrapperComponent } from '../../shared/components/modal-wrapper/modal-wrapper';
import { ConfirmDialogComponent } from '../../shared/components/confirm-dialog/confirm-dialog';

const KINDS: ObservationKind[] = ['insight', 'risk', 'opportunity', 'smell', 'inconsistency', 'improvement'];
const SEVERITIES: ObservationSeverity[] = ['critical', 'high', 'medium', 'low', 'info'];
const STATUSES: ObservationStatus[] = ['open', 'acknowledged', 'acted_on', 'dismissed'];

@Component({
  selector: 'app-observations',
  standalone: true,
  imports: [TranslocoModule, FormsModule, RouterLink, DatePipe, JsonPipe, SlicePipe, FilterBarComponent, ModalWrapperComponent, ConfirmDialogComponent],
  template: `
    <div *transloco="let t">
      <!-- Actions bar -->
      <div class="flex items-center justify-end gap-2 mb-3 sm:mb-4">
        <button (click)="confirmCleanup()"
          class="px-4 py-2 bg-ctp-red/20 text-ctp-red rounded-lg text-sm font-medium hover:bg-ctp-red/30"
          [disabled]="cleaningUp()">
          @if (cleaningUp()) {
            {{ t('observations.cleaningUp') }}
          } @else {
            {{ t('observations.cleanup') }}
          }
        </button>
        <button (click)="openCreate()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
          {{ t('observations.create') }}
        </button>
      </div>

      <!-- Filters -->
      <app-filter-bar
        [placeholder]="t('observations.searchPlaceholder')"
        [query]="searchQuery()"
        (queryChange)="searchQuery.set($event)">
        <select
          [(ngModel)]="selectedStatus"
          (ngModelChange)="loadItems()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('observations.allStatuses') }}</option>
          @for (s of statuses; track s) {
            <option [value]="s">{{ t('observations.status.' + s) }}</option>
          }
        </select>
        <select
          [(ngModel)]="selectedKind"
          (ngModelChange)="loadItems()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('observations.allKinds') }}</option>
          @for (k of kinds; track k) {
            <option [value]="k">{{ t('observations.kind.' + k) }}</option>
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
                : item.resolved_task_id
                  ? 'bg-ctp-green/5 border-ctp-green/30 hover:border-ctp-green/50'
                  : item.status === 'acknowledged'
                    ? 'bg-ctp-peach/5 border-ctp-peach/30 hover:border-ctp-peach/50'
                    : 'bg-surface border-border hover:border-accent/50'">
              <!-- Accordion header -->
              <button (click)="selectItem(item)" class="w-full text-left p-4">
                <div class="flex items-center gap-2">
                  <svg class="w-4 h-4 text-text-secondary shrink-0 transition-transform duration-200"
                    [class.rotate-90]="item.id === selected()?.id"
                    fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
                  </svg>
                  @if (item.resolved_task_id) {
                    <svg class="w-4 h-4 text-ctp-green shrink-0" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                    </svg>
                  } @else if (item.status === 'acknowledged') {
                    <svg class="w-4 h-4 text-ctp-peach shrink-0" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963 7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z" />
                      <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                    </svg>
                  }
                  <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ severityColor(item.severity) }}"
                    [class.opacity-50]="!!item.resolved_task_id"
                    [class.opacity-70]="!item.resolved_task_id && item.status === 'acknowledged'">
                    {{ t('observations.severity.' + item.severity) }}
                  </span>
                  <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ kindColor(item.kind) }}"
                    [class.opacity-50]="!!item.resolved_task_id"
                    [class.opacity-70]="!item.resolved_task_id && item.status === 'acknowledged'">
                    {{ t('observations.kind.' + item.kind) }}
                  </span>
                  <span class="text-sm font-medium" [class]="item.resolved_task_id ? 'text-text-secondary' : item.status === 'acknowledged' ? 'text-text-secondary' : 'text-text-primary'">
                    {{ item.title }}
                  </span>
                </div>
                @if (item.id !== selected()?.id) {
                  @if (item.description) {
                    <p class="text-xs line-clamp-2 mt-1 ml-6" [class]="item.resolved_task_id ? 'text-text-secondary/60' : item.status === 'acknowledged' ? 'text-text-secondary/80' : 'text-text-secondary'">
                      {{ item.description }}
                    </p>
                  }
                  <div class="flex items-center gap-2 mt-2 ml-6 text-xs text-text-secondary">
                    <span>{{ item.created_at | date:'short' }}</span>
                    @if (item.source_task_id) {
                      <a [routerLink]="['/tasks']" [queryParams]="{ id: item.source_task_id }"
                        (click)="$event.stopPropagation()"
                        class="px-1.5 py-0.5 bg-ctp-blue/10 text-ctp-blue rounded hover:bg-ctp-blue/20 hover:underline">
                        {{ taskTitles()[item.source_task_id] ? '#' + taskNumbers()[item.source_task_id] + ' ' + taskTitles()[item.source_task_id] : (item.source ?? item.source_task_id | slice:0:13) }}
                      </a>
                    } @else if (item.source) {
                      <span class="px-1.5 py-0.5 bg-ctp-blue/10 text-ctp-blue rounded">{{ item.source }}</span>
                    }
                    @if (item.resolved_task_id) {
                      <span class="px-1.5 py-0.5 bg-ctp-green/15 text-ctp-green rounded">{{ t('observations.status.' + item.status) }}</span>
                    } @else if (item.status === 'acknowledged') {
                      <span class="px-1.5 py-0.5 bg-ctp-peach/15 text-ctp-peach rounded">{{ t('observations.status.' + item.status) }}</span>
                    } @else {
                      <span class="px-1.5 py-0.5 bg-surface-hover rounded">{{ t('observations.status.' + item.status) }}</span>
                    }
                  </div>
                }
              </button>

              <!-- Expanded detail (inline) -->
              @if (item.id === selected()?.id) {
                <div class="px-4 pb-4 pt-0 border-t border-border/50 mt-0">
                  <!-- Badges + Actions -->
                  <div class="flex items-center gap-2 pt-3 mb-3 flex-wrap">
                    <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ severityColor(item.severity) }}">
                      {{ t('observations.severity.' + item.severity) }}
                    </span>
                    <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ kindColor(item.kind) }}">
                      {{ t('observations.kind.' + item.kind) }}
                    </span>
                    <span class="px-1.5 py-0.5 bg-surface-hover text-text-secondary rounded text-xs">
                      {{ t('observations.status.' + item.status) }}
                    </span>
                  </div>

                  @if (item.status === 'open' || item.status === 'acknowledged') {
                    <div class="flex gap-2 mb-4">
                      @if (item.status === 'open') {
                        <button (click)="acknowledge(item)"
                          class="px-3 py-1.5 text-xs font-medium bg-ctp-blue/20 text-ctp-blue rounded-lg hover:bg-ctp-blue/30">
                          {{ t('observations.acknowledge') }}
                        </button>
                      }
                      <button (click)="confirmDismiss(item)"
                        class="px-3 py-1.5 text-xs font-medium bg-ctp-overlay0/20 text-ctp-overlay0 rounded-lg hover:bg-ctp-overlay0/30">
                        {{ t('observations.dismiss') }}
                      </button>
                      <button (click)="promoteToTask(item)"
                        class="px-3 py-1.5 text-xs font-medium bg-ctp-green/20 text-ctp-green rounded-lg hover:bg-ctp-green/30">
                        {{ t('observations.promote') }}
                      </button>
                    </div>
                  }

                  <!-- Source -->
                  @if (item.source || item.source_task_id) {
                    <div class="mb-4">
                      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('observations.fieldSource') }}</h3>
                      <div class="flex items-center gap-2 flex-wrap">
                        @if (item.source) {
                          <span class="px-2 py-0.5 rounded-full text-xs font-medium bg-ctp-blue/15 text-ctp-blue">{{ item.source }}</span>
                        }
                        @if (item.source_task_id) {
                          <a [routerLink]="['/tasks']" [queryParams]="{ id: item.source_task_id }"
                            class="text-xs text-accent hover:underline">
                            {{ t('observations.sourceTask') }}:
                            @if (taskTitles()[item.source_task_id]) {
                              #{{ taskNumbers()[item.source_task_id] }} {{ taskTitles()[item.source_task_id] }}
                            } @else {
                              {{ item.source_task_id | slice:0:13 }}
                            }
                          </a>
                        }
                      </div>
                    </div>
                  }

                  <!-- Description -->
                  @if (item.description) {
                    <div class="mb-4">
                      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('observations.fieldDescription') }}</h3>
                      <p class="text-sm text-text-primary whitespace-pre-wrap break-words">{{ item.description }}</p>
                    </div>
                  }

                  <!-- Evidence -->
                  @if (item.evidence && objectKeys(item.evidence).length > 0) {
                    <div class="mb-4">
                      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('observations.fieldEvidence') }}</h3>
                      <pre class="text-xs text-text-primary bg-bg rounded-lg p-3 border border-border overflow-x-auto">{{ item.evidence | json }}</pre>
                    </div>
                  }

                  <!-- Resolved task link -->
                  @if (item.resolved_task_id) {
                    <div class="mb-4">
                      <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('observations.resolvedTask') }}</h3>
                      <span class="text-sm text-accent">{{ item.resolved_task_id }}</span>
                    </div>
                  }

                  <div class="pt-3 border-t border-border text-xs text-text-secondary">
                    {{ t('observations.createdAt') }}: {{ item.created_at | date:'medium' }}
                  </div>
                </div>
              }
            </div>
          }
        </div>
      }

      <!-- Create modal -->
      @if (showForm()) {
        <app-modal-wrapper maxWidth="max-w-lg" [scrollable]="true" (closed)="closeForm()">
          <h2 class="text-lg font-semibold text-text-primary mb-4">{{ t('observations.createTitle') }}</h2>
          <div class="space-y-4">
            <div>
              <label for="obs-title" class="block text-sm text-text-secondary mb-1">{{ t('observations.fieldTitle') }}</label>
              <input id="obs-title" type="text" [(ngModel)]="formTitle"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent" />
            </div>
            <div>
              <label for="obs-kind" class="block text-sm text-text-secondary mb-1">{{ t('observations.fieldKind') }}</label>
              <select id="obs-kind" [(ngModel)]="formKind"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent">
                @for (k of kinds; track k) {
                  <option [value]="k">{{ t('observations.kind.' + k) }}</option>
                }
              </select>
            </div>
            <div>
              <label for="obs-severity" class="block text-sm text-text-secondary mb-1">{{ t('observations.fieldSeverity') }}</label>
              <select id="obs-severity" [(ngModel)]="formSeverity"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent">
                @for (s of severities; track s) {
                  <option [value]="s">{{ t('observations.severity.' + s) }}</option>
                }
              </select>
            </div>
            <div>
              <label for="obs-description" class="block text-sm text-text-secondary mb-1">{{ t('observations.fieldDescription') }}</label>
              <textarea id="obs-description" [(ngModel)]="formDescription" rows="4"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
            </div>
            <div class="flex justify-end gap-3 pt-2">
              <button (click)="closeForm()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                {{ t('observations.cancel') }}
              </button>
              <button (click)="submitForm()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
                {{ t('observations.create') }}
              </button>
            </div>
          </div>
        </app-modal-wrapper>
      }

      <!-- Dismiss confirmation -->
      @if (showDismissConfirm()) {
        <app-confirm-dialog
          [title]="t('observations.dismissConfirmTitle')"
          [message]="t('observations.dismissConfirmMessage')"
          [cancelLabel]="t('observations.cancel')"
          [confirmLabel]="t('observations.dismiss')"
          (confirmed)="executeDismiss()"
          (cancelled)="closeDismissConfirm()" />
      }

      <!-- Cleanup confirmation -->
      @if (showCleanupConfirm()) {
        <app-confirm-dialog
          [title]="t('observations.cleanupConfirmTitle')"
          [message]="t('observations.cleanupConfirmMessage')"
          [cancelLabel]="t('observations.cancel')"
          [confirmLabel]="t('observations.cleanup')"
          (confirmed)="executeCleanup()"
          (cancelled)="closeCleanupConfirm()" />
      }

      <!-- Cleanup result -->
      @if (cleanupResult()) {
        <app-modal-wrapper maxWidth="max-w-md" [scrollable]="false" (closed)="cleanupResult.set(null)">
          <h2 class="text-lg font-semibold text-text-primary mb-4">{{ t('observations.cleanupResultTitle') }}</h2>
          <div class="space-y-2 text-sm text-text-primary">
            <div class="flex justify-between">
              <span>{{ t('observations.cleanupDismissed') }}</span>
              <span class="font-medium">{{ cleanupResult()!.deleted_dismissed }}</span>
            </div>
            <div class="flex justify-between">
              <span>{{ t('observations.cleanupAcknowledged') }}</span>
              <span class="font-medium">{{ cleanupResult()!.deleted_acknowledged }}</span>
            </div>
            <div class="flex justify-between">
              <span>{{ t('observations.cleanupActedOn') }}</span>
              <span class="font-medium">{{ cleanupResult()!.deleted_acted_on }}</span>
            </div>
            <div class="flex justify-between">
              <span>{{ t('observations.cleanupResolved') }}</span>
              <span class="font-medium">{{ cleanupResult()!.deleted_resolved }}</span>
            </div>
            <div class="flex justify-between">
              <span>{{ t('observations.cleanupDuplicates') }}</span>
              <span class="font-medium">{{ cleanupResult()!.deleted_duplicates }}</span>
            </div>
            <div class="flex justify-between pt-2 border-t border-border font-semibold">
              <span>{{ t('observations.cleanupTotal') }}</span>
              <span>{{ cleanupResult()!.total_deleted }}</span>
            </div>
          </div>
          <div class="flex justify-end mt-4">
            <button (click)="cleanupResult.set(null)" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
              {{ t('common.done') }}
            </button>
          </div>
        </app-modal-wrapper>
      }
    </div>
  `,
})
export class ObservationsPage {
  private api = inject(ObservationsApiService);
  private tasksApi = inject(TasksApiService);
  private ctx = inject(ProjectContext);

  readonly kinds = KINDS;
  readonly severities = SEVERITIES;
  readonly statuses = STATUSES;

  items = signal<SpObservation[]>([]);
  loading = signal(false);
  selected = signal<SpObservation | null>(null);
  searchQuery = signal('');
  selectedStatus = '';
  selectedKind = '';
  taskTitles = signal<Record<string, string>>({});
  taskNumbers = signal<Record<string, number>>({});

  showForm = signal(false);
  formTitle = '';
  formKind: ObservationKind = 'insight';
  formSeverity: ObservationSeverity = 'medium';
  formDescription = '';

  showDismissConfirm = signal(false);
  dismissTarget: SpObservation | null = null;

  showCleanupConfirm = signal(false);
  cleaningUp = signal(false);
  cleanupResult = signal<CleanupObservationsResult | null>(null);

  filtered = computed(() => {
    const q = this.searchQuery().toLowerCase().trim();
    let result = this.items();
    if (q) {
      result = result.filter(
        item => item.title.toLowerCase().includes(q) || (item.description ?? '').toLowerCase().includes(q),
      );
    }
    return result.slice().sort((a, b) => {
      const rank = (item: SpObservation) =>
        item.resolved_task_id ? 2 : item.status === 'acknowledged' ? 1 : 0;
      return rank(a) - rank(b);
    });
  });

  constructor() {
    effect(() => {
      this.ctx.projectId();
      this.selected.set(null);
      this.loadItems();
    });
  }

  selectItem(item: SpObservation): void {
    this.selected.set(item.id === this.selected()?.id ? null : item);
  }

  severityColor(severity: ObservationSeverity): string {
    return OBSERVATION_SEVERITY_COLORS[severity] ?? '';
  }

  kindColor(kind: ObservationKind): string {
    return OBSERVATION_KIND_COLORS[kind] ?? '';
  }

  objectKeys(obj: Record<string, unknown>): string[] {
    return Object.keys(obj);
  }

  acknowledge(item: SpObservation): void {
    this.api.update(item.id, { status: 'acknowledged' }).subscribe({
      next: () => this.loadItems(),
    });
  }

  confirmDismiss(item: SpObservation): void {
    this.dismissTarget = item;
    this.showDismissConfirm.set(true);
  }

  closeDismissConfirm(): void {
    this.showDismissConfirm.set(false);
    this.dismissTarget = null;
  }

  executeDismiss(): void {
    if (!this.dismissTarget) return;
    this.api.dismiss(this.dismissTarget.id).subscribe({
      next: () => {
        this.closeDismissConfirm();
        this.loadItems();
      },
    });
  }

  promoteToTask(item: SpObservation): void {
    this.api.promote(item.id).subscribe({
      next: () => this.loadItems(),
    });
  }

  openCreate(): void {
    this.formTitle = '';
    this.formKind = 'insight';
    this.formSeverity = 'medium';
    this.formDescription = '';
    this.showForm.set(true);
  }

  closeForm(): void {
    this.showForm.set(false);
  }

  submitForm(): void {
    const data: SpObservationCreate = {
      kind: this.formKind,
      title: this.formTitle,
      severity: this.formSeverity,
      description: this.formDescription || undefined,
    };
    this.api.create(data).subscribe({
      next: () => {
        this.closeForm();
        this.loadItems();
      },
    });
  }

  loadItems(): void {
    this.loading.set(true);
    const status = this.selectedStatus as ObservationStatus | '';
    const kind = this.selectedKind as ObservationKind | '';
    this.api.list(status || undefined, kind || undefined).subscribe({
      next: (items) => {
        this.items.set(items);
        this.loading.set(false);
        if (this.selected()) {
          const still = items.find(i => i.id === this.selected()!.id);
          this.selected.set(still ?? null);
        }
        this.fetchSourceTaskTitles(items);
      },
      error: () => this.loading.set(false),
    });
  }

  /** Fetch titles for all unique source_task_ids that we don't already have cached. */
  private fetchSourceTaskTitles(items: SpObservation[]): void {
    const cached = this.taskTitles();
    const ids = [...new Set(
      items
        .map(i => i.source_task_id)
        .filter((id): id is string => !!id && !cached[id]),
    )];
    if (ids.length === 0) return;

    const requests = Object.fromEntries(
      ids.map(id => [id, this.tasksApi.get(id)]),
    );
    forkJoin(requests).subscribe({
      next: (results) => {
        const titles = { ...this.taskTitles() };
        const numbers = { ...this.taskNumbers() };
        for (const [id, task] of Object.entries(results)) {
          titles[id] = task.title;
          numbers[id] = task.number;
        }
        this.taskTitles.set(titles);
        this.taskNumbers.set(numbers);
      },
    });
  }

  confirmCleanup(): void {
    this.showCleanupConfirm.set(true);
  }

  closeCleanupConfirm(): void {
    this.showCleanupConfirm.set(false);
  }

  executeCleanup(): void {
    this.closeCleanupConfirm();
    this.cleaningUp.set(true);
    this.api.cleanup().subscribe({
      next: (result) => {
        this.cleaningUp.set(false);
        this.cleanupResult.set(result);
        this.loadItems();
      },
      error: () => this.cleaningUp.set(false),
    });
  }
}
