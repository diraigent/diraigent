import { Injectable, inject, signal, effect } from '@angular/core';
import { DiraigentApiService, DgProject } from './diraigent-api.service';

const STORAGE_KEY = 'diraigent-project';

@Injectable({ providedIn: 'root' })
export class ProjectContext {
  private api = inject(DiraigentApiService);

  readonly projectId = signal(localStorage.getItem(STORAGE_KEY) ?? '');
  readonly project = signal<DgProject | null>(null);

  constructor() {
    effect(() => {
      const pid = this.projectId();
      if (!pid) {
        this.project.set(null);
        return;
      }
      this.project.set(null); // clear while loading new project
      this.api.getProject(pid).subscribe({
        next: (p) => this.project.set(p),
        error: () => this.project.set(null),
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
