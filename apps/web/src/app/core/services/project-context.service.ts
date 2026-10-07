import { Injectable, inject, signal, effect, untracked } from '@angular/core';
import { DiraigentApiService, DgProject } from './diraigent-api.service';
import { HttpClient } from '@angular/common/http';
import { AuthService } from './auth.service';
import { environment } from '../../../environments/environment';
import { STORAGE_KEYS } from '../../shared/ui-constants';

const STORAGE_KEY = STORAGE_KEYS.PROJECT;

@Injectable({ providedIn: 'root' })
export class ProjectContext {
  private api = inject(DiraigentApiService);
  private http = inject(HttpClient);
  private auth = inject(AuthService);

  readonly projectId = signal(localStorage.getItem(STORAGE_KEY) ?? '');
  readonly project = signal<DgProject | null>(null);

  constructor() {
    effect(() => {
      const pid = this.projectId();
      untracked(() => {
        this.auth.projectReadOnly.set(!!pid);
        if (!pid) {
          this.project.set(null);
          return;
        }
        this.http.get<{read_only:boolean}>(`${environment.apiServer}/${pid}/people/me`).subscribe({
          next: access => { if(this.projectId()===pid) this.auth.projectReadOnly.set(access.read_only!==false); },
          error: () => { if(this.projectId()===pid) this.auth.projectReadOnly.set(true); },
        });
        this.project.set(null); // clear while loading new project
        this.api.getProject(pid).subscribe({
          next: (p) => { if (this.projectId() === pid) this.project.set(p); },
          error: () => { if (this.projectId() === pid) this.project.set(null); },
        });
      });
    });
  }

  select(id: string): void {
    localStorage.setItem(STORAGE_KEY, id);
    this.projectId.set(id);
  }

  clear(): void {
    localStorage.removeItem(STORAGE_KEY);
    this.projectId.set('');
    this.project.set(null);
  }
}
