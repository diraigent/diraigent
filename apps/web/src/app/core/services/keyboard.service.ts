import { Injectable, inject, signal, NgZone } from '@angular/core';
import { Router } from '@angular/router';
import { Subject } from 'rxjs';
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
 * Payload emitted when a view-specific or global action shortcut fires.
 * Feature components subscribe to `KeyboardService.action$` and filter by their view/action id.
 */
export interface ActionEvent {
  /** The view id derived from the current route (e.g. 'work', 'review', 'decisions'). */
  viewId: string;
  /** The action id from navigation.json (e.g. 'create', 'edit', 'comment'). */
  actionId: string;
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
 * Action definition from navigation.json, keyed by shortcut key.
 */
interface ActionDef {
  id: string;
  label: string;
  key: string;
}

/**
 * View definition from navigation.json with its actions.
 */
interface ViewDef {
  id: string;
  path: string;
  actions: ActionDef[];
}

/**
 * Global keyboard shortcut service.
 *
 * Provides:
 * - Navigation shortcuts (number keys + letter keys to jump between views)
 * - View-specific action shortcuts (dispatched via action$ Subject)
 * - Escape to close modals/overlays
 * - `?` to toggle keyboard help overlay
 * - Chat focus with `C`
 */
@Injectable({ providedIn: 'root' })
export class KeyboardService {
  private router = inject(Router);
  private zone = inject(NgZone);
  private chat = inject(ChatService);

  /** Whether the keyboard-help overlay is visible. */
  readonly helpOpen = signal(false);

  /** Emits when a view-specific action shortcut is pressed. */
  readonly action$ = new Subject<ActionEvent>();

  /** Emits when Escape is pressed while a modal is open. Inline modals can subscribe to this. */
  readonly escape$ = new Subject<void>();

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
    // Number keys (primary views)
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
    // Letter keys (additional views from navigation.json)
    'R': '/reports',
    'B': '/source',
    'A': '/audit',
    'I': '/integrations',
    'S': '/settings',
    'l': '/integrations/logs',
    'E': '/audit',       // events → audit (closest equivalent)
  };

  /**
   * Special navigation keys that trigger non-route actions.
   */
  private readonly specialNavKeys: Record<string, () => void> = {
    'C': () => this.chat.openWithMessage(),
  };

  /**
   * View definitions with their action shortcuts.
   * Derived from navigation.json, adapted for web routes.
   */
  private readonly viewDefs: ViewDef[] = [
    {
      id: 'work', path: '/work',
      actions: [
        { id: 'create', label: 'New', key: 'n' },
        { id: 'edit', label: 'Edit', key: 'e' },
        { id: 'comment', label: 'Comment', key: 'c' },
        { id: 'status', label: 'Status', key: 's' },
      ],
    },
    {
      id: 'review', path: '/review',
      actions: [
        { id: 'create', label: 'New', key: 'n' },
        { id: 'edit', label: 'Edit', key: 'e' },
        { id: 'comment', label: 'Comment', key: 'c' },
        { id: 'reply', label: 'Reply', key: 'r' },
        { id: 'transition', label: 'Transition', key: 't' },
        { id: 'claim', label: 'Claim/Assign', key: 'a' },
        { id: 'flag', label: 'Flag', key: 'f' },
        { id: 'subtask', label: 'Subtask', key: 'd' },
        { id: 'link_work', label: 'Link to Work', key: 'g' },
        { id: 'inspect', label: 'Inspect', key: 'i' },
        { id: 'hierarchy', label: 'Hierarchy', key: 'h' },
        { id: 'bulk_select', label: 'Select', key: 'v' },
        { id: 'sort', label: 'Sort/Filter', key: 's' },
      ],
    },
    {
      id: 'decisions', path: '/decisions',
      actions: [
        { id: 'create', label: 'New', key: 'n' },
        { id: 'accept', label: 'Accept', key: 'a' },
        { id: 'reject', label: 'Reject', key: 'x' },
        { id: 'deprecate', label: 'Deprecate', key: 'X' },
        { id: 'supersede', label: 'Supersede', key: 'S' },
        { id: 'delete', label: 'Delete', key: 'D' },
      ],
    },
    {
      id: 'pipelines', path: '/pipelines',
      actions: [
        { id: 'task_queue', label: 'Task Queue', key: 'a' },
      ],
    },
    {
      id: 'playbooks', path: '/playbooks',
      actions: [
        { id: 'create', label: 'New', key: 'n' },
        { id: 'edit', label: 'Edit', key: 'e' },
        { id: 'delete', label: 'Delete', key: 'D' },
        { id: 'templates', label: 'Step Templates', key: 'T' },
      ],
    },
    {
      id: 'knowledge', path: '/knowledge',
      actions: [
        { id: 'create', label: 'New', key: 'n' },
        { id: 'edit', label: 'Edit', key: 'e' },
        { id: 'delete', label: 'Delete', key: 'D' },
      ],
    },
    {
      id: 'verifications', path: '/verifications',
      actions: [
        { id: 'create', label: 'New', key: 'n' },
        { id: 'status', label: 'Status', key: 's' },
        { id: 'kind_filter', label: 'Kind Filter', key: 'K' },
        { id: 'status_filter', label: 'Status Filter', key: 'S' },
      ],
    },
    {
      id: 'reports', path: '/reports',
      actions: [
        { id: 'create', label: 'New', key: 'n' },
        { id: 'delete', label: 'Delete', key: 'D' },
      ],
    },
    {
      id: 'source', path: '/source',
      actions: [
        { id: 'line_jump', label: 'Go to Line', key: 'L' },
      ],
    },
    {
      id: 'audit', path: '/audit',
      actions: [
        { id: 'filter', label: 'Filter', key: 'h' },
      ],
    },
    {
      id: 'integrations', path: '/integrations',
      actions: [
        { id: 'create', label: 'New', key: 'n' },
        { id: 'edit', label: 'Edit', key: 'e' },
        { id: 'access', label: 'Access', key: 'a' },
        { id: 'delete', label: 'Delete', key: 'D' },
      ],
    },
    {
      id: 'settings', path: '/settings',
      actions: [],
    },
  ];

  /** All registered shortcuts for the help overlay. */
  readonly shortcuts: ShortcutEntry[] = [
    // Navigation – number keys
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
    // Navigation – letter keys
    { key: 'R', description: 'Reports', group: 'Navigation' },
    { key: 'B', description: 'Source', group: 'Navigation' },
    { key: 'C', description: 'Chat', group: 'Navigation' },
    { key: 'I', description: 'Integrations', group: 'Navigation' },
    { key: 'S', description: 'Settings', group: 'Navigation' },
    { key: 'l', description: 'Logs', group: 'Navigation' },
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

  /**
   * Get action shortcuts for the current view (for displaying in help overlay).
   */
  getActionsForCurrentView(): ShortcutEntry[] {
    const view = this.resolveCurrentView();
    if (!view) return [];
    return view.actions.map(a => ({
      key: a.key,
      description: a.label,
      group: `Actions (${view.id})`,
    }));
  }

  private handleKeydown(event: KeyboardEvent): void {
    // Never intercept when modifier keys are held (allow browser shortcuts)
    if (event.ctrlKey || event.metaKey || event.altKey) return;

    const key = event.key;

    // ── Escape ────────────────────────────────────────────────────────────
    if (key === 'Escape') {
      if (this.helpOpen()) {
        this.zone.run(() => this.helpOpen.set(false));
        event.preventDefault();
        return;
      }
      // Always emit escape$ so any open overlay/modal can close,
      // regardless of whether it registered with registerModal().
      this.zone.run(() => this.escape$.next());
      event.preventDefault();
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

    // ── View-specific action shortcuts ──────────────────────────────────
    const view = this.resolveCurrentView();
    if (view) {
      const action = view.actions.find(a => a.key === key);
      if (action) {
        this.zone.run(() => this.action$.next({ viewId: view.id, actionId: action.id }));
        event.preventDefault();
        return;
      }
    }

    // ── Special navigation keys (e.g. Chat) ─────────────────────────────
    const specialHandler = this.specialNavKeys[key];
    if (specialHandler) {
      this.zone.run(() => specialHandler());
      event.preventDefault();
      return;
    }

    // ── Navigation: number + letter keys ─────────────────────────────────
    const route = this.navShortcuts[key];
    if (route) {
      this.zone.run(() => this.router.navigateByUrl(route));
      event.preventDefault();
      return;
    }
  }

  /**
   * Resolves the current view definition based on the active router URL.
   */
  private resolveCurrentView(): ViewDef | undefined {
    const url = this.router.url.split('?')[0]; // strip query params
    // Find the best matching view (longest path prefix match).
    // Skip root path '/' to avoid matching everything.
    return this.viewDefs
      .filter(v => v.path !== '/' && (url === v.path || url.startsWith(v.path + '/')))
      .sort((a, b) => b.path.length - a.path.length)[0];
  }
}
