import { ApplicationConfig, provideBrowserGlobalErrorListeners, provideZoneChangeDetection, importProvidersFrom, APP_INITIALIZER } from '@angular/core';
import { provideRouter, withInMemoryScrolling } from '@angular/router';
import { provideHttpClient, withFetch, withInterceptors } from '@angular/common/http';
import { provideTransloco } from '@jsverse/transloco';
import { isDevMode } from '@angular/core';
import { OAuthModule } from 'angular-oauth2-oidc';
import { routes } from './app.routes';
import { authInterceptor } from './core/interceptors/auth.interceptor';
import { StartupService } from './core/services/startup.service';
import { TranslocoHttpLoader } from './transloco-loader';

export function startupFactory(startup: StartupService) {
  return () => startup.init();
}

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
    provideZoneChangeDetection({ eventCoalescing: true }),
    provideHttpClient(withFetch(), withInterceptors([authInterceptor])),
    provideRouter(routes, withInMemoryScrolling({ scrollPositionRestoration: 'top' })),
    importProvidersFrom(OAuthModule.forRoot()),
    provideTransloco({
      config: {
        availableLangs: ['en', 'de'],
        defaultLang: 'en',
        fallbackLang: 'en',
        reRenderOnLangChange: true,
        prodMode: !isDevMode(),
        missingHandler: { logMissingKey: false },
      },
      loader: TranslocoHttpLoader,
    }),
    {
      provide: APP_INITIALIZER,
      useFactory: startupFactory,
      deps: [StartupService],
      multi: true,
    },
  ],
};
