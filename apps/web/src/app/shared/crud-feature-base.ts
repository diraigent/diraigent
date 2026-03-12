import { inject, signal, effect } from '@angular/core';
import { ProjectContext } from '../core/services/project-context.service';

/**
 * Base class for list-detail CRUD feature pages.
 *
 * Provides the common signal state (items, loading, selected, searchQuery, showForm, editing)
 * and registers an effect that reloads items whenever the active project changes.
 *
 * Subclasses must implement:
 * - `loadItems()` — fetches and sets `this.items`
 * - `resetForm()` — clears form fields for a create operation
 * - `fillForm(item)` — populates form fields for an edit operation
 */
export abstract class CrudFeatureBase<T extends { id: string }> {
  protected ctx = inject(ProjectContext);

  items = signal<T[]>([]);
  loading = signal(false);
  selected = signal<T | null>(null);
  searchQuery = signal('');
  showForm = signal(false);
  editing = signal<T | null>(null);

  constructor() {
    effect(() => {
      this.ctx.projectId();
      this.selected.set(null);
      this.loadItems();
    });
  }

  abstract loadItems(): void;

  protected abstract resetForm(): void;
  protected abstract fillForm(item: T): void;

  selectItem(item: T): void {
    this.selected.set(item.id === this.selected()?.id ? null : item);
  }

  openCreate(): void {
    this.editing.set(null);
    this.resetForm();
    this.showForm.set(true);
  }

  openEdit(item: T): void {
    this.editing.set(item);
    this.fillForm(item);
    this.showForm.set(true);
  }

  closeForm(): void {
    this.showForm.set(false);
    this.editing.set(null);
  }

  /** Refresh items list after a mutation, keeping the currently selected item in sync. */
  protected refreshAfterMutation(newItems: T[]): void {
    this.items.set(newItems);
    this.loading.set(false);
    if (this.selected()) {
      const still = newItems.find(i => i.id === this.selected()!.id);
      this.selected.set(still ?? null);
    }
  }
}
