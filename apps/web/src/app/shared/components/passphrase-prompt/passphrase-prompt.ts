import { Component, output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ModalWrapperComponent } from '../modal-wrapper/modal-wrapper';

@Component({
  selector: 'app-passphrase-prompt',
  standalone: true,
  imports: [FormsModule, ModalWrapperComponent],
  template: `
    <app-modal-wrapper (closed)="dismiss.emit()" maxWidth="max-w-sm">
      <h2 class="text-lg font-semibold text-text-primary mb-2">Enter Passphrase</h2>
      <p class="text-sm text-text-secondary mb-4">
        Your tenant uses passphrase-based encryption. Enter your passphrase to unlock.
      </p>

      <label class="block mb-4">
        <span class="block text-sm font-medium text-text-secondary mb-1">Passphrase</span>
        <input
          type="password"
          [(ngModel)]="passphrase"
          (keydown.enter)="submit()"
          placeholder="Enter your passphrase"
          class="w-full bg-bg-subtle text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                 focus:outline-none focus:ring-1 focus:ring-accent"
        />
      </label>

      @if (error()) {
        <p class="text-sm text-ctp-red mb-3">{{ error() }}</p>
      }

      <div class="flex justify-end gap-2">
        <button
          (click)="dismiss.emit()"
          class="px-4 py-2 text-sm text-text-secondary rounded-lg border border-border hover:border-accent">
          Cancel
        </button>
        <button
          (click)="submit()"
          [disabled]="!passphrase.trim()"
          class="px-4 py-2 text-sm bg-accent text-bg rounded-lg font-medium hover:opacity-90 disabled:opacity-50">
          Unlock
        </button>
      </div>
    </app-modal-wrapper>
  `,
})
export class PassphrasePromptComponent {
  passphrase = '';
  error = signal('');

  /** Emits the entered passphrase when submitted. */
  confirmed = output<string>();
  /** Emits when the user dismisses without entering a passphrase. */
  dismiss = output<void>();

  submit(): void {
    const value = this.passphrase.trim();
    if (!value) return;
    this.error.set('');
    this.confirmed.emit(value);
  }
}
