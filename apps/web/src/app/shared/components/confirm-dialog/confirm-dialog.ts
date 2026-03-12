import { Component, input, output } from '@angular/core';
import { NgClass } from '@angular/common';
import { ModalWrapperComponent } from '../modal-wrapper/modal-wrapper';

/**
 * Reusable confirmation dialog.
 * Wraps ModalWrapper with a standard title / message / cancel + confirm button layout.
 *
 * Usage:
 *   @if (showConfirm()) {
 *     <app-confirm-dialog
 *       [title]="t('foo.confirmTitle')"
 *       [message]="t('foo.confirmMessage')"
 *       [cancelLabel]="t('common.cancel')"
 *       [confirmLabel]="t('foo.delete')"
 *       confirmClass="bg-ctp-red/20 text-ctp-red hover:bg-ctp-red/30"
 *       (confirmed)="onConfirmed()"
 *       (cancelled)="showConfirm.set(false)" />
 *   }
 */
@Component({
  selector: 'app-confirm-dialog',
  standalone: true,
  imports: [NgClass, ModalWrapperComponent],
  template: `
    <app-modal-wrapper maxWidth="max-w-md" (closed)="cancelled.emit()">
      <h2 class="text-lg font-semibold text-text-primary mb-2">{{ title() }}</h2>
      @if (message()) {
        <p class="text-sm text-text-secondary mb-4">{{ message() }}</p>
      }
      <div class="flex justify-end gap-3">
        <button
          (click)="cancelled.emit()"
          class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary">
          {{ cancelLabel() }}
        </button>
        <button
          (click)="confirmed.emit()"
          [ngClass]="confirmClass()"
          class="px-4 py-2 rounded-lg text-sm font-medium">
          {{ confirmLabel() }}
        </button>
      </div>
    </app-modal-wrapper>
  `,
})
export class ConfirmDialogComponent {
  title = input.required<string>();
  message = input('');
  cancelLabel = input('Cancel');
  confirmLabel = input.required<string>();
  confirmClass = input('bg-ctp-red/20 text-ctp-red hover:bg-ctp-red/30');

  confirmed = output<void>();
  cancelled = output<void>();
}
