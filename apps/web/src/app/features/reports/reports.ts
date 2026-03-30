import { Component, inject, signal, computed } from '@angular/core';
import { DatePipe, SlicePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import {
  ReportsApiService,
  SpReport,
  ReportStatus,
  ReportKind,
} from '../../core/services/reports-api.service';
import { CrudFeatureBase } from '../../shared/crud-feature-base';
import { FilterBarComponent } from '../../shared/components/filter-bar/filter-bar';
import { ListDetailLayoutComponent } from '../../shared/components/list-detail-layout/list-detail-layout';
import { ModalWrapperComponent } from '../../shared/components/modal-wrapper/modal-wrapper';
import { ConfirmDialogComponent } from '../../shared/components/confirm-dialog/confirm-dialog';
import {
  REPORT_STATUS_COLORS, REPORT_KIND_COLORS,
} from '../../shared/ui-constants';

const STATUSES: ReportStatus[] = ['pending', 'in_progress', 'completed', 'failed'];
const KINDS: ReportKind[] = ['security', 'component', 'architecture', 'performance', 'custom'];

@Component({
  selector: 'app-reports',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, SlicePipe, RouterLink, FilterBarComponent, ListDetailLayoutComponent, ModalWrapperComponent, ConfirmDialogComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.reports') }}</h1>
        <button (click)="openCreate()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
          {{ t('reports.create') }}
        </button>
      </div>

      <!-- Filters -->
      <app-filter-bar
        [placeholder]="t('reports.searchPlaceholder')"
        [query]="searchQuery()"
        (queryChange)="searchQuery.set($event)">
        <select
          [(ngModel)]="selectedStatus"
          (ngModelChange)="loadItems()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('reports.allStatuses') }}</option>
          @for (s of statuses; track s) {
            <option [value]="s">{{ t('reports.status.' + s) }}</option>
          }
        </select>
      </app-filter-bar>

      <!-- Content: list + detail -->
      <app-list-detail-layout>
        <div list>
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
                      {{ t('reports.status.' + item.status) }}
                    </span>
                    <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ kindColor(item.kind) }}">
                      {{ t('reports.kind.' + item.kind) }}
                    </span>
                    <span class="text-sm font-medium text-text-primary">
                      {{ item.title }}
                    </span>
                  </div>
                  <div class="flex items-center gap-2 mt-2 text-xs text-text-secondary">
                    <span>{{ item.created_at | date:'short' }}</span>
                  </div>
                </button>
              }
            </div>
          }
        </div>

        <!-- Detail panel -->
        @if (selected()) {
          <div detail class="w-full lg:w-[520px] shrink-0 bg-surface rounded-lg border border-border p-4 sm:p-6 max-h-[calc(100vh-200px)] overflow-y-auto overflow-x-hidden min-w-0">
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
                {{ t('reports.status.' + selected()!.status) }}
              </span>
              <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ kindColor(selected()!.kind) }}">
                {{ t('reports.kind.' + selected()!.kind) }}
              </span>
            </div>

            <!-- Actions -->
            <div class="flex gap-2 mb-4">
              <button (click)="confirmDelete(selected()!)"
                class="px-3 py-1.5 text-xs font-medium bg-ctp-red/20 text-ctp-red rounded-lg hover:bg-ctp-red/30">
                {{ t('reports.delete') }}
              </button>
            </div>

            <!-- Prompt -->
            @if (selected()!.prompt) {
              <div class="mb-4">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('reports.fieldPrompt') }}</h3>
                <p class="text-sm text-text-primary whitespace-pre-wrap">{{ selected()!.prompt }}</p>
              </div>
            }

            <!-- Linked Task -->
            @if (selected()!.task_id) {
              <div class="mb-4">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('reports.linkedTask') }}</h3>
                <a [routerLink]="['/tasks']" [queryParams]="{ id: selected()!.task_id }"
                  class="text-sm text-accent hover:underline">
                  {{ selected()!.task_id | slice:0:13 }}
                </a>
              </div>
            }

            <!-- Result -->
            <div class="mb-4">
              <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-1">{{ t('reports.fieldResult') }}</h3>
              @if (selected()!.result) {
                <pre class="text-sm text-text-primary bg-bg rounded-lg p-3 border border-border overflow-x-auto whitespace-pre-wrap">{{ selected()!.result }}</pre>
              } @else {
                <p class="text-sm text-text-secondary italic">{{ t('reports.noResult') }}</p>
              }
            </div>

            <div class="pt-3 border-t border-border text-xs text-text-secondary">
              {{ t('reports.createdAt') }}: {{ selected()!.created_at | date:'medium' }}
            </div>
          </div>
        }
      </app-list-detail-layout>

      <!-- Create modal -->
      @if (showForm()) {
        <app-modal-wrapper maxWidth="max-w-lg" [scrollable]="true" (closed)="closeForm()">
          <h2 class="text-lg font-semibold text-text-primary mb-4">{{ t('reports.createTitle') }}</h2>
          <div class="space-y-4">
            <div>
              <label for="report-title" class="block text-sm text-text-secondary mb-1">{{ t('reports.fieldTitle') }}</label>
              <input id="report-title" type="text" [(ngModel)]="formTitle"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent" />
            </div>
            <div>
              <label for="report-kind" class="block text-sm text-text-secondary mb-1">{{ t('reports.fieldKind') }}</label>
              <select id="report-kind" [(ngModel)]="formKind"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent">
                @for (k of kinds; track k) {
                  <option [value]="k">{{ t('reports.kind.' + k) }}</option>
                }
              </select>
            </div>
            <div>
              <label for="report-prompt" class="block text-sm text-text-secondary mb-1">{{ t('reports.fieldPrompt') }}</label>
              <textarea id="report-prompt" [(ngModel)]="formPrompt" rows="6"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent resize-y"
                [placeholder]="t('reports.promptPlaceholder')"></textarea>
            </div>
            <div class="flex justify-end gap-3 pt-2">
              <button (click)="closeForm()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                {{ t('reports.cancel') }}
              </button>
              <button (click)="submitForm()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
                {{ t('reports.requestReport') }}
              </button>
            </div>
          </div>
        </app-modal-wrapper>
      }

      <!-- Delete confirmation -->
      @if (showDeleteConfirm()) {
        <app-confirm-dialog
          [title]="t('reports.deleteConfirmTitle')"
          [message]="t('reports.deleteConfirmMessage')"
          [cancelLabel]="t('reports.cancel')"
          [confirmLabel]="t('reports.delete')"
          confirmClass="bg-ctp-red text-ctp-base"
          (confirmed)="executeDelete()"
          (cancelled)="closeDeleteConfirm()" />
      }
    </div>
  `,
})
export class ReportsPage extends CrudFeatureBase<SpReport> {
  private api = inject(ReportsApiService);

  readonly statuses = STATUSES;
  readonly kinds = KINDS;

  selectedStatus = '';

  formTitle = '';
  formKind: ReportKind = 'custom';
  formPrompt = '';

  showDeleteConfirm = signal(false);
  deleteTarget: SpReport | null = null;

  filtered = computed(() => {
    const q = this.searchQuery().toLowerCase().trim();
    let result = this.items();
    if (q) {
      result = result.filter(
        item => item.title.toLowerCase().includes(q) || (item.prompt ?? '').toLowerCase().includes(q),
      );
    }
    return result.slice().sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime());
  });

  statusColor(status: ReportStatus): string {
    return REPORT_STATUS_COLORS[status] ?? '';
  }

  kindColor(kind: string): string {
    return REPORT_KIND_COLORS[kind as ReportKind] ?? 'bg-ctp-overlay0/20 text-ctp-overlay0';
  }

  confirmDelete(item: SpReport): void {
    this.deleteTarget = item;
    this.showDeleteConfirm.set(true);
  }

  closeDeleteConfirm(): void {
    this.showDeleteConfirm.set(false);
    this.deleteTarget = null;
  }

  executeDelete(): void {
    if (!this.deleteTarget) return;
    this.api.delete(this.deleteTarget.id).subscribe({
      next: () => {
        this.closeDeleteConfirm();
        if (this.selected()?.id === this.deleteTarget?.id) {
          this.selected.set(null);
        }
        this.loadItems();
      },
    });
  }

  protected override resetForm(): void {
    this.formTitle = '';
    this.formKind = 'custom';
    this.formPrompt = '';
  }

  protected override fillForm(_item: SpReport): void {
    // Reports only support create, not edit
  }

  submitForm(): void {
    const data = {
      title: this.formTitle,
      kind: this.formKind,
      prompt: this.formPrompt,
    };
    this.api.create(data).subscribe({
      next: () => {
        this.closeForm();
        this.loadItems();
      },
    });
  }

  override loadItems(): void {
    this.loading.set(true);
    const status = this.selectedStatus as ReportStatus | '';
    this.api.list(status || undefined).subscribe({
      next: (items) => this.refreshAfterMutation(items),
      error: () => this.loading.set(false),
    });
  }
}
