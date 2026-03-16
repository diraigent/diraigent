/**
 * Centralized UI color constants for status badges and labels.
 * All Catppuccin color class mappings live here to avoid duplication across feature components.
 */

import { DecisionStatus } from '../core/services/decisions-api.service';
import { ObservationKind, ObservationSeverity } from '../core/services/observations-api.service';
import { IntegrationKind } from '../core/services/integrations-api.service';
import type { WorkStatus, WorkType } from '../core/services/work-api.service';
import type { VerificationStatus, VerificationKind } from '../core/services/verifications-api.service';
import type { ReportStatus, ReportKind } from '../core/services/reports-api.service';

// ── Decisions ────────────────────────────────────────────────────────────────

export const DECISION_STATUS_COLORS: Record<DecisionStatus, string> = {
  proposed: 'bg-ctp-blue/20 text-ctp-blue',
  accepted: 'bg-ctp-green/20 text-ctp-green',
  rejected: 'bg-ctp-red/20 text-ctp-red',
  superseded: 'bg-ctp-yellow/20 text-ctp-yellow',
  deprecated: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

// ── Observations ─────────────────────────────────────────────────────────────

export const OBSERVATION_SEVERITY_COLORS: Record<ObservationSeverity, string> = {
  critical: 'bg-ctp-red/20 text-ctp-red',
  high: 'bg-ctp-peach/20 text-ctp-peach',
  medium: 'bg-ctp-yellow/20 text-ctp-yellow',
  low: 'bg-ctp-blue/20 text-ctp-blue',
  info: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

export const OBSERVATION_KIND_COLORS: Record<ObservationKind, string> = {
  insight: 'bg-ctp-blue/20 text-ctp-blue',
  risk: 'bg-ctp-red/20 text-ctp-red',
  opportunity: 'bg-ctp-green/20 text-ctp-green',
  smell: 'bg-ctp-yellow/20 text-ctp-yellow',
  inconsistency: 'bg-ctp-peach/20 text-ctp-peach',
  improvement: 'bg-ctp-teal/20 text-ctp-teal',
};

// ── Knowledge ─────────────────────────────────────────────────────────────────

/**
 * Color map for known knowledge categories (software-dev defaults).
 * Uses `Record<string, string>` to allow unknown package-defined categories
 * to gracefully fall back via the `??` operator in consumers.
 */
export const KNOWLEDGE_CATEGORY_COLORS: Record<string, string> = {
  architecture: 'bg-ctp-blue/20 text-ctp-blue',
  convention: 'bg-ctp-teal/20 text-ctp-teal',
  pattern: 'bg-ctp-green/20 text-ctp-green',
  anti_pattern: 'bg-ctp-red/20 text-ctp-red',
  setup: 'bg-ctp-yellow/20 text-ctp-yellow',
  general: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

/** Fallback color for knowledge categories not present in KNOWLEDGE_CATEGORY_COLORS. */
export const KNOWLEDGE_CATEGORY_FALLBACK_COLOR = 'bg-ctp-surface1/20 text-ctp-subtext0';

// ── Integrations ──────────────────────────────────────────────────────────────

export const INTEGRATION_KIND_COLORS: Record<IntegrationKind, string> = {
  logging: 'bg-ctp-yellow/20 text-ctp-yellow',
  tracing: 'bg-ctp-mauve/20 text-ctp-mauve',
  metrics: 'bg-ctp-blue/20 text-ctp-blue',
  git: 'bg-ctp-peach/20 text-ctp-peach',
  ci: 'bg-ctp-green/20 text-ctp-green',
  messaging: 'bg-ctp-pink/20 text-ctp-pink',
  monitoring: 'bg-ctp-teal/20 text-ctp-teal',
  storage: 'bg-ctp-lavender/20 text-ctp-lavender',
  database: 'bg-ctp-red/20 text-ctp-red',
  custom: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

// ── CI Pipelines ─────────────────────────────────────────────────────────────

export const CI_STATUS_COLORS: Record<string, string> = {
  success: 'bg-ctp-green/20 text-ctp-green',
  failure: 'bg-ctp-red/20 text-ctp-red',
  running: 'bg-ctp-yellow/20 text-ctp-yellow',
  pending: 'bg-ctp-blue/20 text-ctp-blue',
  skipped: 'bg-ctp-overlay0/20 text-ctp-overlay0',
  cancelled: 'bg-ctp-peach/20 text-ctp-peach',
};

// ── Tasks ─────────────────────────────────────────────────────────────────────

export const TASK_STATE_COLORS: Record<string, string> = {
  backlog: 'bg-ctp-overlay0/20 text-ctp-overlay0 latte:text-ctp-subtext1',
  ready: 'bg-ctp-blue/20 text-ctp-blue latte:text-ctp-blue-900',
  working: 'bg-ctp-yellow/20 text-ctp-yellow latte:text-ctp-yellow-950',
  implement: 'bg-ctp-yellow/20 text-ctp-yellow latte:text-ctp-yellow-950',
  review: 'bg-ctp-mauve/20 text-ctp-mauve latte:text-ctp-mauve-900',
  merge: 'bg-ctp-teal/20 text-ctp-teal latte:text-ctp-teal-950',
  dream: 'bg-ctp-pink/20 text-ctp-pink latte:text-ctp-pink-900',
  human_review: 'bg-ctp-peach/20 text-ctp-peach latte:text-ctp-peach-950',
  done: 'bg-ctp-green/20 text-ctp-green latte:text-ctp-green-950',
  cancelled: 'bg-ctp-red/20 text-ctp-red latte:text-ctp-red-900',
  // Research playbook steps
  scope: 'bg-ctp-lavender/20 text-ctp-lavender latte:text-ctp-lavender-900',
  gather: 'bg-ctp-sapphire/20 text-ctp-sapphire latte:text-ctp-sapphire-900',
  synthesize: 'bg-ctp-mauve/20 text-ctp-mauve latte:text-ctp-mauve-900',
  document: 'bg-ctp-teal/20 text-ctp-teal latte:text-ctp-teal-950',
  // wait:* states use a consistent style; matched by prefix in taskStateColor()
  wait: 'bg-ctp-sapphire/20 text-ctp-sapphire latte:text-ctp-sapphire-900',
};

export const TASK_VALID_TRANSITIONS: Record<string, string[]> = {
  backlog: ['ready', 'cancelled'],
  ready: ['backlog', 'cancelled'],
  working: ['ready', 'done', 'cancelled'],
  implement: ['ready', 'done', 'cancelled'],
  review: ['ready', 'done', 'cancelled'],
  merge: ['ready', 'done', 'cancelled'],
  dream: ['ready', 'done', 'cancelled'],
  human_review: ['ready', 'done', 'cancelled'],
  done: ['backlog'],
  cancelled: ['backlog'],
};

/** Fallback task kinds matching the API's TASK_KINDS. */
export const DEFAULT_TASK_KINDS: string[] = [
  'feature', 'bug', 'refactor', 'docs', 'test', 'research', 'chore', 'spike',
];

// ── Helper functions ────────────────────────────────────────────────────────

/** Returns the Tailwind badge classes for a task state. */
export function taskStateColor(state: string): string {
  return TASK_STATE_COLORS[state]
    ?? (state.startsWith('wait:') ? TASK_STATE_COLORS['wait'] : undefined)
    // Default to blue for unknown active states (custom playbook steps)
    ?? 'bg-ctp-blue/20 text-ctp-blue latte:text-ctp-blue-900';
}

/** Returns the list of valid target states for a task in the given state. */
export function taskTransitions(state: string): string[] {
  if (state.startsWith('wait:')) return ['cancelled'];
  // For unknown step states (custom playbook steps), allow the same transitions as implement
  return TASK_VALID_TRANSITIONS[state] ?? ['ready', 'done', 'cancelled'];
}

// ── Work Items ──────────────────────────────────────────────────────────────

export const WORK_STATUS_COLORS: Record<WorkStatus, string> = {
  active: 'bg-ctp-green/20 text-ctp-green',
  ready: 'bg-ctp-sapphire/20 text-ctp-sapphire',
  processing: 'bg-ctp-peach/20 text-ctp-peach',
  achieved: 'bg-ctp-blue/20 text-ctp-blue',
  paused: 'bg-ctp-yellow/20 text-ctp-yellow',
  abandoned: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

export const WORK_PROGRESS_COLORS: Record<WorkStatus, string> = {
  active: 'bg-ctp-green',
  ready: 'bg-ctp-sapphire',
  processing: 'bg-ctp-peach',
  achieved: 'bg-ctp-blue',
  paused: 'bg-ctp-yellow',
  abandoned: 'bg-ctp-overlay0',
};

export const WORK_TYPE_COLORS: Record<WorkType, string> = {
  epic: 'bg-ctp-mauve/20 text-ctp-mauve',
  feature: 'bg-ctp-blue/20 text-ctp-blue',
  milestone: 'bg-ctp-green/20 text-ctp-green',
  sprint: 'bg-ctp-peach/20 text-ctp-peach',
  initiative: 'bg-ctp-teal/20 text-ctp-teal',
};

// ── Verifications ───────────────────────────────────────────────────────────

export const VERIFICATION_STATUS_COLORS: Record<VerificationStatus, string> = {
  pass: 'bg-ctp-green/20 text-ctp-green',
  fail: 'bg-ctp-red/20 text-ctp-red',
  pending: 'bg-ctp-yellow/20 text-ctp-yellow',
  skipped: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

export const VERIFICATION_KIND_COLORS: Record<VerificationKind, string> = {
  test: 'bg-ctp-blue/20 text-ctp-blue',
  acceptance: 'bg-ctp-teal/20 text-ctp-teal',
  sign_off: 'bg-ctp-mauve/20 text-ctp-mauve',
};

// ── Reports ─────────────────────────────────────────────────────────────────

export const REPORT_STATUS_COLORS: Record<ReportStatus, string> = {
  pending: 'bg-ctp-yellow/20 text-ctp-yellow',
  in_progress: 'bg-ctp-blue/20 text-ctp-blue',
  completed: 'bg-ctp-green/20 text-ctp-green',
  failed: 'bg-ctp-red/20 text-ctp-red',
};

export const REPORT_KIND_COLORS: Record<ReportKind, string> = {
  security: 'bg-ctp-red/20 text-ctp-red',
  component: 'bg-ctp-blue/20 text-ctp-blue',
  architecture: 'bg-ctp-teal/20 text-ctp-teal',
  performance: 'bg-ctp-peach/20 text-ctp-peach',
  custom: 'bg-ctp-overlay0/20 text-ctp-overlay0',
};

// ── Audit ───────────────────────────────────────────────────────────────────

export const AUDIT_ENTITY_TYPE_COLORS: Record<string, string> = {
  task: 'bg-ctp-blue/20 text-ctp-blue',
  agent: 'bg-ctp-teal/20 text-ctp-teal',
  knowledge: 'bg-ctp-green/20 text-ctp-green',
  decision: 'bg-ctp-yellow/20 text-ctp-yellow',
  observation: 'bg-ctp-peach/20 text-ctp-peach',
  role: 'bg-ctp-red/20 text-ctp-red',
  membership: 'bg-ctp-pink/20 text-ctp-pink',
  integration: 'bg-ctp-mauve/20 text-ctp-mauve',
  playbook: 'bg-ctp-lavender/20 text-ctp-lavender',
  work: 'bg-ctp-green/20 text-ctp-green',
};

export const AUDIT_ACTION_COLORS: Record<string, string> = {
  created: 'text-ctp-green',
  updated: 'text-ctp-yellow',
  deleted: 'text-ctp-red',
};

// ── Storage Keys ────────────────────────────────────────────────────────────

export const STORAGE_KEYS = {
  PROJECT: 'diraigent-project',
  CHAT_PREFIX: 'diraigent-chat-',
  CHAT_MODEL: 'diraigent-chat-model',
  CHAT_COLLAPSED: 'diraigent-chat-collapsed',
  CHAT_FULLSCREEN: 'diraigent-chat-fullscreen',
  THEME: 'zivue-theme',
  ACCENT: 'zivue-accent',
} as const;

// ── Helper functions ────────────────────────────────────────────────────────

/**
 * Derives the full list of filter states from the loaded playbooks.
 * Combines lifecycle states with all unique step names from playbooks,
 * ordered logically: lifecycle first, then step states.
 */
export function deriveStatesFromPlaybooks(playbooks: { steps: { name: string }[] }[]): string[] {
  const stepNames = new Set<string>();
  for (const pb of playbooks) {
    for (const step of pb.steps) {
      stepNames.add(step.name);
    }
  }
  // Add 'working' as a fallback for tasks without a playbook
  stepNames.add('working');

  // Build ordered list: backlog, ready, then step states, then terminal states
  const ordered: string[] = ['backlog', 'ready'];
  // Add step states in a deterministic order
  const sortedSteps = [...stepNames].sort();
  for (const s of sortedSteps) {
    if (!ordered.includes(s)) {
      ordered.push(s);
    }
  }
  // Append terminal/lifecycle states at the end
  for (const s of ['human_review', 'done', 'cancelled']) {
    if (!ordered.includes(s)) {
      ordered.push(s);
    }
  }
  return ordered;
}
