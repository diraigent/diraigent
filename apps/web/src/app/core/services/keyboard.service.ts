import { Injectable, inject, signal, NgZone } from '@angular/core';
import { Router } from '@angular/router';
import { ChatService } from './chat.service';

/**
 * Describes a single keyboard shortcut for display in the help overlay.
 */
export interface ShortcutEntry {
  key: string;
  description: string;
  group: string;
}

/**
 * Returns true when the focused element is an input, textarea, contenteditable,
 * or any element that typically captures keyboard input.
 */
function isTyping(event: KeyboardEvent): boolean {
  const el = event.target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  if (el.isContentEditable) return true;
  return false;
}

/**
 * Global keyboard shortcut service.
 *
 * Provides:
 * - Navigation shortcuts (number keys to jump between views)
 * - Escape to close modals/overlays
 * - `?` to toggle keyboard help overlay
 * - Extensible shortcut registration for feature pages
 */
@Injectable({ providedIn: 'root' })
export class KeyboardService {
  private router = inject(Router);
  private zone = inject(NgZone);
  private chat = inject(ChatService);

  /** Whether the keyboard-help overlay is visible. */
  readonly helpOpen = signal(false);

  /** Track how many modals are currently open (incremented/decremented by ModalWrapper). */
  private modalCount = 0;

  /** Whether any modal is currently open. */
  get isModalOpen(): boolean {
    return this.modalCount > 0;
  }

  /**
   * Navigation shortcut map: key → route path.
   * Derived from navigation.json (adapted for current web routes).
   */
  private readonly navShortcuts: Record<string, string> = {
    '1': '/work',
    '2': '/review',
    '3': '/decisions',
    '4': '/',           // dashboard
    '5': '/pipelines',
    '6': '/playbooks',
    '7': '/knowledge',
    '8': '/audit',
    '9': '/settings',
    '0': '/verifications',
  };

  /** All registered shortcuts for the help overlay. */
  readonly shortcuts: ShortcutEntry[] = [
    // Navigation
    { key: '1', description: 'Work', group: 'Navigation' },
    { key: '2', description: 'Review', group: 'Navigation' },
    { key: '3', description: 'Decisions', group: 'Navigation' },
    { key: '4', description: 'Dashboard', group: 'Navigation' },
    { key: '5', description: 'Pipelines', group: 'Navigation' },
    { key: '6', description: 'Playbooks', group: 'Navigation' },
    { key: '7', description: 'Knowledge', group: 'Navigation' },
    { key: '8', description: 'Audit', group: 'Navigation' },
    { key: '9', description: 'Settings', group: 'Navigation' },
    { key: '0', description: 'Verifications', group: 'Navigation' },
    // Global
    { key: '?', description: 'Toggle this help', group: 'Global' },
    { key: 'Esc', description: 'Close modal / overlay', group: 'Global' },
  ];

  /** Call once from the root component to start listening. */
  attach(): () => void {
    const handler = (event: KeyboardEvent) => this.handleKeydown(event);
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }

  /** Called by ModalWrapper on init to track open modals. */
  registerModal(): void {
    this.modalCount++;
  }

  /** Called by ModalWrapper on destroy to track open modals. */
  unregisterModal(): void {
    this.modalCount = Math.max(0, this.modalCount - 1);
  }

  private handleKeydown(event: KeyboardEvent): void {
    // Never intercept when modifier keys are held (allow browser shortcuts)
    if (event.ctrlKey || event.metaKey || event.altKey) return;

    const key = event.key;

    // ── Escape: close help overlay if open ──────────────────────────────
    if (key === 'Escape') {
      if (this.helpOpen()) {
        this.zone.run(() => this.helpOpen.set(false));
        event.preventDefault();
        return;
      }
      // Modal escape is handled by ModalWrapper's own @HostListener
      // Sidebar mobile close is handled at the sidebar component level
      return;
    }

    // Don't process shortcuts while typing in inputs
    if (isTyping(event)) return;

    // Don't process shortcuts while a modal is open (except Escape above)
    if (this.isModalOpen) return;

    // ── Help: ? ─────────────────────────────────────────────────────────
    if (key === '?') {
      this.zone.run(() => this.helpOpen.update(v => !v));
      event.preventDefault();
      return;
    }

    // ── Navigation: number keys ─────────────────────────────────────────
    const route = this.navShortcuts[key];
    if (route) {
      this.zone.run(() => this.router.navigateByUrl(route));
      event.preventDefault();
      return;
    }
  }
}
