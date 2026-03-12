import { Component, HostListener, input, output } from '@angular/core';
import { NgClass } from '@angular/common';

/**
 * Reusable modal backdrop wrapper.
 * Renders a fixed overlay with a centered card. Emits `closed` when the backdrop is clicked or Escape is pressed.
 *
 * Usage:
 *   <app-modal-wrapper (closed)="showModal.set(false)" maxWidth="max-w-2xl" [scrollable]="true">
 *     ...content...
 *   </app-modal-wrapper>
 */
@Component({
  selector: 'app-modal-wrapper',
  standalone: true,
  imports: [NgClass],
  template: `
    <div
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-[70]"
      role="dialog"
      aria-modal="true"
      (click)="closed.emit()"
      (keydown.enter)="closed.emit()">
      <div
        class="bg-bg border border-border rounded-xl p-6 w-full"
        [ngClass]="[maxWidth(), scrollable() ? 'max-h-[90vh] overflow-y-auto' : '']"
        role="document"
        tabindex="-1"
        (click)="$event.stopPropagation()"
        (keydown)="$event.stopPropagation()">
        <ng-content />
      </div>
    </div>
  `,
})
export class ModalWrapperComponent {
  maxWidth = input('max-w-lg');
  scrollable = input(false);
  closed = output<void>();

  @HostListener('document:keydown.escape')
  onEscape(): void {
    this.closed.emit();
  }
}
