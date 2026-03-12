import { Component, inject, signal, computed, OnInit, ViewChild, input } from '@angular/core';
import { DatePipe, JsonPipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import {
  StepTemplatesApiService,
  SpStepTemplate,
  SpCreateStepTemplate,
  SpUpdateStepTemplate,
} from '../../core/services/step-templates-api.service';
import {
  StepTemplateEditorComponent,
  StepTemplateFormData,
} from './step-template-editor';

@Component({
  selector: 'app-step-templates',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, JsonPipe, StepTemplateEditorComponent],
  template: `
    <div [class]="embedded() ? '' : 'p-3 sm:p-6'" *transloco="let t">
      <!-- Header (hidden when embedded in playbooks page) -->
      @if (!embedded()) {
        <div class="flex items-center justify-between mb-3 sm:mb-6">
          <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.stepTemplates') }}</h1>
          <button (click)="openCreate()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
            {{ t('stepTemplates.create') }}
          </button>
        </div>
      }

      <!-- Editor (inline, above list) -->
      @if (showForm()) {
        <div class="mb-6">
          <app-step-template-editor
            #editor
            [editing]="editing()"
            (saved)="onFormSaved($event)"
            (cancelled)="closeForm()" />
        </div>
      }

      <!-- Search -->
      <div class="mb-6">
        <input
          type="text"
          [placeholder]="t('stepTemplates.searchPlaceholder')"
          [ngModel]="searchQuery()"
          (ngModelChange)="searchQuery.set($event)"
          class="w-full max-w-md bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
      </div>

      <!-- Accordion list -->
      <div class="space-y-2">
        @if (loading()) {
          <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
        } @else if (filtered().length === 0) {
          <p class="text-text-secondary text-sm">{{ t('common.empty') }}</p>
        } @else {
          @for (tpl of filtered(); track tpl.id) {
            <div class="rounded-lg border transition-colors"
              [class]="tpl.id === selected()?.id
                ? 'bg-accent/10 border-accent'
                : 'bg-surface border-border hover:border-accent/50'">
              <!-- Accordion header -->
              <button
                (click)="selectItem(tpl)"
                class="w-full text-left p-4">
                <div class="flex items-center justify-between mb-1">
                  <div class="flex items-center gap-2 min-w-0">
                    <svg class="w-4 h-4 shrink-0 text-text-secondary transition-transform"
                      [class.rotate-90]="tpl.id === selected()?.id"
                      fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
                    </svg>
                    <span class="text-sm font-medium text-text-primary truncate">{{ tpl.name }}</span>
                    @if (!tpl.tenant_id) {
                      <span class="px-1.5 py-0.5 bg-accent/20 text-accent rounded text-xs font-medium shrink-0">
                        {{ t('stepTemplates.defaultBadge') }}
                      </span>
                    }
                  </div>
                  <div class="flex items-center gap-2 shrink-0 ml-2">
                    @if (tpl.model) {
                      <span class="hidden sm:inline text-xs text-text-muted">{{ tpl.model }}</span>
                    }
                    @if (tpl.budget) {
                      <span class="text-xs text-text-secondary">{{ '$' + tpl.budget }}</span>
                    }
                    @if (tpl.allowed_tools) {
                      <span class="text-xs px-2 py-0.5 rounded bg-surface-hover text-text-secondary">
                        {{ tpl.allowed_tools }}
                      </span>
                    }
                  </div>
                </div>
                @if (tpl.tags.length > 0 && tpl.id !== selected()?.id) {
                  <div class="flex flex-wrap gap-1 mt-1 ml-6">
                    @for (tag of tpl.tags; track tag) {
                      <span class="px-1.5 py-0.5 bg-surface-hover text-text-secondary rounded text-xs">{{ tag }}</span>
                    }
                  </div>
                }
              </button>

              <!-- Accordion body (expanded details) -->
              @if (tpl.id === selected()?.id) {
                <div class="border-t border-border px-4 sm:px-6 py-4">
                  <!-- Action buttons -->
                  <div class="flex items-center justify-between mb-4">
                    <div class="flex items-center gap-2">
                      @if (tpl.description) {
                        <p class="text-sm text-text-secondary line-clamp-2">{{ tpl.description }}</p>
                      }
                    </div>
                    <div class="flex gap-2 shrink-0">
                      @if (!tpl.tenant_id) {
                        <button (click)="forkTemplate(tpl); $event.stopPropagation()" class="px-3 py-1.5 text-xs bg-accent/10 text-accent rounded hover:bg-accent/20" [title]="t('stepTemplates.fork')">
                          {{ t('stepTemplates.fork') }}
                        </button>
                      } @else {
                        <button (click)="openEdit(tpl); $event.stopPropagation()" class="p-1.5 text-text-secondary hover:text-accent rounded" [title]="t('stepTemplates.edit')">
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                            <path d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                          </svg>
                        </button>
                        <button (click)="confirmDelete(tpl); $event.stopPropagation()" class="p-1.5 text-text-secondary hover:text-ctp-red rounded" [title]="t('stepTemplates.delete')">
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                            <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                          </svg>
                        </button>
                      }
                    </div>
                  </div>

                  <!-- Tags -->
                  @if (tpl.tags.length > 0) {
                    <div class="flex flex-wrap gap-1 mb-4">
                      @for (tag of tpl.tags; track tag) {
                        <span class="px-2 py-0.5 bg-surface-hover text-text-secondary rounded text-xs">{{ tag }}</span>
                      }
                    </div>
                  }

                  <!-- Properties grid -->
                  <div class="border-t border-border pt-4 mb-4">
                    <h3 class="text-sm font-medium text-text-primary mb-3">{{ t('stepTemplates.properties') }}</h3>
                    <div class="grid grid-cols-2 sm:grid-cols-3 gap-3">
                      @if (tpl.model) {
                        <div class="bg-bg rounded-lg border border-border p-2">
                          <span class="text-xs text-text-muted block">{{ t('stepTemplates.fieldModel') }}</span>
                          <span class="text-sm text-text-primary">{{ tpl.model }}</span>
                        </div>
                      }
                      @if (tpl.budget) {
                        <div class="bg-bg rounded-lg border border-border p-2">
                          <span class="text-xs text-text-muted block">{{ t('stepTemplates.fieldBudget') }}</span>
                          <span class="text-sm text-text-primary">{{ '$' + tpl.budget }}</span>
                        </div>
                      }
                      @if (tpl.allowed_tools) {
                        <div class="bg-bg rounded-lg border border-border p-2">
                          <span class="text-xs text-text-muted block">{{ t('stepTemplates.fieldAllowedTools') }}</span>
                          <span class="text-sm text-text-primary">{{ tpl.allowed_tools }}</span>
                        </div>
                      }
                      @if (tpl.context_level) {
                        <div class="bg-bg rounded-lg border border-border p-2">
                          <span class="text-xs text-text-muted block">{{ t('stepTemplates.fieldContextLevel') }}</span>
                          <span class="text-sm text-text-primary">{{ tpl.context_level }}</span>
                        </div>
                      }
                      @if (tpl.on_complete) {
                        <div class="bg-bg rounded-lg border border-border p-2">
                          <span class="text-xs text-text-muted block">{{ t('stepTemplates.fieldOnComplete') }}</span>
                          <span class="text-sm text-text-primary">{{ tpl.on_complete }}</span>
                        </div>
                      }
                      @if (tpl.retriable !== null) {
                        <div class="bg-bg rounded-lg border border-border p-2">
                          <span class="text-xs text-text-muted block">{{ t('stepTemplates.fieldRetriable') }}</span>
                          <span class="text-sm text-text-primary">{{ tpl.retriable ? '✓' : '✗' }}</span>
                        </div>
                      }
                      @if (tpl.max_cycles) {
                        <div class="bg-bg rounded-lg border border-border p-2">
                          <span class="text-xs text-text-muted block">{{ t('stepTemplates.fieldMaxCycles') }}</span>
                          <span class="text-sm text-text-primary">{{ tpl.max_cycles }}</span>
                        </div>
                      }
                      @if (tpl.timeout_minutes) {
                        <div class="bg-bg rounded-lg border border-border p-2">
                          <span class="text-xs text-text-muted block">{{ t('stepTemplates.fieldTimeout') }}</span>
                          <span class="text-sm text-text-primary">{{ tpl.timeout_minutes }}m</span>
                        </div>
                      }
                      @if (tpl.agent) {
                        <div class="bg-bg rounded-lg border border-border p-2">
                          <span class="text-xs text-text-muted block">{{ t('stepTemplates.fieldAgent') }}</span>
                          <span class="text-sm text-text-primary">{{ tpl.agent }}</span>
                        </div>
                      }
                    </div>
                  </div>

                  <!-- Env -->
                  @if (tpl.env && objectKeys(tpl.env).length > 0) {
                    <div class="border-t border-border pt-4 mb-4">
                      <h3 class="text-sm font-medium text-text-primary mb-2">{{ t('stepTemplates.fieldEnv') }}</h3>
                      <pre class="text-xs text-text-secondary bg-bg rounded-lg border border-border p-3 overflow-x-auto">{{ tpl.env | json }}</pre>
                    </div>
                  }

                  <!-- Vars -->
                  @if (tpl.vars && objectKeys(tpl.vars).length > 0) {
                    <div class="border-t border-border pt-4 mb-4">
                      <h3 class="text-sm font-medium text-text-primary mb-2">{{ t('stepTemplates.fieldVars') }}</h3>
                      <pre class="text-xs text-text-secondary bg-bg rounded-lg border border-border p-3 overflow-x-auto">{{ tpl.vars | json }}</pre>
                    </div>
                  }

                  <!-- MCP Servers -->
                  @if (tpl.mcp_servers && objectKeys(tpl.mcp_servers).length > 0) {
                    <div class="border-t border-border pt-4 mb-4">
                      <h3 class="text-sm font-medium text-text-primary mb-2">{{ t('stepTemplates.fieldMcpServers') }}</h3>
                      <pre class="text-xs text-text-secondary bg-bg rounded-lg border border-border p-3 overflow-x-auto">{{ tpl.mcp_servers | json }}</pre>
                    </div>
                  }

                  <!-- Metadata -->
                  @if (objectKeys(tpl.metadata).length > 0) {
                    <div class="border-t border-border pt-4 mb-4">
                      <h3 class="text-sm font-medium text-text-primary mb-2">{{ t('stepTemplates.metadata') }}</h3>
                      <pre class="text-xs text-text-secondary bg-bg rounded-lg border border-border p-3 overflow-x-auto">{{ tpl.metadata | json }}</pre>
                    </div>
                  }

                  <div class="border-t border-border pt-3 text-xs text-text-secondary">
                    {{ t('stepTemplates.updatedAt') }}: {{ tpl.updated_at | date:'medium' }}
                  </div>
                </div>
              }
            </div>
          }
        }
      </div>
    </div>
  `,
})
export class StepTemplatesPage implements OnInit {
  /** When true, hides the page header/padding (used when embedded as a tab in PlaybooksPage). */
  embedded = input(false);

  private api = inject(StepTemplatesApiService);

  items = signal<SpStepTemplate[]>([]);
  loading = signal(false);
  selected = signal<SpStepTemplate | null>(null);
  searchQuery = signal('');
  showForm = signal(false);
  editing = signal<SpStepTemplate | null>(null);

  @ViewChild('editor') private editorComp?: StepTemplateEditorComponent;

  filtered = computed(() => {
    const q = this.searchQuery().toLowerCase().trim();
    if (!q) return this.items();
    return this.items().filter(
      tpl =>
        tpl.name.toLowerCase().includes(q) ||
        (tpl.description ?? '').toLowerCase().includes(q) ||
        tpl.tags.some(tag => tag.toLowerCase().includes(q)),
    );
  });

  ngOnInit(): void {
    this.loadTemplates();
  }

  loadTemplates(): void {
    this.loading.set(true);
    this.api.list().subscribe({
      next: items => {
        this.items.set(items);
        this.loading.set(false);
        if (this.selected()) {
          const still = items.find(i => i.id === this.selected()!.id);
          this.selected.set(still ?? null);
        }
      },
      error: () => this.loading.set(false),
    });
  }

  selectItem(tpl: SpStepTemplate): void {
    this.selected.set(tpl.id === this.selected()?.id ? null : tpl);
  }

  objectKeys(obj: Record<string, unknown>): string[] {
    return Object.keys(obj);
  }

  openCreate(): void {
    this.editing.set(null);
    this.showForm.set(true);
    // Reset the editor after it renders
    setTimeout(() => this.editorComp?.resetForm());
  }

  openEdit(tpl: SpStepTemplate): void {
    this.editing.set(tpl);
    this.showForm.set(true);
    // Fill the editor after it renders
    setTimeout(() => this.editorComp?.fillForm(tpl));
  }

  closeForm(): void {
    this.showForm.set(false);
    this.editing.set(null);
  }

  onFormSaved(form: StepTemplateFormData): void {
    const tags = form.tags
      .split(',')
      .map(t => t.trim())
      .filter(t => t.length > 0);
    const env = this.entriesToObject(form.envEntries);
    const vars = this.entriesToObject(form.varsEntries);
    const mcp_servers = this.safeParseJson(form.mcpServersJson);
    const agents = this.safeParseJson(form.agentsJson);
    const settings = this.safeParseJson(form.settingsJson);

    if (this.editing()) {
      const data: SpUpdateStepTemplate = {
        name: form.name,
        description: form.description || undefined,
        model: form.model || undefined,
        budget: form.budget ?? undefined,
        allowed_tools: form.allowed_tools || undefined,
        context_level: form.context_level || undefined,
        on_complete: form.on_complete || undefined,
        retriable: form.retriable,
        max_cycles: form.max_cycles ?? undefined,
        timeout_minutes: form.timeout_minutes ?? undefined,
        agent: form.agent || undefined,
        tags,
        env: env ?? undefined,
        vars: vars ?? undefined,
        mcp_servers: mcp_servers ?? undefined,
        agents: agents ?? undefined,
        settings: settings ?? undefined,
      };
      this.api.update(this.editing()!.id, data).subscribe({
        next: () => {
          this.closeForm();
          this.loadTemplates();
        },
      });
    } else {
      const data: SpCreateStepTemplate = {
        name: form.name,
        description: form.description || undefined,
        model: form.model || undefined,
        budget: form.budget ?? undefined,
        allowed_tools: form.allowed_tools || undefined,
        context_level: form.context_level || undefined,
        on_complete: form.on_complete || undefined,
        retriable: form.retriable,
        max_cycles: form.max_cycles ?? undefined,
        timeout_minutes: form.timeout_minutes ?? undefined,
        agent: form.agent || undefined,
        tags,
        env: env ?? undefined,
        vars: vars ?? undefined,
        mcp_servers: mcp_servers ?? undefined,
        agents: agents ?? undefined,
        settings: settings ?? undefined,
      };
      this.api.create(data).subscribe({
        next: () => {
          this.closeForm();
          this.loadTemplates();
        },
      });
    }
  }

  forkTemplate(tpl: SpStepTemplate): void {
    this.api.fork(tpl.id).subscribe({
      next: () => this.loadTemplates(),
    });
  }

  confirmDelete(tpl: SpStepTemplate): void {
    this.api.delete(tpl.id).subscribe({
      next: () => {
        this.selected.set(null);
        this.loadTemplates();
      },
    });
  }

  private entriesToObject(
    entries: { key: string; value: string }[],
  ): Record<string, string> | undefined {
    const filtered = entries.filter(e => e.key.trim());
    if (filtered.length === 0) return undefined;
    const obj: Record<string, string> = {};
    for (const e of filtered) {
      obj[e.key.trim()] = e.value;
    }
    return obj;
  }

  private safeParseJson(str: string): Record<string, unknown> | undefined {
    if (!str.trim()) return undefined;
    try {
      return JSON.parse(str);
    } catch {
      return undefined;
    }
  }
}
