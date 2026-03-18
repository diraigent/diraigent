import { Component, inject, signal, computed, effect } from '@angular/core';
import { DatePipe, JsonPipe, SlicePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import { ProjectContext } from '../../core/services/project-context.service';
import {
  VerificationsApiService,
  SpVerification,
  SpVerificationCreate,
  VerificationKind,
  VerificationStatus,
} from '../../core/services/verifications-api.service';
import { TasksApiService, SpTask } from '../../core/services/tasks-api.service';
import {
  VERIFICATION_STATUS_COLORS, VERIFICATION_KIND_COLORS,
} from '../../shared/ui-constants';

const KINDS: VerificationKind[] = ['test', 'acceptance', 'sign_off'];
const STATUSES: VerificationStatus[] = ['pass', 'fail', 'pending', 'skipped'];

const STATUS_COLORS = VERIFICATION_STATUS_COLORS;
const KIND_COLORS = VERIFICATION_KIND_COLORS;

@Component({
  selector: 'app-verifications',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, JsonPipe, SlicePipe],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.verifications') }}</h1>
        <button (click)="openCreate()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
          {{ t('verifications.create') }}
        </button>
      </div>

      <!-- Filters -->
      <div class="flex flex-wrap gap-3 mb-6">
        <input
          type="text"
          [placeholder]="t('verifications.searchPlaceholder')"
          [ngModel]="searchQuery()"
          (ngModelChange)="searchQuery.set($event)"
          class="flex-1 min-w-[200px] bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
        <select
          [(ngModel)]="selectedStatus"
          (ngModelChange)="loadVerifications()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('verifications.allStatuses') }}</option>
          @for (s of statuses; track s) {
            <option [value]="s">{{ t('verifications.status.' + s) }}</option>
          }
        </select>
        <select
          [(ngModel)]="selectedKind"
          (ngModelChange)="loadVerifications()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('verifications.allKinds') }}</option>
          @for (k of kinds; track k) {
            <option [value]="k">{{ t('verifications.kind.' + k) }}</option>
          }
        </select>
      </div>

      <!-- Content: list + detail -->
      <div class="flex flex-col lg:flex-row gap-4 lg:gap-6">
        <!-- List -->
        <div class="flex-1 min-w-0">
          @if (loading()) {
            <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
          } @else if (filtered().length === 0) {
            <p class="text-text-secondary text-sm">{{ t('common.empty') }}</p>
          } @else {
            <div class="space-y-2">
              @for (item of filtered(); track item.id) {
                <button
                  (click)="selectItem(item)"
                  class="w-full text-left p-4 rounded-lg border transition-colors"
                  [class]="item.id === selected()?.id
                    ? 'bg-accent/10 border-accent'
                    : 'bg-surface border-border hover:border-accent/50'">
                  <div class="flex items-center gap-2 mb-1">
                    <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ statusColor(item.status) }}">
                      {{ t('verifications.status.' + item.status) }}
                    </span>
                    <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ kindColor(item.kind) }}">
                      {{ t('verifications.kind.' + item.kind) }}
                    </span>
                    <span class="text-sm font-medium text-text-primary">{{ item.title }}</span>
                  </div>
                  @if (item.detail) {
                    <p class="text-xs text-text-secondary line-clamp-2 mt-1">{{ item.detail }}</p>
                  }
                  <div class="flex items-center gap-2 mt-2 text-xs text-text-secondary">
                    <span>{{ item.created_at | date:'short' }}</span>
                    @if (item.task_id) {
                      <button type="button" (click)="showTaskPreview(item.task_id!, $event)"
                        class="px-1.5 py-0.5 bg-surface-hover rounded font-mono hover:bg-accent/20 hover:text-accent cursor-pointer transition-colors">{{ item.task_id | slice:0:8 }}</button>
                    }
                  </div>
                </button>
              }
            </div>

            <!-- Pagination -->
            @if (total() > limit) {
              <div class="flex items-center justify-between mt-4 text-sm text-text-secondary">
                <span>{{ offset() + 1 }}–{{ minVal(offset() + limit, total()) }} / {{ total() }}</span>
                <div class="flex gap-2">
                  <button
                    (click)="prevPage()"
                    [disabled]="offset() === 0"
                    class="px-3 py-1.5 rounded-lg border border-border text-text-secondary hover:text-text-primary disabled:opacity-40">
                    {{ t('tasks.prev') }}
                  </button>
                  <button
                    (click)="nextPage()"
                    [disabled]="!hasMore()"
                    class="px-3 py-1.5 rounded-lg border border-border text-text-secondary hover:text-text-primary disabled:opacity-40">
                    {{ t('tasks.next') }}
                  </button>
                </div>
              </div>
            }
          }
        </div>

        <!-- Detail panel -->
        @if (selected()) {
          <div class="w-full lg:w-[520px] shrink-0 bg-surface rounded-lg border border-border p-4 sm:p-6 max-h-[calc(100vh-200px)] overflow-y-auto overflow-x-hidden min-w-0">
            <div class="flex items-center justify-between mb-3">
              <h2 class="text-lg font-semibold text-text-primary">{{ selected()!.title }}</h2>
              <button (click)="selected.set(null)" class="p-1.5 text-text-secondary hover:text-text-primary rounded">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            <!-- Badges -->
            <div class="flex items-center gap-2 mb-4">
              <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ statusColor(selected()!.status) }}">
                {{ t('verifications.status.' + selected()!.status) }}
              </span>
              <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ kindColor(selected()!.kind) }}">
                {{ t('verifications.kind.' + selected()!.kind) }}
              </span>
            </div>

            <!-- Status actions -->
            <div class="flex gap-2 mb-4">
              @for (s of statuses; track s) {
                @if (s !== selected()!.status) {
                  <button (click)="updateStatus(selected()!, s)"
                    class="px-3 py-1.5 text-xs font-medium rounded-lg {{ statusColor(s) }} hover:opacity-80">
                    {{ t('verifications.status.' + s) }}
                  </button>
                }
              }
            </div>

            <!-- Detail -->
            @if (selected()!.detail) {
              <div class="mb-4">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('verifications.fieldDetail') }}</h3>
                <p class="text-sm text-text-primary whitespace-pre-wrap break-words">{{ selected()!.detail }}</p>
              </div>
            }

            <!-- Task link -->
            @if (selected()!.task_id) {
              <div class="mb-4">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('verifications.fieldTask') }}</h3>
                <button (click)="showTaskPreview(selected()!.task_id!)"
                  class="text-sm text-accent font-mono hover:underline cursor-pointer break-all">{{ selected()!.task_id }}</button>
              </div>
            }

            <!-- Agent / User -->
            @if (selected()!.agent_id) {
              <div class="mb-4">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('verifications.fieldAgent') }}</h3>
                <span class="text-sm text-text-primary font-mono">{{ selected()!.agent_id | slice:0:8 }}</span>
              </div>
            }

            <!-- Evidence -->
            @if (selected()!.evidence && objectKeys(selected()!.evidence).length > 0) {
              <div class="mb-4">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('verifications.fieldEvidence') }}</h3>
                <pre class="text-xs text-text-primary bg-bg rounded-lg p-3 border border-border overflow-x-auto">{{ selected()!.evidence | json }}</pre>
              </div>
            }

            <div class="pt-3 border-t border-border text-xs text-text-secondary">
              {{ t('verifications.createdAt') }}: {{ selected()!.created_at | date:'medium' }}
            </div>
          </div>
        }
      </div>

      <!-- Create modal -->
      @if (showForm()) {
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-[70]"
             role="button" tabindex="0" aria-label="Close modal"
             (click)="closeForm()" (keydown.enter)="closeForm()" (keydown.escape)="closeForm()">
          <div class="bg-bg border border-border rounded-xl p-6 w-full max-w-lg max-h-[90vh] overflow-y-auto"
               tabindex="-1" (click)="$event.stopPropagation()" (keydown.enter)="$event.stopPropagation()">
            <h2 class="text-lg font-semibold text-text-primary mb-4">{{ t('verifications.createTitle') }}</h2>
            <div class="space-y-4">
              <div>
                <label for="ver-title" class="block text-sm text-text-secondary mb-1">{{ t('verifications.fieldTitle') }}</label>
                <input id="ver-title" type="text" [(ngModel)]="formTitle"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent" />
              </div>
              <div>
                <label for="ver-kind" class="block text-sm text-text-secondary mb-1">{{ t('verifications.fieldKind') }}</label>
                <select id="ver-kind" [(ngModel)]="formKind"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent">
                  @for (k of kinds; track k) {
                    <option [value]="k">{{ t('verifications.kind.' + k) }}</option>
                  }
                </select>
              </div>
              <div>
                <label for="ver-status" class="block text-sm text-text-secondary mb-1">{{ t('verifications.fieldStatus') }}</label>
                <select id="ver-status" [(ngModel)]="formStatus"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent">
                  @for (s of statuses; track s) {
                    <option [value]="s">{{ t('verifications.status.' + s) }}</option>
                  }
                </select>
              </div>
              <div>
                <label for="ver-task" class="block text-sm text-text-secondary mb-1">{{ t('verifications.fieldTask') }}</label>
                <input id="ver-task" type="text" [(ngModel)]="formTaskId" [placeholder]="t('verifications.taskIdPlaceholder')"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent" />
              </div>
              <div>
                <label for="ver-detail" class="block text-sm text-text-secondary mb-1">{{ t('verifications.fieldDetail') }}</label>
                <textarea id="ver-detail" [(ngModel)]="formDetail" rows="4"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
              </div>
              <div class="flex justify-end gap-3 pt-2">
                <button (click)="closeForm()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                  {{ t('verifications.cancel') }}
                </button>
                <button (click)="submitForm()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
                  {{ t('verifications.create') }}
                </button>
              </div>
            </div>
          </div>
        </div>
      }

      <!-- Task preview modal -->
      @if (previewTask()) {
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-[70]"
             role="button" tabindex="0" aria-label="Close preview"
             (click)="closeTaskPreview()" (keydown.escape)="closeTaskPreview()">
          <!-- eslint-disable-next-line @angular-eslint/template/click-events-have-key-events, @angular-eslint/template/interactive-supports-focus -->
          <div class="bg-bg border border-border rounded-xl p-6 w-full max-w-lg max-h-[90vh] overflow-y-auto" (click)="$event.stopPropagation()">
            @if (previewTaskLoading()) {
              <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
            } @else {
              <div class="flex items-center justify-between mb-3">
                <h2 class="text-lg font-semibold text-text-primary">{{ previewTask()!.title }}</h2>
                <button (click)="closeTaskPreview()" class="p-1.5 text-text-secondary hover:text-text-primary rounded">
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                    <path d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>

              <div class="flex items-center gap-2 mb-4">
                <span class="px-2 py-0.5 rounded-full text-xs font-medium bg-accent/20 text-accent">{{ previewTask()!.state }}</span>
                <span class="px-2 py-0.5 rounded-full text-xs font-medium bg-ctp-overlay0/20 text-ctp-overlay0">{{ previewTask()!.kind }}</span>
                @if (previewTask()!.urgent) {
                  <span class="px-2 py-0.5 rounded-full text-xs font-medium bg-ctp-red/20 text-ctp-red">Urgent</span>
                }
              </div>

              @if (previewTask()!.context && previewTask()!.context['spec']) {
                <div class="mb-4">
                  <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">Spec</h3>
                  <p class="text-sm text-text-primary whitespace-pre-wrap">{{ previewTask()!.context['spec'] }}</p>
                </div>
              }

              @if (previewTask()!.assigned_agent_id) {
                <div class="mb-4">
                  <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">Agent</h3>
                  <span class="text-sm text-text-primary font-mono">{{ previewTask()!.assigned_agent_id | slice:0:8 }}</span>
                </div>
              }

              <div class="pt-3 border-t border-border flex items-center justify-between">
                <span class="text-xs text-text-secondary">{{ previewTask()!.created_at | date:'medium' }}</span>
                <button (click)="navigateToTask(previewTask()!.id)"
                  class="px-3 py-1.5 text-xs font-medium rounded-lg bg-accent/20 text-accent hover:opacity-80">
                  {{ t('verifications.viewInTasks') }}
                </button>
              </div>
            }
          </div>
        </div>
      }
    </div>
  `,
})
export class VerificationsPage {
  private api = inject(VerificationsApiService);
  private tasksApi = inject(TasksApiService);
  private router = inject(Router);
  private ctx = inject(ProjectContext);

  readonly kinds = KINDS;
  readonly statuses = STATUSES;

  items = signal<SpVerification[]>([]);
  total = signal(0);
  loading = signal(false);
  selected = signal<SpVerification | null>(null);
  searchQuery = signal('');
  selectedStatus = '';
  selectedKind = '';

  readonly limit = 50;
  offset = signal(0);
  hasMore = signal(false);

  showForm = signal(false);
  formTitle = '';
  formKind: VerificationKind = 'test';
  formStatus: VerificationStatus = 'pass';
  formTaskId = '';
  formDetail = '';

  previewTask = signal<SpTask | null>(null);
  previewTaskLoading = signal(false);

  filtered = computed(() => {
    const q = this.searchQuery().toLowerCase().trim();
    if (!q) return this.items();
    return this.items().filter(
      item => item.title.toLowerCase().includes(q) || (item.detail ?? '').toLowerCase().includes(q),
    );
  });

  constructor() {
    effect(() => {
      this.ctx.projectId();
      this.selected.set(null);
      this.offset.set(0);
      this.loadVerifications();
    });
  }

  loadVerifications(): void {
    this.loading.set(true);
    const status = this.selectedStatus as VerificationStatus | '';
    const kind = this.selectedKind as VerificationKind | '';
    this.api
      .list({
        status: status || undefined,
        kind: kind || undefined,
        limit: this.limit,
        offset: this.offset(),
      })
      .subscribe({
        next: res => {
          this.items.set(res.data);
          this.total.set(res.total);
          this.hasMore.set(res.has_more);
          this.loading.set(false);
          if (this.selected()) {
            const still = res.data.find(i => i.id === this.selected()!.id);
            this.selected.set(still ?? null);
          }
        },
        error: () => this.loading.set(false),
      });
  }

  selectItem(item: SpVerification): void {
    this.selected.set(item.id === this.selected()?.id ? null : item);
  }

  statusColor(status: VerificationStatus): string {
    return STATUS_COLORS[status] ?? '';
  }

  kindColor(kind: VerificationKind): string {
    return KIND_COLORS[kind] ?? '';
  }

  objectKeys(obj: Record<string, unknown>): string[] {
    return Object.keys(obj);
  }

  minVal(a: number, b: number): number {
    return Math.min(a, b);
  }

  updateStatus(item: SpVerification, status: VerificationStatus): void {
    this.api.update(item.id, { status }).subscribe({
      next: () => this.loadVerifications(),
    });
  }

  prevPage(): void {
    this.offset.update(v => Math.max(0, v - this.limit));
    this.loadVerifications();
  }

  nextPage(): void {
    this.offset.update(v => v + this.limit);
    this.loadVerifications();
  }

  openCreate(): void {
    this.formTitle = '';
    this.formKind = 'test';
    this.formStatus = 'pass';
    this.formTaskId = '';
    this.formDetail = '';
    this.showForm.set(true);
  }

  closeForm(): void {
    this.showForm.set(false);
  }

  submitForm(): void {
    const data: SpVerificationCreate = {
      kind: this.formKind,
      title: this.formTitle,
      status: this.formStatus,
      task_id: this.formTaskId || undefined,
      detail: this.formDetail || undefined,
    };
    this.api.create(data).subscribe({
      next: () => {
        this.closeForm();
        this.loadVerifications();
      },
    });
  }

  showTaskPreview(taskId: string, event?: Event): void {
    event?.stopPropagation();
    this.previewTaskLoading.set(true);
    this.previewTask.set({} as SpTask); // show modal immediately with loading
    this.tasksApi.get(taskId).subscribe({
      next: task => {
        this.previewTask.set(task);
        this.previewTaskLoading.set(false);
      },
      error: () => {
        this.previewTask.set(null);
        this.previewTaskLoading.set(false);
      },
    });
  }

  closeTaskPreview(): void {
    this.previewTask.set(null);
  }

  navigateToTask(taskId: string): void {
    this.closeTaskPreview();
    this.router.navigate(['/tasks'], { queryParams: { selected: taskId } });
  }
}
