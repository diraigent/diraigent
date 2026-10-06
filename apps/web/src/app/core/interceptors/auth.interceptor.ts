import { HttpErrorResponse, HttpInterceptorFn } from '@angular/common/http';
import { inject } from '@angular/core';
import { OAuthService } from 'angular-oauth2-oidc';
import { tap, throwError } from 'rxjs';
import { AuthService } from '../services/auth.service';
import { StartupService } from '../services/startup.service';
import { environment } from '../../../environments/environment';

export const authInterceptor: HttpInterceptorFn = (req, next) => {
  const publicBase = `${environment.apiServer}/spectator/`;
  if (req.url.startsWith(publicBase)) {
    return next(req.clone({
      headers: req.headers.delete('Authorization').delete('X-Dev-User-Id'),
      withCredentials: false,
      credentials: 'omit',
    }));
  }

  const oauth = inject(OAuthService);
  const auth = inject(AuthService);
  const startup = inject(StartupService);

  if (!req.url.startsWith(environment.apiServer)) {
    return next(req);
  }

  if (auth.readOnly() && !['GET', 'HEAD'].includes(req.method)) {
    return throwError(() => new HttpErrorResponse({
      status: 403, statusText: 'Read-only account',
      error: { error: 'This account is read-only' }, url: req.url,
    }));
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
