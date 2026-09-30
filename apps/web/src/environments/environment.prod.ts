const w = (globalThis as unknown as Record<string, Record<string, string>>)['__env'] || {};

export const environment = {
  production: true,
  apiServer: w['API_SERVER'] || 'https://api.diraigent.com/v1',
  authProviderBase: w['AUTH_PROVIDER_BASE'] || '',
  authIssuer: w['AUTH_ISSUER'] || '',
  authClientId: w['AUTH_CLIENT_ID'] || '',
  authRedirectPath: w['AUTH_REDIRECT_PATH'] || '/auth/callback',
  authRedirectUri: w['AUTH_REDIRECT_URI'] || '',
  authEnrollmentUrl: w['AUTH_ENROLLMENT_URL'] || '',
  appVersion: w['APP_VERSION'] || '0.1.0',
};
