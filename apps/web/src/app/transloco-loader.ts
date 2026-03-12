import { inject, Injectable } from '@angular/core';
import { Translation, TranslocoLoader } from '@jsverse/transloco';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

@Injectable({ providedIn: 'root' })
export class TranslocoHttpLoader implements TranslocoLoader {
  private http = inject(HttpClient);

  getTranslation(lang: string): Observable<Translation> {
    return this.http.get<Translation>(`/assets/i18n/${lang}.json`);
  }
}
