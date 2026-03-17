import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { TranslocoModule, TranslocoService } from '@jsverse/transloco';
import { firstValueFrom } from 'rxjs';
import { TenantApiService, Tenant } from '../../core/services/tenant-api.service';
import { AuthService } from '../../core/services/auth.service';
import { CryptoService } from '../../core/services/crypto.service';
import { ThemeService } from '../../core/services/theme.service';
import { AgentsApiService, SpAgent } from '../../core/services/agents-api.service';
import { PassphrasePromptComponent } from '../../shared/components/passphrase-prompt/passphrase-prompt';
import { AppearanceSettingsComponent } from '../../shared/components/appearance-settings/appearance-settings';
import { environment } from '../../../environments/environment';

@Component({
  selector: 'app-tenant-settings',
  standalone: true,
  imports: [TranslocoModule, FormsModule, PassphrasePromptComponent, AppearanceSettingsComponent],
  template: `
    <div class="p-6 max-w-4xl" *transloco="let t">
      <h1 class="text-2xl font-semibold text-text-primary mb-6">{{ t('tenantSettings.title') }}</h1>

      <!-- Appearance -->
      <section class="mb-8">
        <h2 class="text-lg font-medium text-text-primary mb-4">{{ t('tenantSettings.appearance') }}</h2>
        <div class="bg-surface rounded-lg border border-border p-6">
          <p class="text-sm text-text-secondary mb-4">{{ t('tenantSettings.appearanceHint') }}</p>
          <app-appearance-settings />
        </div>
      </section>

      <!-- Encryption -->
      <section class="mb-8">
        <h2 class="text-lg font-medium text-text-primary mb-4">{{ t('tenantSettings.encryption') }}</h2>
        <div class="bg-surface rounded-lg border border-border p-6 space-y-4">
          @if (loadingTenant()) {
            <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
          } @else if (!tenant()) {
            <div>
              <p class="text-sm text-text-secondary mb-3">{{ t('tenantSettings.noTenant') }}</p>
              <div class="flex items-center gap-3">
                <input type="text" [(ngModel)]="newTenantName" [placeholder]="t('tenantSettings.tenantNamePlaceholder')"
                  class="bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent" />
                <button (click)="createTenant()"
                  [disabled]="creatingTenant() || !newTenantName.trim()"
                  class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50">
                  @if (creatingTenant()) { {{ t('tenantSettings.creating') }} } @else { {{ t('tenantSettings.createTenant') }} }
                </button>
              </div>
            </div>
          } @else {
            <!-- Tenant info -->
            <div class="flex items-center gap-3">
              <span class="text-sm text-text-secondary">{{ t('tenantSettings.tenant') }}:</span>
              <span class="text-sm font-medium text-text-primary">{{ tenant()!.name }}</span>
              <span class="text-xs px-2 py-0.5 rounded font-mono"
                [class]="encryptionModeClass()">
                {{ tenant()!.encryption_mode }}
              </span>
            </div>

            @if (tenant()!.encryption_mode === 'none') {
              <!-- Not encrypted — offer to enable -->
              <div class="pt-2 border-t border-border">
                <p class="text-sm text-text-secondary mb-3">{{ t('tenantSettings.encryptionDisabledHint') }}</p>
                <button (click)="initEncryption()"
                  [disabled]="initializingEncryption()"
                  class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50">
                  @if (initializingEncryption()) { {{ t('tenantSettings.initializing') }} } @else { {{ t('tenantSettings.enableEncryption') }} }
                </button>
                @if (encryptionError()) {
                  <p class="text-sm text-ctp-red mt-2">{{ encryptionError() }}</p>
                }
              </div>
            } @else {
              <!-- Already encrypted — show status -->
              <div class="pt-2 border-t border-border space-y-3">
                <div class="flex items-center gap-2">
                  <span class="w-2 h-2 rounded-full bg-ctp-green"></span>
                  <span class="text-sm text-text-secondary">{{ t('tenantSettings.encryptionActive') }}</span>
                </div>
                <p class="text-xs text-text-secondary">
                  {{ t('tenantSettings.encryptionActiveHint') }}
                  @if (tenant()!.encryption_mode === 'login_derived') {
                    {{ t('tenantSettings.encryptionLoginDerived') }}
                  } @else {
                    {{ t('tenantSettings.encryptionPassphrase') }}
                  }
                </p>

                <!-- Key Rotation -->
                <div class="flex items-center gap-3 pt-2 border-t border-border">
                  <button (click)="rotateKeys()"
                    [disabled]="rotatingKeys()"
                    class="px-3 py-1.5 bg-bg-subtle text-text-primary rounded-lg text-sm border border-border hover:border-accent disabled:opacity-50">
                    @if (rotatingKeys()) { {{ t('tenantSettings.rotating') }} } @else { {{ t('tenantSettings.rotateKey') }} }
                  </button>
                  @if (rotationResult()) {
                    <span class="text-xs text-ctp-green">{{ rotationResult() }}</span>
                  }
                  @if (encryptionError()) {
                    <span class="text-xs text-ctp-red">{{ encryptionError() }}</span>
                  }
                </div>

                <!-- Switch to Passphrase Mode -->
                @if (tenant()!.encryption_mode === 'login_derived') {
                  <div class="pt-3 border-t border-border">
                    <p class="text-xs text-text-secondary mb-2">{{ t('tenantSettings.switchPassphraseHint') }}</p>
                    <button (click)="showPassphrasePrompt.set(true)"
                      class="px-3 py-1.5 bg-bg-subtle text-text-primary rounded-lg text-sm border border-border hover:border-accent">
                      {{ t('tenantSettings.switchPassphrase') }}
                    </button>
                  </div>
                }
              </div>
            }
          }
        </div>
      </section>

      <!-- Account -->
      <section class="mb-8">
        <h2 class="text-lg font-medium text-text-primary mb-4">{{ t('tenantSettings.account') }}</h2>
        <div class="bg-surface rounded-lg border border-border p-6 space-y-4">
          <p class="text-sm text-text-secondary">{{ t('tenantSettings.accountHint') }}</p>
          @if (authProviderBase) {
            <div class="flex flex-wrap gap-3">
              <a [href]="authProviderBase + '/if/flow/diraigent-user-settings/'"
                target="_blank" rel="noopener"
                class="inline-flex items-center gap-2 px-4 py-2 bg-accent/10 text-accent rounded-lg text-sm border border-accent/30 hover:bg-accent/20">
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                </svg>
                {{ t('tenantSettings.editProfile') }}
              </a>
              <a [href]="authProviderBase + '/if/flow/default-password-change/'"
                target="_blank" rel="noopener"
                class="inline-flex items-center gap-2 px-4 py-2 bg-accent/10 text-accent rounded-lg text-sm border border-accent/30 hover:bg-accent/20">
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                </svg>
                {{ t('tenantSettings.changePassword') }}
              </a>
              <a [href]="authProviderBase + '/if/flow/default-authenticator-totp-setup/'"
                target="_blank" rel="noopener"
                class="inline-flex items-center gap-2 px-4 py-2 bg-accent/10 text-accent rounded-lg text-sm border border-accent/30 hover:bg-accent/20">
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                </svg>
                {{ t('tenantSettings.setupMfa') }}
              </a>
            </div>
          }
          <div class="pt-4 border-t border-border">
            <p class="text-sm text-ctp-red/80 mb-3">{{ t('tenantSettings.deleteAccountWarning') }}</p>
            @if (!confirmingDelete()) {
              <button (click)="confirmingDelete.set(true)"
                class="px-4 py-2 bg-ctp-red/10 text-ctp-red rounded-lg text-sm border border-ctp-red/30 hover:bg-ctp-red/20">
                {{ t('tenantSettings.deleteAccount') }}
              </button>
            } @else {
              <div class="flex items-center gap-3">
                <button (click)="deleteAccount()"
                  [disabled]="deletingAccount()"
                  class="px-4 py-2 bg-ctp-red text-ctp-base rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50">
                  @if (deletingAccount()) { {{ t('tenantSettings.deleting') }} } @else { {{ t('tenantSettings.confirmDelete') }} }
                </button>
                <button (click)="confirmingDelete.set(false)"
                  class="px-4 py-2 bg-bg-subtle text-text-primary rounded-lg text-sm border border-border hover:border-accent">
                  {{ t('common.cancel') }}
                </button>
              </div>
            }
            @if (deleteError()) {
              <p class="text-sm text-ctp-red mt-2">{{ deleteError() }}</p>
            }
          </div>
        </div>
      </section>

      <!-- Version Info -->
      <section class="mb-8">
        <h2 class="text-lg font-medium text-text-primary mb-4">{{ t('tenantSettings.versionInfo') }}</h2>
        <div class="bg-surface rounded-lg border border-border p-6">
          <div class="space-y-3">
            <div class="flex items-center justify-between">
              <span class="text-sm text-text-secondary">{{ t('tenantSettings.versionWeb') }}</span>
              <span class="text-sm font-mono text-text-primary">{{ webVersion }}</span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-sm text-text-secondary">{{ t('tenantSettings.versionApi') }}</span>
              <span class="text-sm font-mono text-text-primary">{{ apiVersion() || '—' }}</span>
            </div>
            @if (orchestras().length > 0) {
              <div class="pt-3 border-t border-border">
                <span class="text-sm text-text-secondary">{{ t('tenantSettings.versionOrchestras') }}</span>
                <div class="mt-2 space-y-2">
                  @for (orch of orchestras(); track orch.id) {
                    <div class="flex items-center justify-between bg-bg-subtle rounded px-3 py-2">
                      <div class="flex items-center gap-2">
                        <span class="w-2 h-2 rounded-full" [class]="orch.status === 'idle' || orch.status === 'working' ? 'bg-ctp-green' : 'bg-ctp-overlay1'"></span>
                        <span class="text-sm text-text-primary">{{ orch.name }}</span>
                      </div>
                      <span class="text-sm font-mono text-text-secondary">{{ orch.metadata['version'] || '—' }}</span>
                    </div>
                  }
                </div>
              </div>
            } @else {
              <div class="flex items-center justify-between">
                <span class="text-sm text-text-secondary">{{ t('tenantSettings.versionOrchestras') }}</span>
                <span class="text-sm text-text-secondary">{{ t('tenantSettings.noOrchestras') }}</span>
              </div>
            }
          </div>
        </div>
      </section>

      @if (showPassphrasePrompt()) {
        <app-passphrase-prompt
          (confirmed)="switchToPassphraseMode($event)"
          (dismiss)="showPassphrasePrompt.set(false)" />
      }
    </div>
  `,
})
export class TenantSettingsPage {
  private tenantApi = inject(TenantApiService);
  private auth = inject(AuthService);
  private cryptoSvc = inject(CryptoService);
  private themeService = inject(ThemeService);
  private transloco = inject(TranslocoService);
  private agentsApi = inject(AgentsApiService);

  // Tenant / Encryption
  tenant = signal<Tenant | null>(null);
  loadingTenant = signal(true);
  initializingEncryption = signal(false);
  encryptionError = signal('');
  newTenantName = '';
  creatingTenant = signal(false);
  rotatingKeys = signal(false);
  rotationResult = signal('');
  showPassphrasePrompt = signal(false);

  // Account
  authProviderBase = environment.authProviderBase;
  confirmingDelete = signal(false);
  deletingAccount = signal(false);
  deleteError = signal('');

  // Version Info
  webVersion = environment.appVersion;
  apiVersion = signal('');
  orchestras = signal<SpAgent[]>([]);

  encryptionModeClass = () => {
    const mode = this.tenant()?.encryption_mode;
    switch (mode) {
      case 'login_derived': return 'bg-ctp-green/20 text-ctp-green';
      case 'passphrase': return 'bg-ctp-blue/20 text-ctp-blue';
      default: return 'bg-bg-subtle text-text-secondary';
    }
  };

  constructor() {
    this.loadTenant();
    this.loadVersionInfo();
  }

  private loadVersionInfo(): void {
    // Fetch API version from /v1/config
    fetch(`${environment.apiServer}/config`)
      .then(res => res.json())
      .then(data => {
        if (data.api_version) {
          this.apiVersion.set(data.api_version);
        }
      })
      .catch(() => { /* API unreachable */ });

    // Fetch orchestras (agents with runtime: "orchestra" in metadata)
    this.agentsApi.getAgents().subscribe({
      next: agents => {
        const orchs = agents.filter(a => a.metadata?.['runtime'] === 'orchestra');
        this.orchestras.set(orchs);
      },
      error: () => { /* ignore */ },
    });
  }

  private loadTenant(): void {
    this.loadingTenant.set(true);
    this.tenantApi.getMyTenant().subscribe({
      next: t => {
        this.tenant.set(t);
        this.loadingTenant.set(false);
        this.themeService.setTenant(t?.id ?? null, t?.theme_preference, t?.accent_color);
      },
      error: () => {
        this.tenant.set(null);
        this.loadingTenant.set(false);
      },
    });
  }

  createTenant(): void {
    const name = this.newTenantName.trim();
    if (!name) return;
    const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
    this.creatingTenant.set(true);
    this.tenantApi.createTenant({ name, slug }).subscribe({
      next: t => {
        this.tenant.set(t);
        this.creatingTenant.set(false);
        this.newTenantName = '';
        this.themeService.setTenant(t.id);
      },
      error: () => this.creatingTenant.set(false),
    });
  }

  initEncryption(): void {
    const t = this.tenant();
    if (!t) return;
    this.initializingEncryption.set(true);
    this.encryptionError.set('');
    this.tenantApi.initEncryption(t.id).subscribe({
      next: () => {
        this.initializingEncryption.set(false);
        this.loadTenant();
      },
      error: err => {
        this.initializingEncryption.set(false);
        this.encryptionError.set(err.error?.message || this.transloco.translate('tenantSettings.encryptionInitFailed'));
      },
    });
  }

  rotateKeys(): void {
    const t = this.tenant();
    if (!t) return;
    this.rotatingKeys.set(true);
    this.encryptionError.set('');
    this.rotationResult.set('');
    this.tenantApi.rotateKeys(t.id).subscribe({
      next: res => {
        this.rotatingKeys.set(false);
        this.rotationResult.set(
          this.transloco.translate('tenantSettings.rotationSuccess', {
            new_key_version: res.new_key_version,
            fields_rotated: res.fields_rotated,
          }),
        );
      },
      error: err => {
        this.rotatingKeys.set(false);
        this.encryptionError.set(err.error?.message || this.transloco.translate('tenantSettings.keyRotationFailed'));
      },
    });
  }

  deleteAccount(): void {
    const token = this.auth.getAccessToken();
    if (!token) {
      this.deleteError.set(this.transloco.translate('tenantSettings.noAccessTokenShort'));
      return;
    }
    this.deletingAccount.set(true);
    this.deleteError.set('');
    fetch(`${environment.apiServer}/account`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${token}` },
    })
      .then(res => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        if (this.authProviderBase) {
          // Redirect to Authentik to also delete the auth identity
          window.location.href = this.authProviderBase + '/if/flow/diraigent-account-delete/';
        } else {
          this.auth.logout();
        }
      })
      .catch(err => {
        this.deletingAccount.set(false);
        this.deleteError.set(err.message || this.transloco.translate('tenantSettings.deleteAccountFailed'));
      });
  }

  async switchToPassphraseMode(_passphrase: string): Promise<void> {
    const t = this.tenant();
    if (!t) return;

    try {
      this.encryptionError.set('');

      const currentSalt = t.key_salt;
      if (!currentSalt) {
        this.encryptionError.set(this.transloco.translate('tenantSettings.noEncryptionSalt'));
        return;
      }

      // Fetch the user's internal ID for KEK derivation
      const token = this.auth.getAccessToken();
      if (!token) {
        this.encryptionError.set(this.transloco.translate('tenantSettings.noAccessTokenShort'));
        return;
      }
      const accountRes = await fetch(`${environment.apiServer}/account`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (!accountRes.ok) throw new Error('Failed to fetch account');
      const { user_id: userId } = await accountRes.json();

      const currentKek = await this.cryptoSvc.deriveKek(userId, currentSalt);

      const keys = await firstValueFrom(this.tenantApi.listKeys(t.id, 'me'));
      const loginKey = keys?.find(k => k.key_type === 'login_derived');
      if (!loginKey) {
        this.encryptionError.set(this.transloco.translate('tenantSettings.noLoginKey'));
        return;
      }

      await this.cryptoSvc.unwrapAndStoreDek(loginKey.wrapped_dek, currentKek);

      const newSalt = this.cryptoSvc.generateSalt();
      const updated = await firstValueFrom(
        this.tenantApi.updateTenant(t.id, { encryption_mode: 'passphrase', key_salt: newSalt }),
      );
      await firstValueFrom(
        this.tenantApi.createKey(t.id, loginKey.user_id, {
          key_type: 'passphrase',
          wrapped_dek: loginKey.wrapped_dek,
          kdf_salt: newSalt,
          key_version: loginKey.key_version,
        }),
      );
      this.tenant.set(updated);
      this.showPassphrasePrompt.set(false);
    } catch (e: unknown) {
      this.encryptionError.set(e instanceof Error ? e.message : this.transloco.translate('tenantSettings.passphraseSetupFailed'));
    }
  }
}
