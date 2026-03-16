import { Component, inject, signal, computed, effect } from '@angular/core';
import { DatePipe, JsonPipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { AuditApiService, SpAuditEntry } from '../../core/services/audit-api.service';
import { ProjectContext } from '../../core/services/project-context.service';
import { AUDIT_ENTITY_TYPE_COLORS, AUDIT_ACTION_COLORS } from '../../shared/ui-constants';

const ENTITY_TYPE_COLORS = AUDIT_ENTITY_TYPE_COLORS;
const ACTION_COLORS = AUDIT_ACTION_COLORS;

@Component({
  selector: 'app-audit',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, JsonPipe],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.audit') }}</h1>
      </div>

      <!-- Filters -->
      <div class="flex flex-wrap gap-3 mb-6">
        <input
          type="text"
          [placeholder]="t('audit.searchPlaceholder')"
          [ngModel]="searchQuery()"
          (ngModelChange)="searchQuery.set($event)"
          class="flex-1 min-w-[200px] bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
        <select
          [(ngModel)]="selectedEntityType"
          (ngModelChange)="loadAudit()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('audit.allEntityTypes') }}</option>
          @for (et of entityTypes(); track et) {
            <option [value]="et">{{ et }}</option>
          }
        </select>
        <!-- Entity-specific history lookup -->
        <div class="flex flex-wrap gap-2">
          <input
            type="text"
            [placeholder]="t('audit.entityIdPlaceholder')"
            [(ngModel)]="entityIdQuery"
            class="flex-1 min-w-[180px] max-w-[280px] bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
          <button (click)="loadEntityHistory()"
            [disabled]="!selectedEntityType || !entityIdQuery"
            class="px-3 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50">
            {{ t('audit.loadHistory') }}
          </button>
          @if (historyMode()) {
            <button (click)="clearHistory()"
              class="px-3 py-2 text-sm text-text-secondary hover:text-text-primary border border-border rounded-lg">
              {{ t('audit.clearHistory') }}
            </button>
          }
        </div>
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
            <div class="space-y-1">
              @for (entry of filtered(); track entry.id) {
                <button
                  (click)="selectEntry(entry)"
                  class="w-full text-left px-4 py-3 rounded-lg border transition-colors"
                  [class]="entry.id === selected()?.id
                    ? 'bg-accent/10 border-accent'
                    : 'bg-surface border-border hover:border-accent/50'">
                  <div class="flex items-center gap-2">
                    <span class="text-xs text-text-secondary whitespace-nowrap">{{ entry.created_at | date:'short' }}</span>
                    <span class="text-xs font-medium {{ actionColor(entry.action) }}">{{ t('audit.action.' + entry.action) }}</span>
                    <span class="px-1.5 py-0.5 rounded text-xs font-medium {{ entityTypeColor(entry.entity_type) }}">
                      {{ entry.entity_type }}
                    </span>
                    <span class="text-sm text-text-primary truncate">{{ entry.summary }}</span>
                  </div>
                  @if (entry.actor_name) {
                    <div class="text-xs text-text-secondary mt-1">{{ t('audit.actor') }}: {{ entry.actor_name }}</div>
                  }
                </button>
              }
            </div>
          }
        </div>

        <!-- Detail panel -->
        @if (selected()) {
          <div class="w-full lg:w-[560px] shrink-0 bg-surface rounded-lg border border-border p-4 sm:p-6 max-h-[calc(100vh-200px)] overflow-y-auto overflow-x-hidden min-w-0">
            <div class="flex items-center justify-between mb-3">
              <h2 class="text-lg font-semibold text-text-primary">{{ t('audit.entryDetail') }}</h2>
              <button (click)="selected.set(null)" class="p-1.5 text-text-secondary hover:text-text-primary rounded">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            <!-- Meta info -->
            <div class="space-y-2 mb-4">
              <div class="flex items-center gap-2">
                <span class="text-xs font-medium {{ actionColor(selected()!.action) }}">{{ t('audit.action.' + selected()!.action) }}</span>
                <span class="px-1.5 py-0.5 rounded text-xs font-medium {{ entityTypeColor(selected()!.entity_type) }}">
                  {{ selected()!.entity_type }}
                </span>
              </div>
              <p class="text-sm text-text-primary">{{ selected()!.summary }}</p>
              <div class="text-xs text-text-secondary">
                <span>{{ t('audit.timestamp') }}: {{ selected()!.created_at | date:'medium' }}</span>
              </div>
              @if (selected()!.actor_name) {
                <div class="text-xs text-text-secondary">{{ t('audit.actor') }}: {{ selected()!.actor_name }}</div>
              }
              <div class="text-xs text-text-secondary break-all">{{ t('audit.entityId') }}: {{ selected()!.entity_id }}</div>
            </div>

            <!-- Before/After diff -->
            @if (selected()!.before_state || selected()!.after_state) {
              <div class="border-t border-border pt-4">
                <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-3">{{ t('audit.stateChanges') }}</h3>
                <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  <!-- Before -->
                  <div>
                    <h4 class="text-xs font-medium text-ctp-red mb-1">{{ t('audit.before') }}</h4>
                    @if (selected()!.before_state) {
                      <pre class="text-xs text-text-primary bg-bg rounded-lg p-3 border border-border overflow-x-auto max-h-[400px] overflow-y-auto">{{ selected()!.before_state | json }}</pre>
                    } @else {
                      <p class="text-xs text-text-secondary italic">{{ t('audit.noState') }}</p>
                    }
                  </div>
                  <!-- After -->
                  <div>
                    <h4 class="text-xs font-medium text-ctp-green mb-1">{{ t('audit.after') }}</h4>
                    @if (selected()!.after_state) {
                      <pre class="text-xs text-text-primary bg-bg rounded-lg p-3 border border-border overflow-x-auto max-h-[400px] overflow-y-auto">{{ selected()!.after_state | json }}</pre>
                    } @else {
                      <p class="text-xs text-text-secondary italic">{{ t('audit.noState') }}</p>
                    }
                  </div>
                </div>
              </div>
            }
          </div>
        }
      </div>
    </div>
  `,
})
export class AuditPage {
  private api = inject(AuditApiService);
  private ctx = inject(ProjectContext);

  items = signal<SpAuditEntry[]>([]);
  loading = signal(false);
  selected = signal<SpAuditEntry | null>(null);
  searchQuery = signal('');
  selectedEntityType = '';
  entityIdQuery = '';
  historyMode = signal(false);

  entityTypes = computed(() => {
    const types = new Set<string>();
    for (const item of this.items()) {
      types.add(item.entity_type);
    }
    return [...types].sort();
  });

  filtered = computed(() => {
    const q = this.searchQuery().toLowerCase().trim();
    if (!q) return this.items();
    return this.items().filter(
      item => item.summary.toLowerCase().includes(q) || item.entity_type.toLowerCase().includes(q),
    );
  });

  constructor() {
    effect(() => {
      this.ctx.projectId();
      this.selected.set(null);
      this.loadAudit();
    });
  }

  loadAudit(): void {
    this.historyMode.set(false);
    this.loading.set(true);
    this.api.list(this.selectedEntityType || undefined).subscribe({
      next: (items) => {
        this.items.set(items);
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });
  }

  loadEntityHistory(): void {
    if (!this.selectedEntityType || !this.entityIdQuery) return;
    this.loading.set(true);
    this.historyMode.set(true);
    this.api.entityHistory(this.selectedEntityType, this.entityIdQuery.trim()).subscribe({
      next: (items) => {
        this.items.set(items);
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });
  }

  clearHistory(): void {
    this.entityIdQuery = '';
    this.loadAudit();
  }

  selectEntry(entry: SpAuditEntry): void {
    this.selected.set(entry.id === this.selected()?.id ? null : entry);
  }

  entityTypeColor(type: string): string {
    return ENTITY_TYPE_COLORS[type] ?? 'bg-ctp-overlay0/20 text-ctp-overlay0';
  }

  actionColor(action: string): string {
    return ACTION_COLORS[action] ?? 'text-text-secondary';
  }
}
