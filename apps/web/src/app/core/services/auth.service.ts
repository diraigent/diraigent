import { Injectable, inject } from '@angular/core';
import { PLATFORM_ID } from '@angular/core';
import { isPlatformBrowser } from '@angular/common';
import { Router } from '@angular/router';
import { OAuthService } from 'angular-oauth2-oidc';
import { BehaviorSubject, Observable } from 'rxjs';
import { distinctUntilChanged } from 'rxjs/operators';
import { environment } from '../../../environments/environment';

@Injectable({ providedIn: 'root' })
export class AuthService {
  private _isLoggedIn = new BehaviorSubject<boolean>(false);
  readonly isLoggedIn$: Observable<boolean> = this._isLoggedIn.pipe(distinctUntilChanged());

  private _authInitialized = new BehaviorSubject<boolean>(false);
  readonly authInitialized$: Observable<boolean> = this._authInitialized.pipe(distinctUntilChanged());

  private _authDisabled = false;

  private _user = new BehaviorSubject<Record<string, unknown> | null>(null);
  readonly user$: Observable<Record<string, unknown> | null> = this._user.asObservable();

  private platformId = inject<object>(PLATFORM_ID);
  private oauth = inject(OAuthService);
  private router = inject(Router);

  constructor() {
    if (isPlatformBrowser(this.platformId)) {
      try {
        const oauthEvents = (this.oauth as unknown as { events: Observable<{ type: string }> }).events;
        if (oauthEvents && typeof oauthEvents.subscribe === 'function') {
          oauthEvents.subscribe((event: { type: string }) => {
            if (event.type === 'token_received' || event.type === 'token_refreshed') {
              this._isLoggedIn.next(true);
              const claims = this.oauth.getIdentityClaims() as Record<string, unknown>;
              this._user.next(claims || null);
            } else if (event.type === 'logout') {
              this._isLoggedIn.next(false);
              this._user.next(null);
            } else if (
              event.type === 'silent_refresh_error' ||
              event.type === 'token_error'
            ) {
              this.clearSession(true);
            }
          });
        }
      } catch {
        // OAuth unavailable during init
      }
    }
  }

  markAuthDisabled(): void {
    this._authDisabled = true;
    this._isLoggedIn.next(true);
    this._authInitialized.next(true);
  }

  isAuthDisabled(): boolean {
    return this._authDisabled;
  }

  markInitialized(): void {
    try {
      if (this.oauth.hasValidAccessToken()) {
        this._isLoggedIn.next(true);
        const claims = this.oauth.getIdentityClaims() as Record<string, unknown>;
        this._user.next(claims || null);
      } else {
        this._isLoggedIn.next(false);
        this._user.next(null);
      }
    } catch {
      // ignore
    }
    this._authInitialized.next(true);
  }

  login(): void {
    if (!isPlatformBrowser(this.platformId)) return;
    try {
      this.oauth.initCodeFlow();
    } catch {
      // ignored
    }
  }

  logout(): void {
    if (isPlatformBrowser(this.platformId)) {
      this.clearSession();
      this.oauth.logOut();
    }
  }

  clearSession(redirectHome = false): void {
    this._isLoggedIn.next(false);
    this._user.next(null);
    if (redirectHome) {
      this.router.navigate(['/']);
    }
  }

  isLoggedIn(): boolean {
    return this._isLoggedIn.value;
  }

  isAuthInitialized(): boolean {
    return this._authInitialized.value;
  }

  get registrationUrl(): string {
    return environment.authEnrollmentUrl;
  }

  getAccessToken(): string | null {
    try {
      return this.oauth.getAccessToken() || null;
    } catch {
      return null;
    }
  }
}
