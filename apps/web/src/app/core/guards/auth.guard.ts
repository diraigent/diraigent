import { Injectable, inject, PLATFORM_ID } from '@angular/core';
import { CanActivate, ActivatedRouteSnapshot, RouterStateSnapshot, Router, UrlTree } from '@angular/router';
import { isPlatformBrowser } from '@angular/common';
import { Observable, of } from 'rxjs';
import { filter, take, map } from 'rxjs/operators';
import { AuthService } from '../services/auth.service';
import { StartupService } from '../services/startup.service';

@Injectable({ providedIn: 'root' })
export class AuthGuard implements CanActivate {
  private auth = inject(AuthService);
  private startup = inject(StartupService);
  private router = inject(Router);
  private platformId = inject(PLATFORM_ID);

  canActivate(
    _: ActivatedRouteSnapshot,
    __: RouterStateSnapshot,
  ): Observable<boolean | UrlTree> {
    if (!isPlatformBrowser(this.platformId)) {
      return of(true);
    }

    if (this.auth.isAuthInitialized()) {
      if (this.auth.isLoggedIn()) {
        return of(true);
      }
      if (this.startup.loginFailed) {
        console.error('Auth guard: login previously failed, not redirecting again');
        return of(true); // let through to avoid loop — page will show unauthenticated state
      }
      this.auth.login();
      return of(false);
    }

    return this.auth.authInitialized$.pipe(
      filter(initialized => initialized),
      take(1),
      map(() => {
        if (this.auth.isLoggedIn()) {
          return true;
        }
        if (this.startup.loginFailed) {
          console.error('Auth guard: login previously failed, not redirecting again');
          return true;
        }
        this.auth.login();
        return false;
      }),
    );
  }
}
