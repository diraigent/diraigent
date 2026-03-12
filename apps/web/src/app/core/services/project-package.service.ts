import { Injectable, inject, signal, effect } from '@angular/core';
import { switchMap, catchError, of } from 'rxjs';
import { ProjectContext } from './project-context.service';
import { DiraigentApiService, DgProject } from './diraigent-api.service';
import { PackagesApiService, DgPackage } from './packages-api.service';

/** Default knowledge categories for the software-dev package (fallback). */
export const DEFAULT_KNOWLEDGE_CATEGORIES: string[] = [
  'architecture',
  'convention',
  'pattern',
  'anti_pattern',
  'setup',
  'general',
];

/**
 * Singleton service that tracks the active project's package definition.
 * Re-fetches whenever the selected project changes.
 */
@Injectable({ providedIn: 'root' })
export class ProjectPackageService {
  private ctx = inject(ProjectContext);
  private projectApi = inject(DiraigentApiService);
  private packagesApi = inject(PackagesApiService);

  readonly currentPackage = signal<DgPackage | null>(null);

  readonly knowledgeCategories = (): string[] =>
    this.currentPackage()?.allowed_knowledge_categories ?? DEFAULT_KNOWLEDGE_CATEGORIES;

  constructor() {
    effect(() => {
      const projectId = this.ctx.projectId();
      if (!projectId) {
        this.currentPackage.set(null);
        return;
      }

      this.projectApi.getProject(projectId).pipe(
        switchMap((project: DgProject) => {
          const packageId = project.package?.id;
          if (!packageId) return of(null);
          return this.packagesApi.getById(packageId).pipe(catchError(() => of(null)));
        }),
        catchError(() => of(null)),
      ).subscribe(pkg => this.currentPackage.set(pkg));
    });
  }
}
