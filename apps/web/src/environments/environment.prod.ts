const w = (globalThis as any).__env || {};

export const environment = {
  production: true,
  apiServer: w.API_SERVER || 'https://api.diraigent.com/v1',
  authProviderBase: w.AUTH_PROVIDER_BASE || 'https://auth.diraigent.com',
  authIssuer: w.AUTH_ISSUER || 'https://auth.diraigent.com/application/o/diraigent/',
  authClientId: w.AUTH_CLIENT_ID || 'kvuNVJmjVOdhwSfmBDwlSMJw6XxExtjaib5wEDsu',
  authRedirectPath: w.AUTH_REDIRECT_PATH || '/auth/callback',
  authRedirectUri: w.AUTH_REDIRECT_URI || '',
};
