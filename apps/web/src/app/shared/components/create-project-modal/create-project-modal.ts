import { Component, inject, input, output, signal, OnInit, ElementRef, viewChild, AfterViewInit } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpErrorResponse } from '@angular/common/http';
import { TranslocoModule } from '@jsverse/transloco';
import { DiraigentApiService, DgProject, DgPackage, DgGitMode } from '../../../core/services/diraigent-api.service';
import { ModalWrapperComponent } from '../modal-wrapper/modal-wrapper';

@Component({
  selector: 'app-create-project-modal',
  standalone: true,
  imports: [FormsModule, TranslocoModule, ModalWrapperComponent],
  template: `
    <ng-container *transloco="let t">
      <app-modal-wrapper (closed)="onCancel()" maxWidth="max-w-xl" [scrollable]="true">
        <h2 class="text-lg font-semibold text-text-primary mb-5">{{ t('projects.createTitle') }}</h2>

        <div class="space-y-4">
          <!-- Name (required) -->
          <label class="block">
            <span class="block text-sm font-medium text-text-secondary mb-1">
              {{ t('projects.name') }} <span class="text-ctp-red">*</span>
            </span>
            <input #nameInput type="text" [(ngModel)]="name" [placeholder]="t('projects.namePlaceholder')"
              class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                     focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
          </label>

          <!-- Description -->
          <label class="block">
            <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.description') }}</span>
            <textarea [(ngModel)]="description" [placeholder]="t('projects.descriptionPlaceholder')" rows="2"
              class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                     focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary resize-y"></textarea>
          </label>

          <!-- Package -->
          <div class="block">
            <label for="cp-package" class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.package') }}</label>
            @if (loadingPackages()) {
              <p class="text-xs text-text-secondary">{{ t('common.loading') }}</p>
            } @else if (packageLoadError()) {
              <p class="text-xs text-ctp-red">{{ t('projects.packageLoadError') }}</p>
            } @else {
              <select id="cp-package" [(ngModel)]="packageSlug"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent">
                <option value="">{{ t('projects.packageDefault') }}</option>
                @for (pkg of packages(); track pkg.id) {
                  <option [value]="pkg.slug">{{ pkg.name }}</option>
                }
              </select>
              @if (selectedPackage) {
                <p class="mt-1 text-xs text-text-secondary">{{ selectedPackage.description }}</p>
              }
            }
          </div>

          <!-- Parent project -->
          @if (parentProjects().length > 0) {
            <label class="block">
              <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.parent') }}</span>
              <select [(ngModel)]="parentId" (ngModelChange)="onParentChange()"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent">
                <option value="">{{ t('projects.noParent') }}</option>
                @for (p of parentProjects(); track p.id) {
                  <option [value]="p.id">{{ p.name }}</option>
                }
              </select>
            </label>
          }

          <!-- Repo URL (hidden when parent project is selected — inherited from parent) -->
          @if (!parentId) {
            <label class="block">
              <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.repoUrl') }}</span>
              <input type="text" [(ngModel)]="repoUrl" placeholder="https://github.com/org/repo"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
            </label>
          }

          <!-- Git Mode -->
          <label class="block">
            <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.gitMode') }}</span>
            <select [(ngModel)]="gitMode"
              class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                     focus:outline-none focus:ring-1 focus:ring-accent">
              <option value="standalone">{{ t('projects.gitModeStandalone') }}</option>
              <option value="monorepo">{{ t('projects.gitModeMonorepo') }}</option>
              <option value="none">{{ t('projects.gitModeNone') }}</option>
            </select>
          </label>

          <!-- Git Root — path on disk relative to PROJECTS_PATH -->
          @if (gitMode !== 'none') {
            <label class="block">
              <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.gitRoot') }}</span>
              <input type="text" [(ngModel)]="gitRoot"
                [placeholder]="gitMode === 'monorepo' ? t('projects.gitRootPlaceholderMonorepo', { projectsPath: projectsPath() || 'PROJECTS_PATH' }) : t('projects.gitRootPlaceholderStandalone', { projectsPath: projectsPath() || 'PROJECTS_PATH' })"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
              <span class="block text-xs text-text-secondary mt-1">
                Full path: <code class="font-mono bg-surface px-1 rounded">{{ projectsPath() ?? '(PROJECTS_PATH not set)' }}/{{ gitRoot || '…' }}</code>
              </span>
            </label>
          }

          <!-- Monorepo: Project Root + Default Branch side by side -->
          @if (gitMode === 'monorepo') {
            <div class="grid grid-cols-2 gap-4">
              <label class="block">
                <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.projectRoot') }}</span>
                <input type="text" [(ngModel)]="projectRoot"
                  [placeholder]="t('projects.projectRootPlaceholder')"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
                <span class="block text-xs text-text-secondary mt-1">{{ t('projects.projectRootHint') }}</span>
              </label>
              <label class="block">
                <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.defaultBranch') }}</span>
                <input type="text" [(ngModel)]="defaultBranch" placeholder="main"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
              </label>
            </div>
          } @else if (gitMode !== 'none') {
            <!-- Standalone: default branch full-width -->
            <label class="block">
              <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.defaultBranch') }}</span>
              <input type="text" [(ngModel)]="defaultBranch" placeholder="main"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
            </label>
          }

          <!-- Service Name -->
          <label class="block">
            <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.serviceName') }}</span>
            <input type="text" [(ngModel)]="serviceName" [placeholder]="t('projects.serviceNamePlaceholder')"
              class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                     focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
            <span class="block text-xs text-text-secondary mt-1">{{ t('projects.serviceNameHint') }}</span>
          </label>

          @if (error()) {
            <p class="text-sm text-ctp-red">{{ error() }}</p>
          }

          <!-- Actions -->
          <div class="flex gap-3 pt-2">
            <button (click)="onCancel()" type="button"
              class="flex-1 px-4 py-2 text-sm text-text-secondary hover:text-text-primary border border-border
                     rounded-lg hover:bg-surface transition-colors">
              {{ t('common.cancel') }}
            </button>
            <button (click)="onSubmit()" type="button" [disabled]="!name.trim() || saving()"
              class="flex-1 px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg
                     hover:opacity-90 disabled:opacity-50 transition-opacity">
              @if (saving()) {
                {{ t('common.saving') }}
              } @else {
                {{ t('projects.create') }}
              }
            </button>
          </div>
        </div>
      </app-modal-wrapper>
    </ng-container>
  `,
})
export class CreateProjectModalComponent implements OnInit, AfterViewInit {
  private api = inject(DiraigentApiService);

  /** Projects list for parent selection */
  parentProjects = input<DgProject[]>([]);

  /** Emits the newly created project on success */
  created = output<DgProject>();
  cancelled = output<void>();

  /** Autofocus the name field on open */
  nameInput = viewChild<ElementRef>('nameInput');

  packages = signal<DgPackage[]>([]);
  loadingPackages = signal(true);
  packageLoadError = signal(false);
  saving = signal(false);
  error = signal('');
  projectsPath = signal<string | null>(null);

  name = '';
  description = '';
  packageSlug = '';
  parentId = '';
  repoUrl = '';
  defaultBranch = '';
  serviceName = '';
  gitMode: DgGitMode = 'standalone';
  gitRoot = '';
  projectRoot = '';

  get selectedPackage(): DgPackage | null {
    if (!this.packageSlug) return null;
    return this.packages().find(p => p.slug === this.packageSlug) ?? null;
  }

  ngOnInit(): void {
    this.api.getPackages().subscribe({
      next: (pkgs) => {
        this.packages.set(pkgs);
        // Default to software-dev if available
        const def = pkgs.find(p => p.slug === 'software-dev');
        if (def) this.packageSlug = def.slug;
        this.loadingPackages.set(false);
      },
      error: () => {
        this.loadingPackages.set(false);
        this.packageLoadError.set(true);
      },
    });
    this.api.getSettings().subscribe({
      next: (settings) => this.projectsPath.set(settings.projects_path),
      error: () => { /* settings fetch is best-effort */ },
    });
  }

  ngAfterViewInit(): void {
    // Focus the name input after the modal opens
    setTimeout(() => this.nameInput()?.nativeElement?.focus(), 0);
  }

  onParentChange(): void {
    // Clear repo URL when a parent is selected — it's inherited from the parent
    if (this.parentId) {
      this.repoUrl = '';
    }
  }

  onCancel(): void {
    this.cancelled.emit();
  }

  onSubmit(): void {
    const name = this.name.trim();
    if (!name) return;

    this.saving.set(true);
    this.error.set('');

    const req = {
      name,
      ...(this.description.trim() && { description: this.description.trim() }),
      ...(this.packageSlug && { package_slug: this.packageSlug }),
      ...(this.parentId && { parent_id: this.parentId }),
      ...(!this.parentId && this.repoUrl.trim() && { repo_url: this.repoUrl.trim() }),
      ...(this.defaultBranch.trim() && { default_branch: this.defaultBranch.trim() }),
      ...(this.serviceName.trim() && { service_name: this.serviceName.trim() }),
      git_mode: this.gitMode,
      ...(this.gitRoot.trim() && { git_root: this.gitRoot.trim() }),
      ...(this.projectRoot.trim() && { project_root: this.projectRoot.trim() }),
    };

    this.api.createProject(req).subscribe({
      next: (project) => {
        this.saving.set(false);
        this.created.emit(project);
      },
      error: (err: HttpErrorResponse) => {
        this.saving.set(false);
        const detail = err.error?.error || err.error?.message || err.message;
        this.error.set(detail || 'Failed to create project. Please try again.');
      },
    });
  }
}
