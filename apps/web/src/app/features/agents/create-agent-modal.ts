import { Component, inject, OnInit, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { switchMap } from 'rxjs';
import {
  AgentsApiService,
  SpAgentRegistered,
} from '../../core/services/agents-api.service';
import { SpRole, TeamApiService } from '../../core/services/team-api.service';
import { ModalWrapperComponent } from '../../shared/components/modal-wrapper/modal-wrapper';

const CAPABILITY_PRESETS: Record<string, string[]> = {
  'Full-stack': ['rust', 'typescript', 'angular', 'sql', 'docker', 'code-review'],
  Backend: ['rust', 'sql', 'docker'],
  Frontend: ['typescript', 'angular', 'css'],
};

@Component({
  selector: 'app-create-agent-modal',
  standalone: true,
  imports: [FormsModule, TranslocoModule, ModalWrapperComponent],
  template: `
    <ng-container *transloco="let t">
      <app-modal-wrapper (closed)="onCancel()" maxWidth="max-w-xl" [scrollable]="true">

        <!-- Step 1: Create -->
        @if (!result()) {
          <h2 class="text-lg font-semibold text-text-primary mb-5">{{ t('agents.createTitle') }}</h2>

          <div class="space-y-4">
            <label class="block">
              <span class="block text-sm font-medium text-text-secondary mb-1">
                {{ t('agents.name') }} <span class="text-ctp-red">*</span>
              </span>
              <input type="text" [(ngModel)]="name" [placeholder]="t('agents.namePlaceholder')"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
            </label>

            <div>
              <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('agents.capabilities') }}</span>
              <div class="flex flex-wrap gap-2 mb-2">
                @for (preset of presetNames; track preset) {
                  <button (click)="applyPreset(preset)" type="button"
                    class="px-2.5 py-1 text-xs rounded-lg border transition-colors"
                    [class.border-accent]="activePreset === preset"
                    [class.text-accent]="activePreset === preset"
                    [class.border-border]="activePreset !== preset"
                    [class.text-text-secondary]="activePreset !== preset"
                    [class.hover:border-accent]="activePreset !== preset">
                    {{ preset }}
                  </button>
                }
              </div>
              <input type="text" [(ngModel)]="capsInput" placeholder="rust, typescript, ..."
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
            </div>

            @if (roles().length > 0) {
              <label class="block">
                <span class="block text-sm font-medium text-text-secondary mb-1">
                  {{ t('agents.role') }}
                </span>
                <select [(ngModel)]="selectedRoleId"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent">
                  @for (role of roles(); track role.id) {
                    <option [value]="role.id">{{ role.name }}</option>
                  }
                </select>
                <span class="text-xs text-text-muted mt-1 block">{{ t('agents.roleHint') }}</span>
              </label>
            }

            @if (error()) {
              <p class="text-sm text-ctp-red">{{ error() }}</p>
            }

            <div class="flex gap-3 pt-2">
              <button (click)="onCancel()" type="button"
                class="flex-1 px-4 py-2 text-sm text-text-secondary hover:text-text-primary border border-border
                       rounded-lg hover:bg-surface transition-colors">
                {{ t('common.cancel') }}
              </button>
              <button (click)="onSubmit()" type="button" [disabled]="!name.trim() || saving()"
                class="flex-1 px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg
                       hover:opacity-90 disabled:opacity-50 transition-opacity">
                @if (saving()) {
                  {{ t('common.saving') }}
                } @else {
                  {{ t('agents.create') }}
                }
              </button>
            </div>
          </div>
        }

        <!-- Step 2: Show API key -->
        @if (result(); as r) {
          <h2 class="text-lg font-semibold text-text-primary mb-2">{{ t('agents.created') }}</h2>
          <p class="text-sm text-text-secondary mb-5">{{ t('agents.apiKeyWarning') }}</p>

          <div class="space-y-4">
            <div>
              <span class="block text-xs text-text-muted uppercase tracking-wide mb-1">Agent ID</span>
              <code class="block bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 font-mono select-all">{{ r.id }}</code>
            </div>

            <div>
              <span class="block text-xs text-text-muted uppercase tracking-wide mb-1">API Key</span>
              <div class="flex gap-2">
                <code class="flex-1 bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 font-mono select-all break-all">{{ r.api_key }}</code>
                <button (click)="copyKey(r.api_key)" type="button"
                  class="shrink-0 px-3 py-2 text-sm border border-border rounded-lg hover:bg-surface transition-colors"
                  [class.text-ctp-green]="copied()"
                  [class.text-text-secondary]="!copied()">
                  {{ copied() ? t('common.copied') : t('common.copy') }}
                </button>
              </div>
            </div>

            <div class="bg-bg-subtle rounded-lg p-3 text-sm text-text-secondary">
              <p class="font-medium text-text-primary mb-1">{{ t('agents.envHint') }}</p>
              <code class="block font-mono text-xs mt-1 select-all whitespace-pre">DIRAIGENT_API_URL={{ apiUrl }}
DIRAIGENT_API_TOKEN={{ r.api_key }}
AGENT_ID={{ r.id }}</code>
            </div>

            <button (click)="onDone()" type="button"
              class="w-full px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg
                     hover:opacity-90 transition-opacity">
              {{ t('common.done') }}
            </button>
          </div>
        }

      </app-modal-wrapper>
    </ng-container>
  `,
})
export class CreateAgentModalComponent implements OnInit {
  private api = inject(AgentsApiService);
  private teamApi = inject(TeamApiService);

  created = output<SpAgentRegistered>();
  cancelled = output<void>();

  saving = signal(false);
  error = signal('');
  result = signal<SpAgentRegistered | null>(null);
  copied = signal(false);
  roles = signal<SpRole[]>([]);

  name = '';
  capsInput = 'rust, typescript, angular, sql, docker, code-review';
  activePreset = 'Full-stack';
  presetNames = Object.keys(CAPABILITY_PRESETS);
  selectedRoleId = '';

  ngOnInit(): void {
    this.teamApi.getRoles().subscribe({
      next: (roles) => {
        this.roles.set(roles);
        if (roles.length > 0) {
          this.selectedRoleId = roles[0].id;
        }
      },
    });
  }

  get apiUrl(): string {
    return this.api['baseUrl'] || '';
  }

  applyPreset(preset: string): void {
    this.activePreset = preset;
    this.capsInput = CAPABILITY_PRESETS[preset].join(', ');
  }

  onCancel(): void {
    this.cancelled.emit();
  }

  onSubmit(): void {
    const name = this.name.trim();
    if (!name) return;

    const capabilities = this.capsInput
      .split(',')
      .map(s => s.trim())
      .filter(Boolean);

    this.saving.set(true);
    this.error.set('');

    const roleId = this.selectedRoleId;

    this.api
      .createAgent({
        name,
        capabilities,
        metadata: { model: 'claude-opus-4-6', runtime: 'orchestra' },
      })
      .pipe(
        switchMap((agent) => {
          // Auto-create membership if a role is selected
          if (roleId) {
            return this.teamApi
              .createMember({ agent_id: agent.id, role_id: roleId })
              .pipe(
                // Return the agent regardless of membership result
                switchMap(() => [agent]),
              );
          }
          return [agent];
        }),
      )
      .subscribe({
        next: (agent) => {
          this.saving.set(false);
          this.result.set(agent);
        },
        error: (err) => {
          this.saving.set(false);
          const msg = err?.error?.message || err?.error || 'Failed to create agent';
          this.error.set(typeof msg === 'string' ? msg : JSON.stringify(msg));
        },
      });
  }

  copyKey(key: string): void {
    navigator.clipboard.writeText(key);
    this.copied.set(true);
    setTimeout(() => this.copied.set(false), 2000);
  }

  onDone(): void {
    const r = this.result();
    if (r) this.created.emit(r);
  }
}
