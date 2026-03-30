import { Component, inject } from '@angular/core';
import { PLATFORM_ID } from '@angular/core';
import { isPlatformBrowser } from '@angular/common';
import { AuthService } from '../../../../core/services/auth.service';

@Component({
  selector: 'app-logout',
  standalone: true,
  template: `
    <div class="flex flex-col items-center justify-center h-screen gap-4 text-text-secondary">
      <p>You have been logged out.</p>
      <button
        class="px-4 py-2 rounded bg-surface-1 hover:bg-surface-2 text-text-primary transition-colors"
        (click)="login()"
      >
        Sign in again
      </button>
    </div>
  `,
})
export class LogoutPage {
  private platformId = inject(PLATFORM_ID);
  private auth = inject(AuthService);

  constructor() {
    if (isPlatformBrowser(this.platformId)) {
      this.auth.clearSession();
    }
  }

  login(): void {
    this.auth.login();
  }
}