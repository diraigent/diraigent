import { Component, inject, signal, computed, effect } from '@angular/core';
import { RouterLink } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import {
  IntegrationsApiService,
  Integration,
  IntegrationKind,
} from '../../../../core/services/integrations-api.service';
import { ProjectContext } from '../../../../core/services/project-context.service';
import { INTEGRATION_KIND_COLORS } from '../../../../shared/ui-constants';
import { ProviderIconComponent } from '../../../../shared/components/provider-icon/provider-icon';

@Component({
  selector: 'app-integration-list',
  standalone: true,
  imports: [RouterLink, TranslocoModule, ProviderIconComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('integrations.title') }}</h1>
        <div class="flex items-center gap-2">
          @if (hasLogging()) {
            <a routerLink="logs"
               class="flex items-center gap-2 px-4 py-2 bg-surface border border-border text-text-primary rounded-lg text-sm font-medium hover:border-accent/50 transition-colors">
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 10h16M4 14h16M4 18h12"/>
              </svg>
              {{ t('integrations.viewLogs') }}
            </a>
          }
          <a routerLink="new"
             class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
            {{ t('integrations.create') }}
          </a>
        </div>
      </div>

      @if (loading()) {
        <p class="text-text-secondary">{{ t('common.loading') }}</p>
      } @else if (error()) {
        <p class="text-ctp-red">{{ t('common.error') }}</p>
      } @else if (integrations().length === 0) {
        <div class="text-center py-12">
          <svg class="w-12 h-12 mx-auto text-text-secondary mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
                  d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1"/>
          </svg>
          <p class="text-text-secondary">{{ t('integrations.empty') }}</p>
        </div>
      } @else {
        <div class="grid gap-4">
          @for (integration of integrations(); track integration.id) {
            <a [routerLink]="integration.id"
               class="block bg-surface border border-border rounded-lg p-4 hover:border-accent/50 transition-colors">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <div class="w-10 h-10 rounded-lg bg-bg-subtle flex items-center justify-center"
                       [class]="providerIconColor(integration.provider)">
                    <app-provider-icon [provider]="integration.provider" size="md" />
                  </div>
                  <div>
                    <div class="flex items-center gap-2">
                      <span class="font-medium text-text-primary">{{ integration.name }}</span>
                      <span class="text-xs px-2 py-0.5 rounded-full {{ kindColor(integration.kind) }}">
                        {{ integration.kind }}
                      </span>
                    </div>
                    <p class="text-sm text-text-secondary">{{ integration.provider }} · {{ integration.base_url }}</p>
                  </div>
                </div>
                <div class="flex items-center gap-3">
                  <span class="text-xs px-2 py-1 rounded-full"
                        [class]="integration.enabled ? 'bg-ctp-green/20 text-ctp-green' : 'bg-ctp-overlay0/20 text-ctp-overlay0'">
                    {{ integration.enabled ? t('integrations.enabled') : t('integrations.disabled') }}
                  </span>
                  <button (click)="toggleEnabled($event, integration)"
                          class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors"
                          [class]="integration.enabled ? 'bg-accent' : 'bg-ctp-overlay0'"
                          [attr.aria-label]="integration.enabled ? t('integrations.disable') : t('integrations.enable')">
                    <span class="inline-block h-4 w-4 rounded-full bg-white transition-transform"
                          [class]="integration.enabled ? 'translate-x-6' : 'translate-x-1'"></span>
                  </button>
                </div>
              </div>
              @if (integration.capabilities.length > 0) {
                <div class="mt-2 flex flex-wrap gap-1">
                  @for (cap of integration.capabilities; track cap) {
                    <span class="text-xs px-1.5 py-0.5 rounded bg-bg-subtle text-text-secondary">{{ cap }}</span>
                  }
                </div>
              }
            </a>
          }
        </div>
      }
    </div>
  `,
})
export class IntegrationListPage {
  private api = inject(IntegrationsApiService);
  private ctx = inject(ProjectContext);

  integrations = signal<Integration[]>([]);
  loading = signal(true);
  error = signal(false);

  /** True when at least one enabled logging integration (e.g. Grafana/Loki) exists */
  hasLogging = computed(() =>
    this.integrations().some(i => i.kind === 'logging' && i.enabled),
  );

  constructor() {
    effect(() => {
      this.ctx.projectId();
      this.loadIntegrations();
    });
  }

  kindColor(kind: IntegrationKind): string {
    return INTEGRATION_KIND_COLORS[kind] ?? INTEGRATION_KIND_COLORS['custom'];
  }

  providerIconColor(provider: string): string {
    switch (provider) {
      case 'github': return 'text-ctp-mauve';
      case 'forgejo': return 'text-ctp-peach';
      default: return 'text-text-secondary';
    }
  }

  toggleEnabled(event: Event, integration: Integration): void {
    event.preventDefault();
    event.stopPropagation();
    const newEnabled = !integration.enabled;
    this.api.update(integration.id, { enabled: newEnabled }).subscribe({
      next: (updated) => {
        this.integrations.update(list =>
          list.map(i => (i.id === updated.id ? updated : i)),
        );
      },
    });
  }

  private loadIntegrations(): void {
    const projectId = localStorage.getItem('diraigent-project');
    if (!projectId) {
      this.loading.set(false);
      return;
    }
    this.api.list(projectId).subscribe({
      next: (data) => {
        this.integrations.set(data);
        this.loading.set(false);
      },
      error: () => {
        this.error.set(true);
        this.loading.set(false);
      },
    });
  }
}
