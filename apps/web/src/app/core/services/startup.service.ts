import { Injectable, inject } from '@angular/core';
import { PLATFORM_ID } from '@angular/core';
import { isPlatformBrowser } from '@angular/common';
import { OAuthService } from 'angular-oauth2-oidc';
import { AuthService } from './auth.service';
import { TenantApiService } from './tenant-api.service';
import { getAuthConfig } from '../config/oauth-config';
import { environment } from '../../../environments/environment';

@Injectable({ providedIn: 'root' })
export class StartupService {
  private initialized = false;
  private oauth = inject(OAuthService);
  private auth = inject(AuthService);
  private tenantApi = inject(TenantApiService);
  private platformId = inject<object>(PLATFORM_ID);

  async init(timeoutMs = 8000): Promise<void> {
    if (this.initialized) return;

    if (!isPlatformBrowser(this.platformId)) {
      this.auth.markInitialized();
      this.initialized = true;
      return;
    }

    // Check if the API requires authentication
    const authRequired = await this.checkAuthRequired();
    if (!authRequired) {
      this.auth.markAuthDisabled();
      this.initialized = true;
      return;
    }

    const cfg = getAuthConfig();
    try {
      this.oauth.configure(cfg);
    } catch (e) {
      if (!environment.production) console.warn('oauth.configure failed', e);
    }

    try {
      (this.oauth as unknown as { strictDiscoveryDocumentValidation: boolean }).strictDiscoveryDocumentValidation = false;
    } catch {
      // ignore
    }

    const hasValidToken = this.oauth.hasValidAccessToken();
    const urlParams = new URLSearchParams(window.location.search);
    const hasAuthCode = urlParams.has('code');

    if (hasValidToken && !hasAuthCode) {
      // Fast path: token exists and is not expired — mark initialized immediately,
      // load discovery + set up refresh in background.
      this.auth.markInitialized();
      this.oauth.loadDiscoveryDocumentAndTryLogin()
        .then(() => this.oauth.setupAutomaticSilentRefresh())
        .catch(err => { if (!environment.production) console.warn('Background OAuth discovery failed', err); });
    } else {
      // Slow path: no valid token or completing a login callback.
      // Wait for discovery before marking initialized.
      try {
        await Promise.race([
          this.oauth.loadDiscoveryDocumentAndTryLogin(),
          new Promise((_, reject) => setTimeout(() => reject(new Error('OAuth init timeout')), timeoutMs)),
        ]);
        // If we have a refresh token but the access token is expired, refresh now
        // before marking initialized — otherwise the first API call will 401.
        if (this.oauth.getRefreshToken() && !this.oauth.hasValidAccessToken()) {
          try {
            await this.oauth.refreshToken();
          } catch (refreshErr) {
            if (!environment.production) console.warn('Token refresh failed', refreshErr);
          }
        }
        this.oauth.setupAutomaticSilentRefresh();
      } catch (err) {
        if (!environment.production) console.warn('OAuth discovery/login failed or timed out', err);
        // If we had an auth code but token exchange failed, mark as failed
        // so the auth guard doesn't keep redirecting
        if (hasAuthCode) {
          if (!environment.production) console.error('Token exchange failed — breaking redirect loop');
          this._loginFailed = true;
        }
      }
      this.auth.markInitialized();
    }

    // Auto-unlock encryption for login-derived tenants (fire-and-forget)
    this.tryUnlockEncryption();

    this.initialized = true;
  }

  /**
   * Auto-unlock encryption for login-derived tenants.
   *
   * After OAuth login, fetches the user's tenant. If it uses `login_derived`
   * encryption, sends the access token to the server so it can derive the KEK,
   * unwrap the DEK, and cache it for subsequent requests.
   *
   * This is fire-and-forget — encryption unlock failure doesn't block the app.
   */
  private tryUnlockEncryption(): void {
    if (!this.auth.isLoggedIn()) return;

    this.tenantApi.getMyTenant().subscribe({
      next: tenant => {
        if (!tenant) return;
        if (tenant.encryption_mode === 'login_derived') {
          this.tenantApi.unlockEncryption(tenant.id).subscribe({
            next: res => {
              if (res.status === 'unlocked') {
                if (!environment.production) console.info('Encryption unlocked for tenant', tenant.slug);
              }
            },
            error: err => { if (!environment.production) console.warn('Failed to unlock encryption:', err); },
          });
        } else if (tenant.encryption_mode === 'passphrase') {
          // Passphrase mode requires user input — handled by the UI.
          // Signal that passphrase prompt is needed.
          if (!environment.production) console.info('Tenant uses passphrase encryption — prompt required');
          this.passphraseRequired = true;
        }
      },
      error: () => { /* No tenant — ignore */ },
    });
  }

  /** Whether the tenant requires a passphrase to unlock encryption. */
  passphraseRequired = false;

  /** Whether login failed (token exchange error) — prevents redirect loop. */
  private _loginFailed = false;
  get loginFailed(): boolean { return this._loginFailed; }

  private async checkAuthRequired(): Promise<boolean> {
    try {
      const res = await fetch(`${environment.apiServer}/config`);
      if (!res.ok) return true;
      const data = await res.json();
      return data.auth_required !== false;
    } catch {
      // API unreachable or error — assume auth required
      return true;
    }
  }
}
