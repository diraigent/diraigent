import { Component, inject, computed } from '@angular/core';
import { DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TranslocoModule } from '@jsverse/transloco';
import { KnowledgeApiService, SpKnowledge, KnowledgeCategory, SpKnowledgeCreate } from '../../core/services/knowledge-api.service';
import { KNOWLEDGE_CATEGORY_COLORS, KNOWLEDGE_CATEGORY_FALLBACK_COLOR } from '../../shared/ui-constants';
import { ProjectPackageService } from '../../core/services/project-package.service';
import { CrudFeatureBase } from '../../shared/crud-feature-base';
import { ModalWrapperComponent } from '../../shared/components/modal-wrapper/modal-wrapper';
import { FilterBarComponent } from '../../shared/components/filter-bar/filter-bar';
@Component({
  selector: 'app-knowledge',
  standalone: true,
  imports: [TranslocoModule, FormsModule, DatePipe, ModalWrapperComponent, FilterBarComponent],
  template: `
    <div class="p-3 sm:p-6" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <h1 class="text-2xl font-semibold text-text-primary">{{ t('nav.knowledge') }}</h1>
        <button (click)="openCreate()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
          {{ t('knowledge.create') }}
        </button>
      </div>

      <!-- Filters -->
      <app-filter-bar
        [placeholder]="t('knowledge.searchPlaceholder')"
        [query]="searchQuery()"
        (queryChange)="searchQuery.set($event)">
        <select
          [(ngModel)]="selectedCategory"
          (ngModelChange)="loadItems()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('knowledge.allCategories') }}</option>
          @for (cat of categories(); track cat) {
            <option [value]="cat">{{ t('knowledge.category.' + cat) }}</option>
          }
        </select>
        <select
          [(ngModel)]="selectedTag"
          (ngModelChange)="loadItems()"
          class="bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent">
          <option value="">{{ t('knowledge.allTags') }}</option>
          @for (tag of allTags(); track tag) {
            <option [value]="tag">{{ tag }}</option>
          }
        </select>
      </app-filter-bar>

      <!-- Content: accordion list -->
      @if (loading()) {
        <p class="text-text-secondary text-sm">{{ t('common.loading') }}</p>
      } @else if (filtered().length === 0) {
        <p class="text-text-secondary text-sm">{{ t('common.empty') }}</p>
      } @else {
        <div class="space-y-2">
          @for (item of filtered(); track item.id) {
            <div class="rounded-lg border transition-colors"
              [class]="item.id === selected()?.id
                ? 'bg-accent/10 border-accent'
                : 'bg-surface border-border hover:border-accent/50'">
              <!-- Accordion header -->
              <button (click)="selectItem(item)" class="w-full text-left p-4">
                <div class="flex items-center gap-2">
                  <svg class="w-4 h-4 text-text-secondary shrink-0 transition-transform duration-200"
                    [class.rotate-90]="item.id === selected()?.id"
                    fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
                  </svg>
                  <span class="text-sm font-medium text-text-primary">{{ item.title }}</span>
                  <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ categoryColor(item.category) }}">
                    {{ t('knowledge.category.' + item.category) }}
                  </span>
                </div>
                @if (item.id !== selected()?.id && item.tags.length > 0) {
                  <div class="flex flex-wrap gap-1 mt-1 ml-6">
                    @for (tag of item.tags; track tag) {
                      <span class="px-1.5 py-0.5 bg-surface-hover text-text-secondary rounded text-xs">{{ tag }}</span>
                    }
                  </div>
                }
              </button>

              <!-- Expanded detail (inline) -->
              @if (item.id === selected()?.id) {
                <div class="px-4 pb-4 pt-0 border-t border-border/50 mt-0">
                  <!-- Actions -->
                  <div class="flex items-center gap-2 pt-3 mb-3">
                    <span class="px-2 py-0.5 rounded-full text-xs font-medium {{ categoryColor(item.category) }}">
                      {{ t('knowledge.category.' + item.category) }}
                    </span>
                    @if (item.tags.length > 0) {
                      @for (tag of item.tags; track tag) {
                        <span class="px-2 py-0.5 bg-surface-hover text-text-secondary rounded text-xs">{{ tag }}</span>
                      }
                    }
                    <div class="flex gap-2 ml-auto">
                      <button (click)="openEdit(item)" class="p-1.5 text-text-secondary hover:text-accent rounded" title="Edit">
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                          <path d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                        </svg>
                      </button>
                      <button (click)="deleteItem(item)" class="p-1.5 text-text-secondary hover:text-ctp-red rounded" title="Delete">
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                          <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      </button>
                    </div>
                  </div>

                  <div class="text-sm text-text-primary whitespace-pre-wrap leading-relaxed border-t border-border pt-4">{{ item.content }}</div>
                  <div class="mt-4 pt-3 border-t border-border text-xs text-text-secondary">
                    {{ t('knowledge.updatedAt') }}: {{ item.updated_at | date:'medium' }}
                  </div>
                </div>
              }
            </div>
          }
        </div>
      }

      <!-- Create/Edit modal -->
      @if (showForm()) {
        <app-modal-wrapper maxWidth="max-w-lg" [scrollable]="true" (closed)="closeForm()">
          <h2 class="text-lg font-semibold text-text-primary mb-4">
            {{ editing() ? t('knowledge.editTitle') : t('knowledge.createTitle') }}
          </h2>
          <div class="space-y-4">
            <div>
              <label for="know-title" class="block text-sm text-text-secondary mb-1">{{ t('knowledge.fieldTitle') }}</label>
              <input id="know-title" type="text" [(ngModel)]="formTitle"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent" />
            </div>
            <div>
              <label for="know-category" class="block text-sm text-text-secondary mb-1">{{ t('knowledge.fieldCategory') }}</label>
              <select id="know-category" [(ngModel)]="formCategory"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent">
                @for (cat of categories(); track cat) {
                  <option [value]="cat">{{ t('knowledge.category.' + cat) }}</option>
                }
              </select>
            </div>
            <div>
              <label for="know-content" class="block text-sm text-text-secondary mb-1">{{ t('knowledge.fieldContent') }}</label>
              <textarea id="know-content" [(ngModel)]="formContent" rows="8"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent resize-y"></textarea>
            </div>
            <div>
              <label for="know-tags" class="block text-sm text-text-secondary mb-1">{{ t('knowledge.fieldTags') }}</label>
              <input id="know-tags" type="text" [(ngModel)]="formTagsInput" [placeholder]="t('knowledge.tagsPlaceholder')"
                class="w-full bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                       focus:outline-none focus:ring-1 focus:ring-accent" />
            </div>
            <div class="flex justify-end gap-3 pt-2">
              <button (click)="closeForm()" class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
                {{ t('knowledge.cancel') }}
              </button>
              <button (click)="submitForm()" class="px-4 py-2 bg-accent text-bg rounded-lg text-sm font-medium hover:opacity-90">
                {{ editing() ? t('knowledge.save') : t('knowledge.create') }}
              </button>
            </div>
          </div>
        </app-modal-wrapper>
      }
    </div>
  `,
})
export class KnowledgePage extends CrudFeatureBase<SpKnowledge> {
  private api = inject(KnowledgeApiService);
  private packageCtx = inject(ProjectPackageService);

  /** Categories loaded from the project's package, falling back to software-dev defaults. */
  readonly categories = this.packageCtx.knowledgeCategories;

  selectedCategory = '';
  selectedTag = '';

  formTitle = '';
  formCategory: KnowledgeCategory = 'general';
  formContent = '';
  formTagsInput = '';

  allTags = computed(() => {
    const tags = new Set<string>();
    for (const item of this.items()) {
      for (const tag of item.tags) tags.add(tag);
    }
    return [...tags].sort();
  });

  filtered = computed(() => {
    const q = this.searchQuery().toLowerCase().trim();
    if (!q) return this.items();
    return this.items().filter(
      item => item.title.toLowerCase().includes(q) || item.content.toLowerCase().includes(q),
    );
  });

  override loadItems(): void {
    this.loading.set(true);
    const cat = this.selectedCategory as KnowledgeCategory | '';
    this.api.list(cat || undefined, this.selectedTag || undefined).subscribe({
      next: (items) => this.refreshAfterMutation(items),
      error: () => this.loading.set(false),
    });
  }

  protected override resetForm(): void {
    this.formTitle = '';
    const cats = this.categories();
    this.formCategory = (cats.includes('general') ? 'general' : cats[0] ?? 'general') as KnowledgeCategory;
    this.formContent = '';
    this.formTagsInput = '';
  }

  protected override fillForm(item: SpKnowledge): void {
    this.formTitle = item.title;
    this.formCategory = item.category;
    this.formContent = item.content;
    this.formTagsInput = item.tags.join(', ');
  }

  categoryColor(cat: string): string {
    return KNOWLEDGE_CATEGORY_COLORS[cat] ?? KNOWLEDGE_CATEGORY_FALLBACK_COLOR;
  }

  deleteItem(item: SpKnowledge): void {
    this.api.delete(item.id).subscribe({
      next: () => {
        this.selected.set(null);
        this.loadItems();
      },
    });
  }

  submitForm(): void {
    const tags = this.formTagsInput
      .split(',')
      .map(t => t.trim())
      .filter(t => t.length > 0);

    const existing = this.editing();
    if (existing) {
      this.api.update(existing.id, {
        title: this.formTitle,
        category: this.formCategory,
        content: this.formContent,
        tags,
      }).subscribe({
        next: () => {
          this.closeForm();
          this.loadItems();
        },
      });
    } else {
      const data: SpKnowledgeCreate = {
        title: this.formTitle,
        category: this.formCategory,
        content: this.formContent,
        tags,
      };
      this.api.create(data).subscribe({
        next: () => {
          this.closeForm();
          this.loadItems();
        },
      });
    }
  }
}
