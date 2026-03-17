import { Component, inject, OnInit, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { catchError, of, switchMap } from 'rxjs';
import {
  AgentsApiService,
  SpAgentRegistered,
} from '../../core/services/agents-api.service';
import { SpRole, TeamApiService } from '../../core/services/team-api.service';
import { TenantApiService } from '../../core/services/tenant-api.service';
import { ProviderConfigsApiService, ProviderConfig } from '../../core/services/provider-configs-api.service';
import { ModalWrapperComponent } from '../../shared/components/modal-wrapper/modal-wrapper';

const CAPABILITY_PRESETS: Record<string, string[]> = {
  'Full-stack': ['rust', 'typescript', 'angular', 'sql', 'docker', 'code-review'],
  Backend: ['rust', 'sql', 'docker'],
  Frontend: ['typescript', 'angular', 'css'],
};

type OnboardingStep = 'details' | 'provider' | 'credentials';

const STEP_LIST: OnboardingStep[] = ['details', 'provider', 'credentials'];

@Component({
  selector: 'app-create-agent-modal',
  standalone: true,
  imports: [FormsModule, TranslocoModule, ModalWrapperComponent],
  template: `
    <ng-container *transloco="let t">
      <app-modal-wrapper (closed)="onCancel()" maxWidth="max-w-xl" [scrollable]="true">

        <!-- Progress bar -->
        <div class="flex items-center gap-2 mb-6">
          @for (s of stepList; track s; let i = $index) {
            <div class="flex items-center gap-2" [class.flex-1]="i < stepList.length - 1">
              <div class="w-7 h-7 rounded-full flex items-center justify-center text-xs font-semibold shrink-0 transition-colors"
                [class.bg-accent]="stepIndex() >= i"
                [class.text-bg]="stepIndex() >= i"
                [class.bg-surface]="stepIndex() < i"
                [class.text-text-secondary]="stepIndex() < i"
                [class.border]="stepIndex() < i"
                [class.border-border]="stepIndex() < i">
                @if (stepIndex() > i) {
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>
                  </svg>
                } @else {
                  {{ i + 1 }}
                }
              </div>
              @if (i < stepList.length - 1) {
                <div class="flex-1 h-0.5 rounded transition-colors"
                  [class.bg-accent]="stepIndex() > i"
                  [class.bg-border]="stepIndex() <= i"></div>
              }
            </div>
          }
        </div>

        <!-- Step 1: Agent Details -->
        @if (step() === 'details') {
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

            @if (error()) {
              <p class="text-sm text-ctp-red">{{ error() }}</p>
            }

            <div class="flex gap-3 pt-2">
              <button (click)="onCancel()" type="button"
                class="flex-1 px-4 py-2 text-sm text-text-secondary hover:text-text-primary border border-border
                       rounded-lg hover:bg-surface transition-colors">
                {{ t('common.cancel') }}
              </button>
              <button (click)="onSubmitDetails()" type="button" [disabled]="!name.trim() || saving()"
                class="flex-1 px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg
                       hover:opacity-90 disabled:opacity-50 transition-opacity">
                @if (saving()) {
                  {{ t('common.saving') }}
                } @else {
                  {{ t('common.next') }}
                }
              </button>
            </div>
          </div>
        }

        <!-- Step 2: Provider Setup -->
        @if (step() === 'provider') {
          <h2 class="text-lg font-semibold text-text-primary mb-2">{{ t('agents.providerTitle') }}</h2>
          <p class="text-sm text-text-secondary mb-5">{{ t('agents.providerHint') }}</p>

          @if (hasExistingProvider()) {
            <div class="bg-ctp-green/10 border border-ctp-green/30 rounded-lg p-3 mb-4">
              <p class="text-sm text-ctp-green">{{ t('agents.providerAlreadyConfigured') }}</p>
            </div>
          }

          <div class="space-y-4">
            <label class="block">
              <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('agents.provider') }}</span>
              <select [(ngModel)]="providerName"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent">
                @for (opt of providerOptions; track opt) {
                  <option [value]="opt">{{ opt }}</option>
                }
              </select>
            </label>

            <label class="block">
              <span class="block text-sm font-medium text-text-secondary mb-1">
                {{ t('agents.apiKey') }} @if (!hasExistingProvider()) { <span class="text-ctp-red">*</span> }
              </span>
              <input type="password" [(ngModel)]="providerApiKey" placeholder="sk-..."
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
            </label>

            <label class="block">
              <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('agents.baseUrl') }}</span>
              <input type="text" [(ngModel)]="providerBaseUrl" placeholder="https://api.openai.com"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
              <span class="text-xs text-text-muted mt-1 block">{{ t('agents.baseUrlHint') }}</span>
            </label>

            @if (providerError()) {
              <p class="text-sm text-ctp-red">{{ providerError() }}</p>
            }

            <div class="flex gap-3 pt-2">
              <button (click)="skipProvider()" type="button"
                class="flex-1 px-4 py-2 text-sm text-text-secondary hover:text-text-primary border border-border
                       rounded-lg hover:bg-surface transition-colors">
                @if (hasExistingProvider()) {
                  {{ t('common.skip') }}
                } @else {
                  {{ t('agents.skipProvider') }}
                }
              </button>
              <button (click)="saveProvider()" type="button" [disabled]="!providerApiKey.trim() || savingProvider()"
                class="flex-1 px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg
                       hover:opacity-90 disabled:opacity-50 transition-opacity">
                @if (savingProvider()) {
                  {{ t('common.saving') }}
                } @else {
                  {{ t('common.save') }}
                }
              </button>
            </div>
          </div>
        }

        <!-- Step 3: Credentials -->
        @if (step() === 'credentials') {
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
AGENT_ID={{ r.id }}@if (dek()) {

DIRAIGENT_DEK={{ dek() }}}</code>
              </div>
              @if (dek()) {
                <p class="text-xs text-ctp-yellow mt-1">{{ t('agents.dekWarning') }}</p>
              }

              <button (click)="onDone()" type="button"
                class="w-full px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg
                       hover:opacity-90 transition-opacity">
                {{ t('common.done') }}
              </button>
            </div>
          }
        }

      </app-modal-wrapper>
    </ng-container>
  `,
})
export class CreateAgentModalComponent implements OnInit {
  private api = inject(AgentsApiService);
  private teamApi = inject(TeamApiService);
  private tenantApi = inject(TenantApiService);
  private providerApi = inject(ProviderConfigsApiService);

  created = output<SpAgentRegistered>();
  cancelled = output<void>();

  // Step management
  step = signal<OnboardingStep>('details');
  stepIndex = signal(0);
  readonly stepList = STEP_LIST;

  // Step 1: Agent details
  saving = signal(false);
  error = signal('');
  result = signal<SpAgentRegistered | null>(null);
  roles = signal<SpRole[]>([]);
  dek = signal('');

  name = '';
  capsInput = 'rust, typescript, angular, sql, docker, code-review';
  activePreset = 'Full-stack';
  presetNames = Object.keys(CAPABILITY_PRESETS);
  selectedRoleId = '';

  // Step 2: Provider setup
  hasExistingProvider = signal(false);
  savingProvider = signal(false);
  providerError = signal('');
  readonly providerOptions = ['anthropic', 'openai', 'ollama'];
  providerName = 'anthropic';
  providerApiKey = '';
  providerBaseUrl = '';

  // Step 3: Credentials
  copied = signal(false);

  ngOnInit(): void {
    this.teamApi.getRoles().subscribe({
      next: (roles) => {
        this.roles.set(roles);
        if (roles.length > 0) {
          this.selectedRoleId = roles[0].id;
        }
      },
    });
    // Check if a provider is already configured
    this.providerApi.listGlobal().pipe(
      catchError(() => of([])),
    ).subscribe(configs => {
      this.hasExistingProvider.set(configs.length > 0);
    });
    // Fetch DEK for orchestra env hint
    this.tenantApi.getMyTenant().pipe(
      switchMap(tenant => tenant ? this.tenantApi.getDekForOrchestra(tenant.id) : of(null)),
      catchError(() => of(null)),
    ).subscribe({
      next: (res) => { if (res?.dek) this.dek.set(res.dek); },
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

  /** Step 1 → create agent, then advance to provider step */
  onSubmitDetails(): void {
    const name = this.name.trim();
    if (!name) return;

    const capabilities = this.capsInput
      .split(',')
      .map(s => s.trim())
      .filter(Boolean);

    this.saving.set(true);
    this.error.set('');

    this.api
      .createAgent({
        name,
        capabilities,
        metadata: { model: 'claude-opus-4-6', runtime: 'orchestra' },
      })
      .subscribe({
        next: (agent) => {
          this.saving.set(false);
          this.result.set(agent);
          // Advance to provider step (or skip if already configured)
          if (this.hasExistingProvider()) {
            this.step.set('credentials');
            this.stepIndex.set(2);
          } else {
            this.step.set('provider');
            this.stepIndex.set(1);
          }
        },
        error: (err) => {
          this.saving.set(false);
          const msg = err?.error?.message || err?.error || 'Failed to create agent';
          this.error.set(typeof msg === 'string' ? msg : JSON.stringify(msg));
        },
      });
  }

  /** Step 2 → save provider config, then advance to credentials */
  saveProvider(): void {
    const apiKey = this.providerApiKey.trim();
    if (!apiKey) return;

    this.savingProvider.set(true);
    this.providerError.set('');

    this.providerApi
      .createGlobal({
        provider: this.providerName,
        api_key: apiKey,
        ...(this.providerBaseUrl.trim() && { base_url: this.providerBaseUrl.trim() }),
      })
      .subscribe({
        next: () => {
          this.savingProvider.set(false);
          this.step.set('credentials');
          this.stepIndex.set(2);
        },
        error: (err) => {
          this.savingProvider.set(false);
          const msg = err?.error?.message || err?.error || 'Failed to save provider';
          this.providerError.set(typeof msg === 'string' ? msg : JSON.stringify(msg));
        },
      });
  }

  /** Skip provider setup */
  skipProvider(): void {
    this.step.set('credentials');
    this.stepIndex.set(2);
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
