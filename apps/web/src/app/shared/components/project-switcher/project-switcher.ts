import { Component, inject, signal, OnInit, computed } from '@angular/core';
import { TranslocoModule } from '@jsverse/transloco';
import { DiraigentApiService, DgProject } from '../../../core/services/diraigent-api.service';
import { ProjectContext } from '../../../core/services/project-context.service';
import { CreateProjectService } from '../../services/create-project.service';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';

@Component({
  selector: 'app-project-switcher',
  standalone: true,
  imports: [TranslocoModule],
  template: `
    <ng-container *transloco="let t">
      <div class="flex gap-1.5">
        <select (change)="onSelect($event)"
                class="flex-1 min-w-0 bg-surface text-text-primary text-sm rounded-lg px-3 py-1.5 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent">
          @for (p of projects(); track p.id) {
            <option [value]="p.id" [selected]="p.id === ctx.projectId()">{{ p.parent_id ? '↳ ' : '' }}{{ p.name }}</option>
          }
          @if (projects().length === 0) {
            <option disabled selected>No projects</option>
          }
        </select>
        <button (click)="openCreateModal()" [title]="t('projects.create')"
          class="shrink-0 w-8 h-8 flex items-center justify-center bg-surface text-text-secondary
                 border border-border rounded-lg hover:text-accent hover:border-accent transition-colors">
          +
        </button>
      </div>
      @if (currentProject()?.parent_id) {
        <p class="mt-1 text-[10px] text-text-secondary flex items-center gap-1">
          <svg class="w-3 h-3 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
          </svg>
          {{ t('projects.childOf') }} <span class="font-medium text-text-primary ml-1">{{ parentName() }}</span>
        </p>
      }
    </ng-container>
  `,
})
export class ProjectSwitcherComponent implements OnInit {
  private api = inject(DiraigentApiService);
  private createProjectService = inject(CreateProjectService);
  ctx = inject(ProjectContext);

  projects = signal<DgProject[]>([]);

  readonly currentProject = computed(() =>
    this.projects().find(p => p.id === this.ctx.projectId()) ?? null
  );

  readonly parentName = computed(() => {
    const current = this.currentProject();
    if (!current?.parent_id) return '';
    return this.projects().find(p => p.id === current.parent_id)?.name ?? current.parent_id;
  });

  constructor() {
    // React to newly created projects from the root-level modal
    this.createProjectService.projectCreated$.pipe(takeUntilDestroyed()).subscribe((project) => {
      this.projects.update(ps => [...ps, project]);
      this.ctx.select(project.id);
    });
  }

  ngOnInit(): void {
    this.loadProjects();
  }

  private loadProjects(): void {
    this.api.getProjects().subscribe({
      next: (ps) => {
        this.projects.set(ps);
        if (ps.length > 0) {
          const stored = this.ctx.projectId();
          const match = ps.find(p => p.id === stored);
          if (!match) {
            this.ctx.select(ps[0].id);
          }
        }
      },
      error: () => {
        // API unavailable
      },
    });
  }

  onSelect(event: Event): void {
    const id = (event.target as HTMLSelectElement).value;
    this.ctx.select(id);
  }

  openCreateModal(): void {
    this.createProjectService.open(this.projects());
  }
}
