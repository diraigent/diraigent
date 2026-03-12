import { Component, inject } from '@angular/core';
import { PLATFORM_ID } from '@angular/core';
import { isPlatformBrowser } from '@angular/common';
import { Router, ActivatedRoute } from '@angular/router';
import { AuthService } from '../../../../core/services/auth.service';

@Component({
  selector: 'app-auth-callback',
  standalone: true,
  template: `<div class="flex items-center justify-center h-screen text-text-secondary">{{ message }}</div>`,
})
export class AuthCallbackPage {
  private router = inject(Router);
  private route = inject(ActivatedRoute);
  private platformId = inject(PLATFORM_ID);
  private auth = inject(AuthService);

  message = 'Processing login...';

  constructor() {
    this.handleCallback();
  }

  private async handleCallback(): Promise<void> {
    if (!isPlatformBrowser(this.platformId)) {
      this.message = 'Callback received on server.';
      return;
    }

    const qp = this.route.snapshot.queryParamMap;
    const error = qp.get('error');

    if (error) {
      this.message = `OAuth error: ${error}`;
      return;
    }

    // The code exchange is handled by StartupService (APP_INITIALIZER) via
    // loadDiscoveryDocumentAndTryLogin(). By the time Angular routes resolve,
    // the initializer has already completed. Just check if we're logged in
    // and redirect.
    if (this.auth.isLoggedIn()) {
      this.message = 'Login complete — redirecting...';
    } else {
      this.message = 'Completing sign-in...';
    }
    this.router.navigate(['/']);
  }
}
