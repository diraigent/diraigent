import { AuthConfig } from 'angular-oauth2-oidc';
import { environment } from '../../../environments/environment';

export function getAuthConfig(): AuthConfig {
  const origin = typeof window !== 'undefined' && window.location?.origin
    ? window.location.origin
    : 'http://localhost:4200';
  const redirectUri = environment.authRedirectUri || `${origin}${environment.authRedirectPath || '/auth/callback'}`;

  const issuer = environment.authIssuer
    || (environment.authProviderBase ? environment.authProviderBase.replace(/\/$/, '') + '/' : undefined);

  return {
    issuer,
    redirectUri,
    clientId: environment.authClientId,
    responseType: 'code',
    scope: 'openid profile email offline_access',
    showDebugInformation: !environment.production,
    disableAtHashCheck: true,
    // Use refresh tokens instead of iframe-based silent refresh.
    // Prevents full-page redirects that destroy unsaved work.
    useSilentRefresh: false,
    timeoutFactor: 0.75,
  };
}
