import { Component, inject, signal, OnInit, DestroyRef } from '@angular/core';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import {
  IntegrationsApiService,
  Integration,
  IntegrationAccess,
} from '../../../../core/services/integrations-api.service';
import { ConfirmDialogComponent } from '../../../../shared/components/confirm-dialog/confirm-dialog';

@Component({
  selector: 'app-integration-detail',
  standalone: true,
  imports: [RouterLink, TranslocoModule, ConfirmDialogComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      @if (loading()) {
        <p class="text-text-secondary">{{ t('common.loading') }}</p>
      } @else if (!integration()) {
        <p class="text-ctp-red">{{ t('integrations.notFound') }}</p>
      } @else {
        <div class="mb-6 flex items-center gap-2 text-sm text-text-secondary">
          <a routerLink="/integrations" class="hover:text-accent">{{ t('integrations.title') }}</a>
          <span>/</span>
          <span class="text-text-primary">{{ integration()!.name }}</span>
        </div>

        <div class="flex items-center justify-between mb-6">
          <div class="flex items-center gap-3">
            <h1 class="text-2xl font-semibold text-text-primary">{{ integration()!.name }}</h1>
            <span class="text-xs px-2 py-1 rounded-full"
                  [class]="integration()!.enabled ? 'bg-ctp-green/20 text-ctp-green' : 'bg-ctp-overlay0/20 text-ctp-overlay0'">
              {{ integration()!.enabled ? t('integrations.enabled') : t('integrations.disabled') }}
            </span>
          </div>
          <div class="flex items-center gap-2">
            <a [routerLink]="['edit']"
               class="px-3 py-1.5 text-sm border border-border rounded-lg text-text-primary hover:bg-surface transition-colors">
              {{ t('integrations.edit') }}
            </a>
            <button (click)="confirmDelete()"
                    class="px-3 py-1.5 text-sm border border-ctp-red/50 rounded-lg text-ctp-red hover:bg-ctp-red/10 transition-colors">
              {{ t('integrations.delete') }}
            </button>
          </div>
        </div>

        <!-- Info Grid -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-8">
          <div class="bg-surface border border-border rounded-lg p-4">
            <h3 class="text-xs font-medium text-text-secondary uppercase tracking-wider mb-2">{{ t('integrations.provider') }}</h3>
            <p class="text-text-primary">{{ integration()!.provider }}</p>
          </div>
          <div class="bg-surface border border-border rounded-lg p-4">
            <h3 class="text-xs font-medium text-text-secondary uppercase tracking-wider mb-2">{{ t('integrations.kind') }}</h3>
            <p class="text-text-primary">{{ integration()!.kind }}</p>
          </div>
          <div class="bg-surface border border-border rounded-lg p-4">
            <h3 class="text-xs font-medium text-text-secondary uppercase tracking-wider mb-2">{{ t('integrations.baseUrl') }}</h3>
            <p class="text-text-primary font-mono text-sm">{{ integration()!.base_url }}</p>
          </div>
          <div class="bg-surface border border-border rounded-lg p-4">
            <h3 class="text-xs font-medium text-text-secondary uppercase tracking-wider mb-2">{{ t('integrations.authType') }}</h3>
            <p class="text-text-primary">{{ integration()!.auth_type }}</p>
          </div>
        </div>

        <!-- Capabilities -->
        @if (integration()!.capabilities.length > 0) {
          <div class="mb-8">
            <h2 class="text-lg font-medium text-text-primary mb-3">{{ t('integrations.capabilities') }}</h2>
            <div class="flex flex-wrap gap-2">
              @for (cap of integration()!.capabilities; track cap) {
                <span class="text-sm px-3 py-1 rounded-full bg-accent/10 text-accent">{{ cap }}</span>
              }
            </div>
          </div>
        }

        <!-- Config -->
        @if (hasConfig()) {
          <div class="mb-8">
            <h2 class="text-lg font-medium text-text-primary mb-3">{{ t('integrations.config') }}</h2>
            <pre class="bg-surface border border-border rounded-lg p-4 text-sm text-text-primary font-mono overflow-x-auto">{{ configJson() }}</pre>
          </div>
        }

        <!-- Agent Access -->
        <div class="mb-8">
          <div class="flex items-center justify-between mb-3">
            <h2 class="text-lg font-medium text-text-primary">{{ t('integrations.agentAccess') }}</h2>
            <button (click)="showGrantForm.set(!showGrantForm())"
                    class="text-sm text-accent hover:underline">
              {{ showGrantForm() ? t('integrations.cancel') : t('integrations.grantAccess') }}
            </button>
          </div>

          @if (showGrantForm()) {
            <div class="bg-surface border border-border rounded-lg p-4 mb-4">
              <label for="intd-agent" class="block text-sm text-text-secondary mb-1">{{ t('integrations.agentId') }}</label>
              <div class="flex gap-2">
                <input #agentInput id="intd-agent" type="text" placeholder="Agent ID"
                       class="flex-1 bg-bg-subtle border border-border rounded-lg px-3 py-1.5 text-sm text-text-primary
                              focus:outline-none focus:ring-1 focus:ring-accent" />
                <button (click)="grantAccess(agentInput.value); agentInput.value = ''"
                        class="px-4 py-1.5 bg-accent text-white rounded-lg text-sm hover:opacity-90 transition-opacity">
                  {{ t('integrations.grant') }}
                </button>
              </div>
            </div>
          }

          @if (accessList().length === 0) {
            <p class="text-sm text-text-secondary">{{ t('integrations.noAccess') }}</p>
          } @else {
            <div class="bg-surface border border-border rounded-lg divide-y divide-border">
              @for (access of accessList(); track access.agent_id) {
                <div class="flex items-center justify-between p-3">
                  <div>
                    <span class="text-sm text-text-primary font-mono">{{ access.agent_id }}</span>
                    @if (access.agent_name) {
                      <span class="text-sm text-text-secondary ml-2">{{ access.agent_name }}</span>
                    }
                  </div>
                  <button (click)="revokeAccess(access.agent_id)"
                          class="text-xs text-ctp-red hover:underline">
                    {{ t('integrations.revoke') }}
                  </button>
                </div>
              }
            </div>
          }
        </div>

        <!-- Delete confirmation -->
        @if (showDeleteConfirm()) {
          <app-confirm-dialog
            [title]="t('integrations.deleteConfirmTitle')"
            [message]="t('integrations.deleteConfirmMessage')"
            [cancelLabel]="t('integrations.cancel')"
            [confirmLabel]="t('integrations.delete')"
            (confirmed)="deleteIntegration()"
            (cancelled)="showDeleteConfirm.set(false)" />
        }
      }
    </div>
  `,
})
export class IntegrationDetailPage implements OnInit {
  private api = inject(IntegrationsApiService);
  private route = inject(ActivatedRoute);
  private router = inject(Router);
  private destroyRef = inject(DestroyRef);

  integration = signal<Integration | null>(null);
  accessList = signal<IntegrationAccess[]>([]);
  loading = signal(true);
  showGrantForm = signal(false);
  showDeleteConfirm = signal(false);

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id');
    if (!id) return;

    this.api.get(id).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
      next: (data) => {
        this.integration.set(data);
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });

    this.api.listAccess(id).pipe(takeUntilDestroyed(this.destroyRef)).subscribe({
      next: (data) => this.accessList.set(data),
      error: () => this.accessList.set([]),
    });
  }

  hasConfig(): boolean {
    const cfg = this.integration()?.config;
    return !!cfg && Object.keys(cfg).length > 0;
  }

  configJson(): string {
    return JSON.stringify(this.integration()?.config, null, 2);
  }

  grantAccess(agentId: string): void {
    const id = this.integration()?.id;
    if (!id || !agentId.trim()) return;
    this.api.grantAccess(id, { agent_id: agentId.trim() }).subscribe({
      next: (access) => {
        this.accessList.update(list => [...list, access]);
        this.showGrantForm.set(false);
      },
      error: () => { /* grant failed — form stays open so user can retry */ },
    });
  }

  revokeAccess(agentId: string): void {
    const id = this.integration()?.id;
    if (!id) return;
    this.api.revokeAccess(id, agentId).subscribe({
      next: () => {
        this.accessList.update(list => list.filter(a => a.agent_id !== agentId));
      },
      error: () => { /* revoke failed — list unchanged */ },
    });
  }

  confirmDelete(): void {
    this.showDeleteConfirm.set(true);
  }

  deleteIntegration(): void {
    const id = this.integration()?.id;
    if (!id) return;
    this.api.delete(id).subscribe({
      next: () => this.router.navigate(['/integrations']),
      error: () => this.showDeleteConfirm.set(false),
    });
  }
}
