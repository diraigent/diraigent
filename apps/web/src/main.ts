import { bootstrapApplication } from '@angular/platform-browser';
import { appConfig } from './app/app.config';
import { App } from './app/app';
import { StartupService } from './app/core/services/startup.service';

async function main() {
  try {
    const appRef = await bootstrapApplication(App, appConfig);
    try {
      const startup = appRef.injector.get(StartupService);
      await startup.init();
    } catch (e) {
      console.warn('Startup init failed', e);
    }
  } catch (err) {
    console.error('Bootstrap failed', err);
  }
}

main();
