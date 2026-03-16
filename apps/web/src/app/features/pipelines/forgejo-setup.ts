import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { TranslocoModule } from '@jsverse/transloco';
import {
  CiApiService,
  ForgejoIntegrationResponse,
} from '../../core/services/ci-api.service';
import { ProjectContext } from '../../core/services/project-context.service';
import { ProviderIconComponent } from '../../shared/components/provider-icon/provider-icon';

type SetupStep = 'form' | 'webhook' | 'sync';

@Component({
  selector: 'app-forgejo-setup',
  standalone: true,
  imports: [TranslocoModule, FormsModule, ProviderIconComponent],
  template: `
    <div class="p-3 sm:p-6 max-w-3xl mx-auto" *transloco="let t">
      <!-- Header -->
      <div class="mb-6">
        <button (click)="goBack()" class="text-sm text-text-secondary hover:text-text-primary mb-3 flex items-center gap-1">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
          </svg>
          {{ t('forgejo.backToPipelines') }}
        </button>
        <h1 class="text-2xl font-semibold text-text-primary flex items-center gap-2">
          <app-provider-icon provider="forgejo" size="lg" class="text-ctp-peach" />
          {{ t('forgejo.title') }}
        </h1>
        <p class="text-sm text-text-secondary mt-1">{{ t('forgejo.subtitle') }}</p>
      </div>

      <!-- Progress steps -->
      <div class="flex items-center gap-2 mb-8">
        @for (s of steps; track s; let i = $index) {
          <div class="flex items-center gap-2">
            <div class="w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium transition-colors"
                 [class]="stepIndex() >= i ? 'bg-accent text-bg' : 'bg-surface border border-border text-text-muted'">
              @if (stepIndex() > i) {
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                </svg>
              } @else {
                {{ i + 1 }}
              }
            </div>
            <span class="text-sm font-medium" [class]="stepIndex() >= i ? 'text-text-primary' : 'text-text-muted'">
              {{ t('forgejo.step' + (i + 1)) }}
            </span>
            @if (i < steps.length - 1) {
              <div class="w-8 h-px" [class]="stepIndex() > i ? 'bg-accent' : 'bg-border'"></div>
            }
          </div>
        }
      </div>

      <!-- Step 1: Registration Form -->
      @if (currentStep() === 'form') {
        <div class="bg-surface rounded-lg border border-border p-6">
          <h2 class="text-lg font-medium text-text-primary mb-1">{{ t('forgejo.connectTitle') }}</h2>
          <p class="text-sm text-text-secondary mb-6">{{ t('forgejo.connectDescription') }}</p>

          <div class="space-y-4">
            <!-- Base URL -->
            <label class="block">
              <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('forgejo.baseUrl') }}</span>
              <input type="url" [(ngModel)]="formBaseUrl"
                placeholder="https://git.example.com"
                class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent" />
              <span class="block text-xs text-text-muted mt-1">{{ t('forgejo.baseUrlHint') }}</span>
            </label>

            <!-- Token (optional) -->
            <label class="block">
              <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('forgejo.token') }}</span>
              <input type="password" [(ngModel)]="formToken"
                placeholder="Forgejo access token"
                class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent" />
              <span class="block text-xs text-text-muted mt-1">{{ t('forgejo.tokenHint') }}</span>
            </label>

            @if (error()) {
              <div class="p-3 bg-ctp-red/10 border border-ctp-red/20 rounded-lg">
                <p class="text-sm text-ctp-red">{{ error() }}</p>
              </div>
            }

            <div class="flex items-center gap-3 pt-2">
              <button (click)="register()"
                [disabled]="registering() || !formBaseUrl.trim()"
                class="px-5 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50 transition-opacity">
                @if (registering()) {
                  {{ t('forgejo.registering') }}
                } @else {
                  {{ t('forgejo.register') }}
                }
              </button>
              <button (click)="goBack()"
                class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary transition-colors">
                {{ t('common.cancel') }}
              </button>
            </div>
          </div>
        </div>
      }

      <!-- Step 2: Webhook Configuration -->
      @if (currentStep() === 'webhook') {
        <div class="bg-surface rounded-lg border border-border p-6">
          <h2 class="text-lg font-medium text-text-primary mb-1">{{ t('forgejo.webhookTitle') }}</h2>
          <p class="text-sm text-text-secondary mb-6">{{ t('forgejo.webhookDescription') }}</p>

          <!-- Instructions -->
          <div class="bg-bg-subtle rounded-lg p-4 mb-6 border border-border">
            <h3 class="text-sm font-semibold text-text-primary mb-3">{{ t('forgejo.webhookInstructions') }}</h3>
            <ol class="space-y-2 text-sm text-text-secondary list-decimal list-inside">
              <li>{{ t('forgejo.webhookStep1') }}</li>
              <li>{{ t('forgejo.webhookStep2') }}</li>
              <li>{{ t('forgejo.webhookStep3') }}</li>
              <li>{{ t('forgejo.webhookStep4') }}</li>
              <li>{{ t('forgejo.webhookStep5') }}</li>
            </ol>
          </div>

          <!-- Webhook URL -->
          <div class="mb-4">
            <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('forgejo.webhookUrl') }}</span>
            <div class="flex items-center gap-2">
              <code class="flex-1 bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border font-mono break-all">
                {{ integration()!.webhook_url }}
              </code>
              <button (click)="copyToClipboard(integration()!.webhook_url, 'url')"
                class="shrink-0 px-3 py-2 text-sm font-medium bg-surface border border-border rounded-lg hover:border-accent/50 transition-colors">
                {{ copiedField() === 'url' ? t('common.copied') : t('common.copy') }}
              </button>
            </div>
          </div>

          <!-- Webhook Secret -->
          <div class="mb-6">
            <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('forgejo.webhookSecret') }}</span>
            <div class="flex items-center gap-2">
              <code class="flex-1 bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border font-mono break-all">
                @if (showSecret()) {
                  {{ integration()!.webhook_secret }}
                } @else {
                  ********************************
                }
              </code>
              <button (click)="showSecret.set(!showSecret())"
                class="shrink-0 px-3 py-2 text-sm font-medium bg-surface border border-border rounded-lg hover:border-accent/50 transition-colors">
                {{ showSecret() ? t('forgejo.hide') : t('forgejo.show') }}
              </button>
              <button (click)="copyToClipboard(integration()!.webhook_secret, 'secret')"
                class="shrink-0 px-3 py-2 text-sm font-medium bg-surface border border-border rounded-lg hover:border-accent/50 transition-colors">
                {{ copiedField() === 'secret' ? t('common.copied') : t('common.copy') }}
              </button>
            </div>
            <p class="text-xs text-ctp-yellow mt-1">{{ t('forgejo.secretWarning') }}</p>
          </div>

          <button (click)="currentStep.set('sync')"
            class="px-5 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 transition-opacity">
            {{ t('forgejo.continue') }}
          </button>
        </div>
      }

      <!-- Step 3: Sync & Finish -->
      @if (currentStep() === 'sync') {
        <div class="bg-surface rounded-lg border border-border p-6">
          <h2 class="text-lg font-medium text-text-primary mb-1">{{ t('forgejo.syncTitle') }}</h2>
          <p class="text-sm text-text-secondary mb-6">{{ t('forgejo.syncDescription') }}</p>

          <div class="flex items-center gap-4 mb-6">
            <button (click)="syncRuns()"
              [disabled]="syncing()"
              class="px-5 py-2 bg-surface border border-border text-text-primary rounded-lg text-sm font-medium hover:border-accent/50 disabled:opacity-50 transition-colors">
              @if (syncing()) {
                <span class="flex items-center gap-2">
                  <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
                  </svg>
                  {{ t('forgejo.syncing') }}
                </span>
              } @else {
                {{ t('forgejo.syncNow') }}
              }
            </button>

            @if (syncResult()) {
              <span class="text-sm text-ctp-green">
                {{ t('forgejo.syncResult', { synced: syncResult()!.synced, errors: syncResult()!.errors }) }}
              </span>
            }
          </div>

          @if (syncError()) {
            <div class="p-3 bg-ctp-red/10 border border-ctp-red/20 rounded-lg mb-6">
              <p class="text-sm text-ctp-red">{{ syncError() }}</p>
            </div>
          }

          <!-- Success summary -->
          <div class="bg-ctp-green/5 border border-ctp-green/20 rounded-lg p-4 mb-6">
            <div class="flex items-start gap-3">
              <svg class="w-5 h-5 text-ctp-green shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <div>
                <p class="text-sm font-medium text-text-primary">{{ t('forgejo.setupComplete') }}</p>
                <p class="text-sm text-text-secondary mt-1">{{ t('forgejo.setupCompleteHint') }}</p>
              </div>
            </div>
          </div>

          <button (click)="finish()"
            class="px-5 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90 transition-opacity">
            {{ t('forgejo.goToPipelines') }}
          </button>
        </div>
      }
    </div>
  `,
})
export class ForgejoSetupPage {
  private ciApi = inject(CiApiService);
  private ctx = inject(ProjectContext);
  private router = inject(Router);

  readonly steps: SetupStep[] = ['form', 'webhook', 'sync'];

  // Form state
  formBaseUrl = '';
  formToken = '';

  // Step state
  currentStep = signal<SetupStep>('form');
  stepIndex = signal(0);

  // Registration
  registering = signal(false);
  error = signal<string | null>(null);
  integration = signal<ForgejoIntegrationResponse | null>(null);

  // Webhook display
  showSecret = signal(false);
  copiedField = signal<string | null>(null);
  private copyTimer: ReturnType<typeof setTimeout> | null = null;

  // Sync
  syncing = signal(false);
  syncResult = signal<{ synced: number; errors: number } | null>(null);
  syncError = signal<string | null>(null);

  register(): void {
    const pid = this.ctx.projectId();
    if (!pid || !this.formBaseUrl.trim()) return;

    this.registering.set(true);
    this.error.set(null);

    this.ciApi.registerForgejo(pid, {
      base_url: this.formBaseUrl.trim(),
      token: this.formToken.trim() || undefined,
    }).subscribe({
      next: (result) => {
        this.integration.set(result);
        this.registering.set(false);
        this.currentStep.set('webhook');
        this.stepIndex.set(1);
      },
      error: (err) => {
        this.registering.set(false);
        const msg = err?.error?.error || err?.error?.message || 'Registration failed. Please try again.';
        this.error.set(msg);
      },
    });
  }

  syncRuns(): void {
    const pid = this.ctx.projectId();
    if (!pid) return;

    this.syncing.set(true);
    this.syncError.set(null);
    this.syncResult.set(null);

    this.ciApi.syncForgejo(pid).subscribe({
      next: (result) => {
        this.syncResult.set(result);
        this.syncing.set(false);
        this.stepIndex.set(2);
      },
      error: (err) => {
        this.syncing.set(false);
        const msg = err?.error?.error || err?.error?.message || 'Sync failed. Make sure the token has read access.';
        this.syncError.set(msg);
        // Still advance — sync is optional
        this.stepIndex.set(2);
      },
    });
  }

  copyToClipboard(text: string, field: string): void {
    navigator.clipboard.writeText(text).then(() => {
      this.copiedField.set(field);
      if (this.copyTimer) clearTimeout(this.copyTimer);
      this.copyTimer = setTimeout(() => this.copiedField.set(null), 2000);
    });
  }

  goBack(): void {
    this.router.navigate(['/pipelines']);
  }

  finish(): void {
    this.router.navigate(['/pipelines']);
  }
}
