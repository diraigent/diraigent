import { Component, inject, input, output, signal, OnInit, ElementRef, viewChild, AfterViewInit } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpErrorResponse } from '@angular/common/http';
import { TranslocoModule } from '@jsverse/transloco';
import { catchError, of } from 'rxjs';
import { DiraigentApiService, DgProject, DgPackage, DgGitMode } from '../../../core/services/diraigent-api.service';
import { PlaybooksApiService, SpPlaybook } from '../../../core/services/playbooks-api.service';
import { AgentsApiService, SpAgent } from '../../../core/services/agents-api.service';
import { SpRole, TeamApiService } from '../../../core/services/team-api.service';
import { ModalWrapperComponent } from '../modal-wrapper/modal-wrapper';

type WizardStep = 'project' | 'playbook' | 'agent' | 'done';

const STEP_LIST: WizardStep[] = ['project', 'playbook', 'agent', 'done'];

@Component({
  selector: 'app-create-project-modal',
  standalone: true,
  imports: [FormsModule, TranslocoModule, ModalWrapperComponent],
  template: `
    <ng-container *transloco="let t">
      <app-modal-wrapper (closed)="onCancel()" maxWidth="max-w-2xl" [scrollable]="true">

        <!-- Progress bar -->
        <div class="flex items-center gap-2 mb-6">
          @for (s of stepList; track s; let i = $index) {
            <div class="flex items-center gap-2" [class.flex-1]="i < stepList.length - 1">
              <div class="w-7 h-7 rounded-full flex items-center justify-center text-xs font-semibold shrink-0 transition-colors"
                [class.bg-accent]="stepIndex() >= i"
                [class.text-bg]="stepIndex() >= i"
                [class.bg-surface]="stepIndex() < i"
                [class.text-text-secondary]="stepIndex() < i"
                [class.border]="stepIndex() < i"
                [class.border-border]="stepIndex() < i">
                @if (stepIndex() > i) {
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>
                  </svg>
                } @else {
                  {{ i + 1 }}
                }
              </div>
              @if (i < stepList.length - 1) {
                <div class="flex-1 h-0.5 rounded transition-colors"
                  [class.bg-accent]="stepIndex() > i"
                  [class.bg-border]="stepIndex() <= i"></div>
              }
            </div>
          }
        </div>

        <!-- Step 1: Project Details -->
        @if (step() === 'project') {
          <h2 class="text-lg font-semibold text-text-primary mb-1">{{ t('projects.createTitle') }}</h2>
          <p class="text-sm text-text-secondary mb-5">{{ t('projects.wizard.step1Hint') }}</p>

          <div class="space-y-6">

            <!-- ── Section: Basics ── -->
            <fieldset class="space-y-4">
              <legend class="text-xs font-semibold uppercase tracking-wide text-text-secondary mb-1">{{ t('projects.wizard.sectionBasics') }}</legend>

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
                <p class="text-xs text-text-secondary mb-1.5">{{ t('projects.wizard.packageHint') }}</p>
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
                  <p class="text-xs text-text-secondary mb-1.5">{{ t('projects.wizard.parentHint') }}</p>
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
            </fieldset>

            <hr class="border-border" />

            <!-- ── Section: Source Code ── -->
            <fieldset class="space-y-4">
              <legend class="text-xs font-semibold uppercase tracking-wide text-text-secondary mb-1">{{ t('projects.wizard.sectionSource') }}</legend>
              <p class="text-xs text-text-secondary -mt-1">{{ t('projects.wizard.sectionSourceHint') }}</p>

              <!-- Repo URL (hidden when parent project is selected — inherited from parent) -->
              @if (!parentId) {
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.repoUrl') }}</span>
                  <p class="text-xs text-text-secondary mb-1.5">{{ t('projects.wizard.repoUrlHint') }}</p>
                  <input type="text" [(ngModel)]="repoUrl" placeholder="https://github.com/org/repo"
                    class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
                </label>
              }

              <!-- Git Mode — radio cards -->
              <div>
                <span class="block text-sm font-medium text-text-secondary mb-2">{{ t('projects.wizard.gitModeLabel') }}</span>
                <div class="grid grid-cols-3 gap-3">
                  @for (opt of gitModeOptions; track opt.value) {
                    <button (click)="gitMode = opt.value" type="button"
                      class="text-left p-3 rounded-lg border transition-colors"
                      [class.border-accent]="gitMode === opt.value"
                      [class.bg-accent/5]="gitMode === opt.value"
                      [class.border-border]="gitMode !== opt.value"
                      [class.hover:border-accent]="gitMode !== opt.value">
                      <span class="block text-sm font-medium text-text-primary">{{ t(opt.labelKey) }}</span>
                      <span class="block text-xs text-text-secondary mt-1">{{ t(opt.descKey) }}</span>
                    </button>
                  }
                </div>
              </div>

              <!-- Git Root — path on disk relative to PROJECTS_PATH -->
              @if (gitMode !== 'none') {
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.gitRoot') }}</span>
                  <p class="text-xs text-text-secondary mb-1.5">{{ t('projects.wizard.gitRootHint') }}</p>
                  <input type="text" [(ngModel)]="gitRoot"
                    [placeholder]="gitMode === 'monorepo' ? t('projects.gitRootPlaceholderMonorepo', { projectsPath: projectsPath() || 'PROJECTS_PATH' }) : t('projects.gitRootPlaceholderStandalone', { projectsPath: projectsPath() || 'PROJECTS_PATH' })"
                    class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
                  <span class="block text-xs text-text-secondary mt-1">
                    Full path: <code class="font-mono bg-surface px-1 rounded">{{ projectsPath() ?? '(PROJECTS_PATH not set)' }}/{{ gitRoot || '…' }}</code>
                  </span>
                </label>
              }

              <!-- Default Branch -->
              @if (gitMode !== 'none') {
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.defaultBranch') }}</span>
                  <p class="text-xs text-text-secondary mb-1.5">{{ t('projects.wizard.defaultBranchHint') }}</p>
                  <input type="text" [(ngModel)]="defaultBranch" placeholder="main"
                    class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
                </label>
              }

              <!-- Monorepo: Project Root -->
              @if (gitMode === 'monorepo') {
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.projectRoot') }}</span>
                  <p class="text-xs text-text-secondary mb-1.5">{{ t('projects.wizard.projectRootHint') }}</p>
                  <input type="text" [(ngModel)]="projectRoot"
                    [placeholder]="t('projects.projectRootPlaceholder')"
                    class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
                </label>
              }
            </fieldset>

            <hr class="border-border" />

            <!-- ── Section: Integrations (optional) ── -->
            <fieldset class="space-y-4">
              <legend class="text-xs font-semibold uppercase tracking-wide text-text-secondary mb-1">{{ t('projects.wizard.sectionIntegrations') }}</legend>

              <!-- Service Name -->
              <label class="block">
                <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.serviceName') }}</span>
                <p class="text-xs text-text-secondary mb-1.5">{{ t('projects.wizard.serviceNameHint') }}</p>
                <input type="text" [(ngModel)]="serviceName" [placeholder]="t('projects.serviceNamePlaceholder')"
                  class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                         focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
              </label>
            </fieldset>

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
              <button (click)="onSubmitProject()" type="button" [disabled]="!name.trim() || saving()"
                class="flex-1 px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg
                       hover:opacity-90 disabled:opacity-50 transition-opacity">
                @if (saving()) {
                  {{ t('common.saving') }}
                } @else {
                  {{ t('common.next') }}
                }
              </button>
            </div>
          </div>
        }

        <!-- Step 2: Select Default Playbook -->
        @if (step() === 'playbook') {
          <h2 class="text-lg font-semibold text-text-primary mb-2">{{ t('projects.wizard.playbookTitle') }}</h2>
          <p class="text-sm text-text-secondary mb-5">{{ t('projects.wizard.playbookHint') }}</p>

          <div class="space-y-4">
            @if (loadingPlaybooks()) {
              <p class="text-sm text-text-secondary">{{ t('common.loading') }}</p>
            } @else {
              <!-- Default Playbooks -->
              @if (defaultPlaybooks().length > 0) {
                <div>
                  <span class="block text-xs font-medium text-text-secondary uppercase tracking-wide mb-2">{{ t('projects.wizard.defaultPlaybooks') }}</span>
                  <div class="grid grid-cols-2 gap-3">
                    @for (pb of defaultPlaybooks(); track pb.id) {
                      <button (click)="selectPlaybook(pb)" type="button"
                        class="text-left p-3 rounded-lg border transition-colors"
                        [class.border-accent]="selectedPlaybook()?.id === pb.id"
                        [class.bg-accent/5]="selectedPlaybook()?.id === pb.id"
                        [class.border-border]="selectedPlaybook()?.id !== pb.id"
                        [class.hover:border-accent]="selectedPlaybook()?.id !== pb.id">
                        <span class="block text-sm font-medium text-text-primary">{{ pb.title }}</span>
                        <span class="block text-xs text-text-secondary mt-1 line-clamp-2">{{ pb.trigger_description }}</span>
                        @if (pb.steps.length > 0) {
                          <span class="block text-xs text-text-secondary mt-1">{{ pb.steps.length }} {{ pb.steps.length === 1 ? 'step' : 'steps' }}</span>
                        }
                      </button>
                    }
                  </div>
                </div>
              }

              <!-- Your Playbooks -->
              @if (tenantPlaybooks().length > 0) {
                <div>
                  <span class="block text-xs font-medium text-text-secondary uppercase tracking-wide mb-2">{{ t('projects.wizard.yourPlaybooks') }}</span>
                  <div class="grid grid-cols-2 gap-3">
                    @for (pb of tenantPlaybooks(); track pb.id) {
                      <button (click)="selectPlaybook(pb)" type="button"
                        class="text-left p-3 rounded-lg border transition-colors"
                        [class.border-accent]="selectedPlaybook()?.id === pb.id"
                        [class.bg-accent/5]="selectedPlaybook()?.id === pb.id"
                        [class.border-border]="selectedPlaybook()?.id !== pb.id"
                        [class.hover:border-accent]="selectedPlaybook()?.id !== pb.id">
                        <span class="block text-sm font-medium text-text-primary">{{ pb.title }}</span>
                        <span class="block text-xs text-text-secondary mt-1 line-clamp-2">{{ pb.trigger_description }}</span>
                        @if (pb.steps.length > 0) {
                          <span class="block text-xs text-text-secondary mt-1">{{ pb.steps.length }} {{ pb.steps.length === 1 ? 'step' : 'steps' }}</span>
                        }
                      </button>
                    }
                  </div>
                </div>
              }

              @if (defaultPlaybooks().length === 0 && tenantPlaybooks().length === 0) {
                <p class="text-sm text-text-secondary">{{ t('projects.wizard.noPlaybooks') }}</p>
              }
            }

            @if (playbookError()) {
              <p class="text-sm text-ctp-red">{{ playbookError() }}</p>
            }

            <div class="flex gap-3 pt-2">
              <button (click)="skipPlaybook()" type="button"
                class="flex-1 px-4 py-2 text-sm text-text-secondary hover:text-text-primary border border-border
                       rounded-lg hover:bg-surface transition-colors">
                {{ t('common.skip') }}
              </button>
              <button (click)="confirmPlaybook()" type="button" [disabled]="!selectedPlaybook() || savingPlaybook()"
                class="flex-1 px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg
                       hover:opacity-90 disabled:opacity-50 transition-opacity">
                @if (savingPlaybook()) {
                  {{ t('common.saving') }}
                } @else {
                  {{ t('projects.wizard.setPlaybook') }}
                }
              </button>
            </div>
          </div>
        }

        <!-- Step 3: Assign Agent -->
        @if (step() === 'agent') {
          <h2 class="text-lg font-semibold text-text-primary mb-2">{{ t('projects.wizard.agentTitle') }}</h2>
          <p class="text-sm text-text-secondary mb-5">{{ t('projects.wizard.agentHint') }}</p>

          <div class="space-y-4">
            @if (loadingAgents()) {
              <p class="text-sm text-text-secondary">{{ t('common.loading') }}</p>
            } @else {
              @if (agents().length === 0 || roles().length === 0) {
                <div class="bg-surface rounded-lg p-4 border border-border">
                  <p class="text-sm text-text-secondary">{{ t('projects.wizard.noAgentsOrRoles') }}</p>
                </div>
              } @else {
                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.wizard.agent') }}</span>
                  <select [(ngModel)]="selectedAgentId"
                    class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent">
                    <option value="">{{ t('projects.wizard.selectAgent') }}</option>
                    @for (a of agents(); track a.id) {
                      <option [value]="a.id">{{ a.name }}</option>
                    }
                  </select>
                </label>

                <label class="block">
                  <span class="block text-sm font-medium text-text-secondary mb-1">{{ t('projects.wizard.role') }}</span>
                  <select [(ngModel)]="selectedRoleId"
                    class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                           focus:outline-none focus:ring-1 focus:ring-accent">
                    <option value="">{{ t('projects.wizard.selectRole') }}</option>
                    @for (r of roles(); track r.id) {
                      <option [value]="r.id">{{ r.name }}</option>
                    }
                  </select>
                </label>
              }
            }

            @if (agentError()) {
              <p class="text-sm text-ctp-red">{{ agentError() }}</p>
            }

            <div class="flex gap-3 pt-2">
              <button (click)="skipAgent()" type="button"
                class="flex-1 px-4 py-2 text-sm text-text-secondary hover:text-text-primary border border-border
                       rounded-lg hover:bg-surface transition-colors">
                {{ t('common.skip') }}
              </button>
              <button (click)="confirmAgent()" type="button"
                [disabled]="!selectedAgentId || !selectedRoleId || savingAgent()"
                class="flex-1 px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg
                       hover:opacity-90 disabled:opacity-50 transition-opacity">
                @if (savingAgent()) {
                  {{ t('common.saving') }}
                } @else {
                  {{ t('projects.wizard.assignAgent') }}
                }
              </button>
            </div>
          </div>
        }

        <!-- Step 4: Summary -->
        @if (step() === 'done') {
          <h2 class="text-lg font-semibold text-text-primary mb-5">{{ t('projects.wizard.doneTitle') }}</h2>

          <div class="space-y-3">
            <!-- Project -->
            <div class="flex items-start gap-3 p-3 rounded-lg bg-surface border border-border">
              <div class="w-6 h-6 rounded-full bg-ctp-green/20 flex items-center justify-center shrink-0 mt-0.5">
                <svg class="w-3.5 h-3.5 text-ctp-green" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>
                </svg>
              </div>
              <div>
                <span class="block text-sm font-medium text-text-primary">{{ t('projects.wizard.summaryProject') }}</span>
                <span class="block text-xs text-text-secondary">{{ createdProject()?.name }}</span>
              </div>
            </div>

            <!-- Playbook -->
            <div class="flex items-start gap-3 p-3 rounded-lg bg-surface border border-border">
              @if (configuredPlaybook()) {
                <div class="w-6 h-6 rounded-full bg-ctp-green/20 flex items-center justify-center shrink-0 mt-0.5">
                  <svg class="w-3.5 h-3.5 text-ctp-green" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>
                  </svg>
                </div>
                <div>
                  <span class="block text-sm font-medium text-text-primary">{{ t('projects.wizard.summaryPlaybook') }}</span>
                  <span class="block text-xs text-text-secondary">{{ configuredPlaybook()?.title }}</span>
                </div>
              } @else {
                <div class="w-6 h-6 rounded-full bg-surface flex items-center justify-center shrink-0 mt-0.5 border border-border">
                  <span class="text-xs text-text-secondary">—</span>
                </div>
                <div>
                  <span class="block text-sm font-medium text-text-secondary">{{ t('projects.wizard.summaryPlaybook') }}</span>
                  <span class="block text-xs text-text-secondary">{{ t('projects.wizard.skipped') }}</span>
                </div>
              }
            </div>

            <!-- Agent -->
            <div class="flex items-start gap-3 p-3 rounded-lg bg-surface border border-border">
              @if (configuredAgent()) {
                <div class="w-6 h-6 rounded-full bg-ctp-green/20 flex items-center justify-center shrink-0 mt-0.5">
                  <svg class="w-3.5 h-3.5 text-ctp-green" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>
                  </svg>
                </div>
                <div>
                  <span class="block text-sm font-medium text-text-primary">{{ t('projects.wizard.summaryAgent') }}</span>
                  <span class="block text-xs text-text-secondary">{{ configuredAgent()?.name }} · {{ configuredRole()?.name }}</span>
                </div>
              } @else {
                <div class="w-6 h-6 rounded-full bg-surface flex items-center justify-center shrink-0 mt-0.5 border border-border">
                  <span class="text-xs text-text-secondary">—</span>
                </div>
                <div>
                  <span class="block text-sm font-medium text-text-secondary">{{ t('projects.wizard.summaryAgent') }}</span>
                  <span class="block text-xs text-text-secondary">{{ t('projects.wizard.skipped') }}</span>
                </div>
              }
            </div>

            <button (click)="onDone()" type="button"
              class="w-full px-4 py-2 text-sm font-medium bg-accent text-bg rounded-lg
                     hover:opacity-90 transition-opacity mt-4">
              {{ t('common.done') }}
            </button>
          </div>
        }

      </app-modal-wrapper>
    </ng-container>
  `,
})
export class CreateProjectModalComponent implements OnInit, AfterViewInit {
  private api = inject(DiraigentApiService);
  private playbooksApi = inject(PlaybooksApiService);
  private agentsApi = inject(AgentsApiService);
  private teamApi = inject(TeamApiService);

  /** Projects list for parent selection */
  parentProjects = input<DgProject[]>([]);

  /** Emits the newly created project on success */
  created = output<DgProject>();
  cancelled = output<void>();

  /** Autofocus the name field on open */
  nameInput = viewChild<ElementRef>('nameInput');

  // Step management
  step = signal<WizardStep>('project');
  stepIndex = signal(0);
  readonly stepList = STEP_LIST;

  // Step 1: Project details
  packages = signal<DgPackage[]>([]);
  loadingPackages = signal(true);
  packageLoadError = signal(false);
  saving = signal(false);
  error = signal('');
  projectsPath = signal<string | null>(null);
  createdProject = signal<DgProject | null>(null);

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

  readonly gitModeOptions: { value: DgGitMode; labelKey: string; descKey: string }[] = [
    { value: 'standalone', labelKey: 'projects.wizard.gitStandaloneLabel', descKey: 'projects.wizard.gitStandaloneDesc' },
    { value: 'monorepo', labelKey: 'projects.wizard.gitMonorepoLabel', descKey: 'projects.wizard.gitMonorepoDesc' },
    { value: 'none', labelKey: 'projects.wizard.gitNoneLabel', descKey: 'projects.wizard.gitNoneDesc' },
  ];

  // Step 2: Playbook selection
  playbooks = signal<SpPlaybook[]>([]);
  loadingPlaybooks = signal(false);
  selectedPlaybook = signal<SpPlaybook | null>(null);
  savingPlaybook = signal(false);
  playbookError = signal('');
  configuredPlaybook = signal<SpPlaybook | null>(null);

  // Step 3: Agent assignment
  agents = signal<SpAgent[]>([]);
  roles = signal<SpRole[]>([]);
  loadingAgents = signal(false);
  savingAgent = signal(false);
  agentError = signal('');
  configuredAgent = signal<SpAgent | null>(null);
  configuredRole = signal<SpRole | null>(null);
  selectedAgentId = '';
  selectedRoleId = '';

  get selectedPackage(): DgPackage | null {
    if (!this.packageSlug) return null;
    return this.packages().find(p => p.slug === this.packageSlug) ?? null;
  }

  get defaultPlaybooks(): () => SpPlaybook[] {
    return () => this.playbooks().filter(p => p.tenant_id === null);
  }

  get tenantPlaybooks(): () => SpPlaybook[] {
    return () => this.playbooks().filter(p => p.tenant_id !== null);
  }

  ngOnInit(): void {
    this.api.getPackages().subscribe({
      next: (pkgs) => {
        this.packages.set(pkgs);
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
    setTimeout(() => this.nameInput()?.nativeElement?.focus(), 0);
  }

  onParentChange(): void {
    if (this.parentId) {
      this.repoUrl = '';
    }
  }

  onCancel(): void {
    // If project was already created, emit it so the list refreshes
    const project = this.createdProject();
    if (project) {
      this.created.emit(project);
    } else {
      this.cancelled.emit();
    }
  }

  /** Step 1 → create project, then advance to playbook step */
  onSubmitProject(): void {
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
        this.createdProject.set(project);
        this.goToPlaybookStep();
      },
      error: (err: HttpErrorResponse) => {
        this.saving.set(false);
        const detail = err.error?.error || err.error?.message || err.message;
        this.error.set(detail || 'Failed to create project. Please try again.');
      },
    });
  }

  /** Load playbooks and advance to step 2 */
  private goToPlaybookStep(): void {
    this.step.set('playbook');
    this.stepIndex.set(1);
    this.loadingPlaybooks.set(true);

    this.playbooksApi.list().pipe(
      catchError(() => of([] as SpPlaybook[])),
    ).subscribe(playbooks => {
      this.playbooks.set(playbooks);
      this.loadingPlaybooks.set(false);
    });
  }

  selectPlaybook(pb: SpPlaybook): void {
    this.selectedPlaybook.set(pb);
  }

  /** Confirm playbook selection — clone if shared, then set as default */
  confirmPlaybook(): void {
    const pb = this.selectedPlaybook();
    const project = this.createdProject();
    if (!pb || !project) return;

    this.savingPlaybook.set(true);
    this.playbookError.set('');

    // If it's a shared default (tenant_id === null), clone it first
    if (pb.tenant_id === null) {
      this.playbooksApi.create({
        title: pb.title,
        trigger_description: pb.trigger_description,
        steps: pb.steps,
        tags: pb.tags,
        initial_state: pb.initial_state,
        metadata: pb.metadata,
      }).subscribe({
        next: (cloned) => {
          this.setPlaybookOnProject(project.id, cloned);
        },
        error: (err) => {
          this.savingPlaybook.set(false);
          const msg = err?.error?.message || err?.error || 'Failed to clone playbook';
          this.playbookError.set(typeof msg === 'string' ? msg : JSON.stringify(msg));
        },
      });
    } else {
      this.setPlaybookOnProject(project.id, pb);
    }
  }

  private setPlaybookOnProject(projectId: string, pb: SpPlaybook): void {
    this.api.updateProject(projectId, { default_playbook_id: pb.id }).subscribe({
      next: () => {
        this.savingPlaybook.set(false);
        this.configuredPlaybook.set(pb);
        this.goToAgentStep();
      },
      error: (err) => {
        this.savingPlaybook.set(false);
        const msg = err?.error?.message || err?.error || 'Failed to set playbook';
        this.playbookError.set(typeof msg === 'string' ? msg : JSON.stringify(msg));
      },
    });
  }

  skipPlaybook(): void {
    this.goToAgentStep();
  }

  /** Load agents/roles and advance to step 3 */
  private goToAgentStep(): void {
    this.step.set('agent');
    this.stepIndex.set(2);
    this.loadingAgents.set(true);

    let agentsLoaded = false;
    let rolesLoaded = false;
    const checkDone = () => {
      if (agentsLoaded && rolesLoaded) this.loadingAgents.set(false);
    };

    this.agentsApi.getAgents().pipe(
      catchError(() => of([] as SpAgent[])),
    ).subscribe(agents => {
      this.agents.set(agents);
      agentsLoaded = true;
      checkDone();
    });

    this.teamApi.getRoles().pipe(
      catchError(() => of([] as SpRole[])),
    ).subscribe(roles => {
      this.roles.set(roles);
      rolesLoaded = true;
      checkDone();
    });
  }

  /** Confirm agent assignment */
  confirmAgent(): void {
    if (!this.selectedAgentId || !this.selectedRoleId) return;

    this.savingAgent.set(true);
    this.agentError.set('');

    this.teamApi.createMember({
      agent_id: this.selectedAgentId,
      role_id: this.selectedRoleId,
    }).subscribe({
      next: () => {
        this.savingAgent.set(false);
        this.configuredAgent.set(this.agents().find(a => a.id === this.selectedAgentId) ?? null);
        this.configuredRole.set(this.roles().find(r => r.id === this.selectedRoleId) ?? null);
        this.goToDone();
      },
      error: (err) => {
        this.savingAgent.set(false);
        const msg = err?.error?.message || err?.error || 'Failed to assign agent';
        this.agentError.set(typeof msg === 'string' ? msg : JSON.stringify(msg));
      },
    });
  }

  skipAgent(): void {
    this.goToDone();
  }

  private goToDone(): void {
    this.step.set('done');
    this.stepIndex.set(3);
  }

  onDone(): void {
    const project = this.createdProject();
    if (project) this.created.emit(project);
  }
}
