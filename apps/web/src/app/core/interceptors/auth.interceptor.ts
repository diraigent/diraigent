import { HttpInterceptorFn } from '@angular/common/http';
import { inject } from '@angular/core';
import { OAuthService } from 'angular-oauth2-oidc';
import { tap } from 'rxjs';
import { AuthService } from '../services/auth.service';
import { StartupService } from '../services/startup.service';
import { environment } from '../../../environments/environment';

export const authInterceptor: HttpInterceptorFn = (req, next) => {
  const oauth = inject(OAuthService);
  const auth = inject(AuthService);
  const startup = inject(StartupService);

  if (!req.url.startsWith(environment.apiServer)) {
    return next(req);
  }

  const token = oauth.getAccessToken();
  if (!token) {
    return next(req);
  }

  const authReq = req.clone({
    setHeaders: { Authorization: `Bearer ${token}` },
  });

  return next(authReq).pipe(
    tap({
      error: (err) => {
        if (err.status === 401 && !startup.loginFailed) {
          auth.clearSession(true);
        }
      },
    }),
  );
};
