import { Component, inject, signal, OnInit } from '@angular/core';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import {
  IntegrationsApiService,
  Integration,
  IntegrationKind,
  AuthType,
  CreateIntegrationRequest,
} from '../../../../core/services/integrations-api.service';

const INTEGRATION_KINDS: IntegrationKind[] = [
  'logging', 'tracing', 'metrics', 'git', 'ci',
  'messaging', 'monitoring', 'storage', 'database', 'custom',
];

const AUTH_TYPES: AuthType[] = ['none', 'token', 'basic', 'api_key', 'oauth2'];

@Component({
  selector: 'app-integration-form',
  standalone: true,
  imports: [FormsModule, RouterLink, TranslocoModule],
  template: `
    <div class="p-3 sm:p-6 max-w-2xl" *transloco="let t">
      <div class="mb-6 flex items-center gap-2 text-sm text-text-secondary">
        <a routerLink="/integrations" class="hover:text-accent">{{ t('integrations.title') }}</a>
        <span>/</span>
        <span class="text-text-primary">{{ isEdit() ? t('integrations.editTitle') : t('integrations.createTitle') }}</span>
      </div>

      <h1 class="text-2xl font-semibold text-text-primary mb-6">
        {{ isEdit() ? t('integrations.editTitle') : t('integrations.createTitle') }}
      </h1>

      @if (loading()) {
        <p class="text-text-secondary">{{ t('common.loading') }}</p>
      } @else {
        <form (ngSubmit)="onSubmit()" class="space-y-5">
          <!-- Name -->
          <div>
            <label for="int-name" class="block text-sm font-medium text-text-secondary mb-1">{{ t('integrations.name') }}</label>
            <input id="int-name" type="text" [(ngModel)]="name" name="name" required
                   class="w-full bg-bg-subtle border border-border rounded-lg px-3 py-2 text-sm text-text-primary
                          focus:outline-none focus:ring-1 focus:ring-accent"
                   [placeholder]="t('integrations.namePlaceholder')" />
          </div>

          <!-- Kind -->
          <div>
            <label for="int-kind" class="block text-sm font-medium text-text-secondary mb-1">{{ t('integrations.kind') }}</label>
            <select id="int-kind" [(ngModel)]="kind" name="kind" required
                    class="w-full bg-bg-subtle border border-border rounded-lg px-3 py-2 text-sm text-text-primary
                           focus:outline-none focus:ring-1 focus:ring-accent">
              @for (k of kinds; track k) {
                <option [value]="k">{{ k }}</option>
              }
            </select>
          </div>

          <!-- Provider -->
          <div>
            <label for="int-provider" class="block text-sm font-medium text-text-secondary mb-1">{{ t('integrations.provider') }}</label>
            <input id="int-provider" type="text" [(ngModel)]="provider" name="provider" required
                   class="w-full bg-bg-subtle border border-border rounded-lg px-3 py-2 text-sm text-text-primary
                          focus:outline-none focus:ring-1 focus:ring-accent"
                   [placeholder]="t('integrations.providerPlaceholder')" />
          </div>

          <!-- Base URL -->
          <div>
            <label for="int-base-url" class="block text-sm font-medium text-text-secondary mb-1">{{ t('integrations.baseUrl') }}</label>
            <input id="int-base-url" type="url" [(ngModel)]="baseUrl" name="baseUrl" required
                   class="w-full bg-bg-subtle border border-border rounded-lg px-3 py-2 text-sm text-text-primary font-mono
                          focus:outline-none focus:ring-1 focus:ring-accent"
                   placeholder="https://" />
          </div>

          <!-- Auth Type -->
          <div>
            <label for="int-auth-type" class="block text-sm font-medium text-text-secondary mb-1">{{ t('integrations.authType') }}</label>
            <select id="int-auth-type" [(ngModel)]="authType" name="authType" required
                    class="w-full bg-bg-subtle border border-border rounded-lg px-3 py-2 text-sm text-text-primary
                           focus:outline-none focus:ring-1 focus:ring-accent">
              @for (a of authTypes; track a) {
                <option [value]="a">{{ a }}</option>
              }
            </select>
          </div>

          <!-- Credentials (conditional) -->
          @if (authType !== 'none') {
            <div>
              <label for="int-credentials" class="block text-sm font-medium text-text-secondary mb-1">{{ t('integrations.credentials') }}</label>
              <textarea id="int-credentials" [(ngModel)]="credentialsJson" name="credentials" rows="3"
                        class="w-full bg-bg-subtle border border-border rounded-lg px-3 py-2 text-sm text-text-primary font-mono
                               focus:outline-none focus:ring-1 focus:ring-accent"
                        [placeholder]="t('integrations.credentialsPlaceholder')"></textarea>
            </div>
          }

          <!-- Capabilities -->
          <div>
            <label for="int-capabilities" class="block text-sm font-medium text-text-secondary mb-1">{{ t('integrations.capabilities') }}</label>
            <input id="int-capabilities" type="text" [(ngModel)]="capabilitiesStr" name="capabilities"
                   class="w-full bg-bg-subtle border border-border rounded-lg px-3 py-2 text-sm text-text-primary
                          focus:outline-none focus:ring-1 focus:ring-accent"
                   [placeholder]="t('integrations.capabilitiesPlaceholder')" />
            <p class="text-xs text-text-secondary mt-1">{{ t('integrations.capabilitiesHint') }}</p>
          </div>

          <!-- Config -->
          <div>
            <label for="int-config" class="block text-sm font-medium text-text-secondary mb-1">{{ t('integrations.config') }}</label>
            <textarea id="int-config" [(ngModel)]="configJson" name="config" rows="4"
                      class="w-full bg-bg-subtle border border-border rounded-lg px-3 py-2 text-sm text-text-primary font-mono
                             focus:outline-none focus:ring-1 focus:ring-accent"
                      placeholder="{}"></textarea>
          </div>

          @if (errorMessage()) {
            <p class="text-sm text-ctp-red">{{ errorMessage() }}</p>
          }

          <!-- Actions -->
          <div class="flex gap-3 pt-2">
            <button type="submit" [disabled]="saving()"
                    class="px-4 py-2 bg-accent text-white rounded-lg text-sm font-medium hover:opacity-90 transition-opacity disabled:opacity-50">
              {{ saving() ? t('common.loading') : (isEdit() ? t('integrations.save') : t('integrations.create')) }}
            </button>
            <a routerLink="/integrations"
               class="px-4 py-2 border border-border rounded-lg text-sm text-text-primary hover:bg-surface transition-colors">
              {{ t('integrations.cancel') }}
            </a>
          </div>
        </form>
      }
    </div>
  `,
})
export class IntegrationFormPage implements OnInit {
  private api = inject(IntegrationsApiService);
  private route = inject(ActivatedRoute);
  private router = inject(Router);

  readonly kinds = INTEGRATION_KINDS;
  readonly authTypes = AUTH_TYPES;

  isEdit = signal(false);
  loading = signal(false);
  saving = signal(false);
  errorMessage = signal('');

  name = '';
  kind: IntegrationKind = 'custom';
  provider = '';
  baseUrl = '';
  authType: AuthType = 'none';
  credentialsJson = '';
  capabilitiesStr = '';
  configJson = '{}';

  private integrationId = '';

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id');
    if (id) {
      this.isEdit.set(true);
      this.integrationId = id;
      this.loading.set(true);
      this.api.get(id).subscribe({
        next: (data) => this.populateForm(data),
        error: () => this.loading.set(false),
      });
    }
  }

  onSubmit(): void {
    this.errorMessage.set('');
    const req = this.buildRequest();
    if (!req) return;

    this.saving.set(true);

    if (this.isEdit()) {
      this.api.update(this.integrationId, req).subscribe({
        next: () => this.router.navigate(['/integrations', this.integrationId]),
        error: () => {
          this.errorMessage.set('Failed to update integration');
          this.saving.set(false);
        },
      });
    } else {
      const projectId = localStorage.getItem('diraigent-project');
      if (!projectId) {
        this.errorMessage.set('No project selected');
        this.saving.set(false);
        return;
      }
      this.api.create(projectId, req as CreateIntegrationRequest).subscribe({
        next: (created) => this.router.navigate(['/integrations', created.id]),
        error: () => {
          this.errorMessage.set('Failed to create integration');
          this.saving.set(false);
        },
      });
    }
  }

  private populateForm(data: Integration): void {
    this.name = data.name;
    this.kind = data.kind;
    this.provider = data.provider;
    this.baseUrl = data.base_url;
    this.authType = data.auth_type;
    this.capabilitiesStr = data.capabilities.join(', ');
    this.configJson = Object.keys(data.config).length > 0
      ? JSON.stringify(data.config, null, 2)
      : '{}';
    this.loading.set(false);
  }

  private buildRequest(): CreateIntegrationRequest | null {
    if (!this.name.trim() || !this.provider.trim() || !this.baseUrl.trim()) {
      this.errorMessage.set('Name, provider, and base URL are required');
      return null;
    }

    let credentials: Record<string, string> | undefined;
    if (this.credentialsJson.trim()) {
      try {
        credentials = JSON.parse(this.credentialsJson);
      } catch {
        this.errorMessage.set('Invalid credentials JSON');
        return null;
      }
    }

    let config: Record<string, unknown> | undefined;
    if (this.configJson.trim() && this.configJson.trim() !== '{}') {
      try {
        config = JSON.parse(this.configJson);
      } catch {
        this.errorMessage.set('Invalid config JSON');
        return null;
      }
    }

    const capabilities = this.capabilitiesStr
      .split(',')
      .map(s => s.trim())
      .filter(s => s.length > 0);

    return {
      name: this.name.trim(),
      kind: this.kind,
      provider: this.provider.trim(),
      base_url: this.baseUrl.trim(),
      auth_type: this.authType,
      ...(credentials && { credentials }),
      ...(config && { config }),
      ...(capabilities.length > 0 && { capabilities }),
    };
  }
}
