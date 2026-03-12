import { Component, EventEmitter, Input, Output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { SpStepTemplate } from '../../core/services/step-templates-api.service';

export interface StepTemplateFormData {
  name: string;
  description: string;
  model: string;
  budget: number | null;
  allowed_tools: string;
  context_level: string;
  on_complete: string;
  retriable: boolean;
  max_cycles: number | null;
  timeout_minutes: number | null;
  agent: string;
  tags: string;
  envEntries: { key: string; value: string }[];
  varsEntries: { key: string; value: string }[];
  mcpServersJson: string;
  agentsJson: string;
  settingsJson: string;
}

@Component({
  selector: 'app-step-template-editor',
  standalone: true,
  imports: [TranslocoModule, FormsModule],
  template: `
    <div class="border border-border rounded-lg bg-surface p-4 sm:p-6" *transloco="let t">
      <h2 class="text-lg font-semibold text-text-primary mb-4">
        {{ editing ? t('stepTemplates.editTitle') : t('stepTemplates.createTitle') }}
      </h2>

      <!-- Name -->
      <div class="mb-4">
        <label for="ste-name" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldName') }}</label>
        <input
          id="ste-name"
          type="text"
          [(ngModel)]="form.name"
          [placeholder]="t('stepTemplates.namePlaceholder')"
          class="w-full bg-bg text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent" />
      </div>

      <!-- Description -->
      <div class="mb-4">
        <label for="ste-description" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldDescription') }}</label>
        <textarea
          id="ste-description"
          [(ngModel)]="form.description"
          [placeholder]="t('stepTemplates.descriptionPlaceholder')"
          rows="4"
          class="w-full bg-bg text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
      </div>

      <!-- Model + Budget row -->
      <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-4">
        <div>
          <label for="ste-model" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldModel') }}</label>
          <select
            id="ste-model"
            [(ngModel)]="form.model"
            class="w-full bg-bg text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent">
            <option value="">{{ t('playbooks.modelDefault') }}</option>
            <option value="claude-opus-4-6">claude-opus-4-6</option>
            <option value="claude-sonnet-4-6">claude-sonnet-4-6</option>
            <option value="claude-haiku-3-5">claude-haiku-3-5</option>
          </select>
        </div>
        <div>
          <label for="ste-budget" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldBudget') }}</label>
          <input
            id="ste-budget"
            type="number"
            [(ngModel)]="form.budget"
            min="0"
            step="0.5"
            placeholder="12.0"
            class="w-full bg-bg text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent" />
        </div>
      </div>

      <!-- Allowed Tools + Context Level row -->
      <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-4">
        <div>
          <label for="ste-allowed-tools" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldAllowedTools') }}</label>
          <select
            id="ste-allowed-tools"
            [(ngModel)]="form.allowed_tools"
            class="w-full bg-bg text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent">
            <option value="">—</option>
            <option value="full">full</option>
            <option value="readonly">readonly</option>
          </select>
        </div>
        <div>
          <label for="ste-context-level" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldContextLevel') }}</label>
          <select
            id="ste-context-level"
            [(ngModel)]="form.context_level"
            class="w-full bg-bg text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent">
            <option value="">—</option>
            <option value="full">full</option>
            <option value="minimal">minimal</option>
            <option value="dream">dream</option>
          </select>
        </div>
      </div>

      <!-- On Complete + Retriable + Max Cycles row -->
      <div class="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-4">
        <div>
          <label for="ste-on-complete" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldOnComplete') }}</label>
          <input
            id="ste-on-complete"
            type="text"
            [(ngModel)]="form.on_complete"
            placeholder="next"
            class="w-full bg-bg text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent" />
        </div>
        <div>
          <label for="ste-max-cycles" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldMaxCycles') }}</label>
          <input
            id="ste-max-cycles"
            type="number"
            [(ngModel)]="form.max_cycles"
            min="0"
            placeholder="3"
            class="w-full bg-bg text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent" />
        </div>
        <div>
          <label for="ste-timeout" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldTimeout') }}</label>
          <input
            id="ste-timeout"
            type="number"
            [(ngModel)]="form.timeout_minutes"
            min="0"
            placeholder="30"
            class="w-full bg-bg text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent" />
        </div>
      </div>

      <!-- Retriable checkbox -->
      <div class="mb-4">
        <label class="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
          <input
            type="checkbox"
            [(ngModel)]="form.retriable"
            class="rounded border-border text-accent focus:ring-accent" />
          {{ t('stepTemplates.fieldRetriable') }}
        </label>
      </div>

      <!-- Agent -->
      <div class="mb-4">
        <label for="ste-agent" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldAgent') }}</label>
        <input
          id="ste-agent"
          type="text"
          [(ngModel)]="form.agent"
          [placeholder]="t('stepTemplates.agentPlaceholder')"
          class="w-full bg-bg text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent" />
      </div>

      <!-- Tags -->
      <div class="mb-4">
        <label for="ste-tags" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldTags') }}</label>
        <input
          id="ste-tags"
          type="text"
          [(ngModel)]="form.tags"
          [placeholder]="t('stepTemplates.tagsPlaceholder')"
          class="w-full bg-bg text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent" />
      </div>

      <!-- Advanced section toggle -->
      <button
        (click)="showAdvanced.set(!showAdvanced())"
        class="flex items-center gap-1 text-sm text-accent hover:underline mb-4">
        <svg class="w-4 h-4 transition-transform" [class.rotate-90]="showAdvanced()"
          fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
        </svg>
        {{ t('stepTemplates.advanced') }}
      </button>

      @if (showAdvanced()) {
        <!-- Env key-value pairs -->
        <div class="mb-4">
          <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldEnv') }}</span>
          @for (entry of form.envEntries; track $index; let i = $index) {
            <div class="flex gap-2 mb-1">
              <input
                type="text"
                [(ngModel)]="entry.key"
                placeholder="KEY"
                class="flex-1 bg-bg text-text-primary text-xs rounded px-2 py-1.5 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent font-mono" />
              <input
                type="text"
                [(ngModel)]="entry.value"
                placeholder="value"
                class="flex-1 bg-bg text-text-primary text-xs rounded px-2 py-1.5 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent font-mono" />
              <button (click)="removeEntry(form.envEntries, i)" class="text-text-secondary hover:text-ctp-red p-1">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          }
          <button (click)="addEntry(form.envEntries)" class="text-xs text-accent hover:underline">
            + {{ t('stepTemplates.addEntry') }}
          </button>
        </div>

        <!-- Vars key-value pairs -->
        <div class="mb-4">
          <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldVars') }}</span>
          @for (entry of form.varsEntries; track $index; let i = $index) {
            <div class="flex gap-2 mb-1">
              <input
                type="text"
                [(ngModel)]="entry.key"
                placeholder="KEY"
                class="flex-1 bg-bg text-text-primary text-xs rounded px-2 py-1.5 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent font-mono" />
              <input
                type="text"
                [(ngModel)]="entry.value"
                placeholder="value"
                class="flex-1 bg-bg text-text-primary text-xs rounded px-2 py-1.5 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent font-mono" />
              <button (click)="removeEntry(form.varsEntries, i)" class="text-text-secondary hover:text-ctp-red p-1">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                  <path d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          }
          <button (click)="addEntry(form.varsEntries)" class="text-xs text-accent hover:underline">
            + {{ t('stepTemplates.addEntry') }}
          </button>
        </div>

        <!-- MCP Servers (JSON) -->
        <div class="mb-4">
          <label for="ste-mcp-servers" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldMcpServers') }}</label>
          <textarea
            id="ste-mcp-servers"
            [(ngModel)]="form.mcpServersJson"
            placeholder="{}"
            rows="3"
            class="w-full bg-bg text-text-primary text-xs rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent resize-y font-mono"></textarea>
        </div>

        <!-- Agents (JSON) -->
        <div class="mb-4">
          <label for="ste-agents-json" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldAgents') }}</label>
          <textarea
            id="ste-agents-json"
            [(ngModel)]="form.agentsJson"
            placeholder="{}"
            rows="3"
            class="w-full bg-bg text-text-primary text-xs rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent resize-y font-mono"></textarea>
        </div>

        <!-- Settings (JSON) -->
        <div class="mb-4">
          <label for="ste-settings" class="block text-sm font-medium text-text-secondary mb-1">{{ t('stepTemplates.fieldSettings') }}</label>
          <textarea
            id="ste-settings"
            [(ngModel)]="form.settingsJson"
            placeholder="{}"
            rows="3"
            class="w-full bg-bg text-text-primary text-xs rounded-lg px-3 py-2 border border-border
                   focus:outline-none focus:ring-1 focus:ring-accent resize-y font-mono"></textarea>
        </div>
      }

      <!-- Actions -->
      <div class="flex justify-end gap-2 pt-4 border-t border-border">
        <button (click)="cancelled.emit()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
          {{ t('common.cancel') }}
        </button>
        <button
          (click)="onSave()"
          [disabled]="!form.name.trim()"
          class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90
                 disabled:opacity-50 disabled:cursor-not-allowed">
          {{ t('common.save') }}
        </button>
      </div>
    </div>
  `,
})
export class StepTemplateEditorComponent {
  @Input() editing: SpStepTemplate | null = null;
  @Output() saved = new EventEmitter<StepTemplateFormData>();
  @Output() cancelled = new EventEmitter<void>();

  showAdvanced = signal(false);

  form: StepTemplateFormData = this.emptyForm();

  private emptyForm(): StepTemplateFormData {
    return {
      name: '',
      description: '',
      model: '',
      budget: null,
      allowed_tools: '',
      context_level: '',
      on_complete: '',
      retriable: false,
      max_cycles: null,
      timeout_minutes: null,
      agent: '',
      tags: '',
      envEntries: [],
      varsEntries: [],
      mcpServersJson: '',
      agentsJson: '',
      settingsJson: '',
    };
  }

  resetForm(): void {
    this.form = this.emptyForm();
    this.showAdvanced.set(false);
  }

  fillForm(item: SpStepTemplate): void {
    this.form = {
      name: item.name,
      description: item.description ?? '',
      model: item.model ?? '',
      budget: item.budget,
      allowed_tools: item.allowed_tools ?? '',
      context_level: item.context_level ?? '',
      on_complete: item.on_complete ?? '',
      retriable: item.retriable ?? false,
      max_cycles: item.max_cycles,
      timeout_minutes: item.timeout_minutes,
      agent: item.agent ?? '',
      tags: item.tags.join(', '),
      envEntries: this.objectToEntries(item.env),
      varsEntries: this.objectToEntries(item.vars),
      mcpServersJson: item.mcp_servers ? JSON.stringify(item.mcp_servers, null, 2) : '',
      agentsJson: item.agents ? JSON.stringify(item.agents, null, 2) : '',
      settingsJson: item.settings ? JSON.stringify(item.settings, null, 2) : '',
    };
    // Show advanced if any advanced fields are populated
    if (
      this.form.envEntries.length > 0 ||
      this.form.varsEntries.length > 0 ||
      this.form.mcpServersJson ||
      this.form.agentsJson ||
      this.form.settingsJson
    ) {
      this.showAdvanced.set(true);
    }
  }

  addEntry(entries: { key: string; value: string }[]): void {
    entries.push({ key: '', value: '' });
  }

  removeEntry(entries: { key: string; value: string }[], index: number): void {
    entries.splice(index, 1);
  }

  onSave(): void {
    this.saved.emit(this.form);
  }

  private objectToEntries(
    obj: Record<string, string> | null | undefined,
  ): { key: string; value: string }[] {
    if (!obj) return [];
    return Object.entries(obj).map(([key, value]) => ({ key, value }));
  }
}
