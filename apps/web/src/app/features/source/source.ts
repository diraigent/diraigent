import {
  Component,
  ElementRef,
  QueryList,
  ViewChildren,
  inject,
  signal,
  computed,
} from '@angular/core';
import { HttpClient, HttpParams } from '@angular/common/http';
import { FormsModule } from '@angular/forms';
import { environment } from '../../../environments/environment';
import hljs from 'highlight.js/lib/common';
import { marked } from 'marked';
import DOMPurify from 'dompurify';

/** A node in the file tree returned by the API. */
export interface TreeNode {
  name: string;
  path: string;
  kind: 'file' | 'dir';
  children?: TreeNode[];
}

/** A flat entry as returned from the backend tree endpoint. */
interface TreeEntry {
  name: string;
  path: string;
  kind: 'file' | 'dir';
}

/** A single node displayed in the tree view, with expanded state. */
interface DisplayNode {
  entry: TreeEntry;
  depth: number;
  expanded: boolean;
  hasChildren: boolean;
}

/** Summary task data for branch label lookups. */
interface TaskSummary {
  id: string;
  title: string;
  number: number;
}

/** Map file extensions to highlight.js language identifiers. */
const EXT_TO_LANG: Record<string, string> = {
  ts: 'typescript',
  tsx: 'typescript',
  mts: 'typescript',
  js: 'javascript',
  jsx: 'javascript',
  mjs: 'javascript',
  rs: 'rust',
  py: 'python',
  pyw: 'python',
  json: 'json',
  json5: 'json',
  yaml: 'yaml',
  yml: 'yaml',
  toml: 'ini',
  html: 'xml',
  htm: 'xml',
  xml: 'xml',
  svg: 'xml',
  css: 'css',
  scss: 'scss',
  less: 'less',
  sh: 'bash',
  bash: 'bash',
  zsh: 'bash',
  fish: 'bash',
  sql: 'sql',
  kt: 'kotlin',
  kts: 'kotlin',
  java: 'java',
  go: 'go',
  c: 'c',
  h: 'c',
  cpp: 'cpp',
  cc: 'cpp',
  cxx: 'cpp',
  hpp: 'cpp',
  cs: 'csharp',
  rb: 'ruby',
  php: 'php',
  swift: 'swift',
  dart: 'dart',
  r: 'r',
  proto: 'protobuf',
  graphql: 'graphql',
  gql: 'graphql',
  dockerfile: 'dockerfile',
  tf: 'hcl',
  ini: 'ini',
  cfg: 'ini',
  conf: 'ini',
};

/** Map file extensions to Tailwind text colour classes for the file icon. */
const EXT_COLOUR: Record<string, string> = {
  ts: 'text-blue-400',
  tsx: 'text-blue-400',
  mts: 'text-blue-400',
  js: 'text-yellow-400',
  jsx: 'text-yellow-400',
  mjs: 'text-yellow-400',
  rs: 'text-orange-400',
  py: 'text-green-400',
  pyw: 'text-green-400',
  json: 'text-yellow-300',
  json5: 'text-yellow-300',
  yaml: 'text-purple-400',
  yml: 'text-purple-400',
  toml: 'text-orange-300',
  html: 'text-orange-400',
  htm: 'text-orange-400',
  xml: 'text-orange-300',
  css: 'text-blue-300',
  scss: 'text-pink-400',
  less: 'text-pink-300',
  sql: 'text-cyan-400',
  kt: 'text-purple-300',
  kts: 'text-purple-300',
  java: 'text-red-400',
  go: 'text-cyan-300',
  sh: 'text-green-300',
  bash: 'text-green-300',
  md: 'text-gray-300',
  markdown: 'text-gray-300',
  swift: 'text-orange-300',
  proto: 'text-teal-400',
  graphql: 'text-pink-300',
  dockerfile: 'text-blue-300',
  tf: 'text-purple-400',
  lock: 'text-gray-400',
  gitignore: 'text-gray-400',
  env: 'text-green-300',
};

function fileIconColour(name: string): string {
  const ext = name.includes('.') ? (name.split('.').pop()?.toLowerCase() ?? '') : '';
  if (!ext) return 'text-text-secondary';
  // Special dotfiles
  if (name.startsWith('.')) return EXT_COLOUR[name.replace('.', '')] ?? EXT_COLOUR[ext] ?? 'text-gray-400';
  return EXT_COLOUR[ext] ?? 'text-text-secondary';
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function getHighlightedHtml(content: string, path: string): string {
  const ext = path.includes('.') ? (path.split('.').pop()?.toLowerCase() ?? '') : '';

  if (ext === 'md' || ext === 'markdown') {
    const raw = marked.parse(content, { async: false }) as string;
    return DOMPurify.sanitize(raw);
  }

  const lang = EXT_TO_LANG[ext];
  try {
    if (lang) {
      return hljs.highlight(content, { language: lang }).value;
    }
    // Use auto-detection for unknown extensions
    return hljs.highlightAuto(content).value;
  } catch {
    return escapeHtml(content);
  }
}

@Component({
  selector: 'app-source',
  standalone: true,
  imports: [FormsModule],
  host: { class: 'block h-full' },
  styles: [
    `
      @import 'highlight.js/styles/atom-one-dark.css';

      /* Override hljs background so it uses our surface colour */
      pre code.hljs {
        background: transparent !important;
        padding: 0 !important;
      }
      /* Markdown prose styles */
      .md-prose h1,
      .md-prose h2,
      .md-prose h3,
      .md-prose h4 {
        font-weight: 600;
        margin-top: 1.25rem;
        margin-bottom: 0.5rem;
        color: var(--color-text-primary, #e6edf3);
      }
      .md-prose h1 {
        font-size: 1.5rem;
      }
      .md-prose h2 {
        font-size: 1.25rem;
      }
      .md-prose h3 {
        font-size: 1.125rem;
      }
      .md-prose p {
        margin-bottom: 0.75rem;
        line-height: 1.6;
      }
      .md-prose ul,
      .md-prose ol {
        padding-left: 1.5rem;
        margin-bottom: 0.75rem;
      }
      .md-prose ul {
        list-style-type: disc;
      }
      .md-prose ol {
        list-style-type: decimal;
      }
      .md-prose li {
        margin-bottom: 0.25rem;
      }
      .md-prose code {
        background: rgba(255, 255, 255, 0.07);
        border-radius: 0.25rem;
        padding: 0.1rem 0.3rem;
        font-family: monospace;
        font-size: 0.875em;
      }
      .md-prose pre {
        background: rgba(255, 255, 255, 0.05);
        border-radius: 0.375rem;
        padding: 1rem;
        overflow-x: auto;
        margin-bottom: 1rem;
      }
      .md-prose pre code {
        background: transparent;
        padding: 0;
      }
      .md-prose blockquote {
        border-left: 3px solid rgba(255, 255, 255, 0.2);
        padding-left: 1rem;
        margin-left: 0;
        color: rgba(255, 255, 255, 0.6);
      }
      .md-prose a {
        color: #60a5fa;
        text-decoration: underline;
      }
      .md-prose table {
        border-collapse: collapse;
        width: 100%;
        margin-bottom: 1rem;
        font-size: 0.875rem;
      }
      .md-prose th,
      .md-prose td {
        border: 1px solid rgba(255, 255, 255, 0.15);
        padding: 0.5rem 0.75rem;
        text-align: left;
      }
      .md-prose th {
        background: rgba(255, 255, 255, 0.05);
        font-weight: 600;
      }
      .md-prose hr {
        border: none;
        border-top: 1px solid rgba(255, 255, 255, 0.15);
        margin: 1.5rem 0;
      }
    `,
  ],
  template: `
    <div class="flex h-full overflow-hidden">
      <!-- ─── File tree panel ──────────────────────────────────── -->
      <aside [class.hidden]="!treeVisible()" [class.flex]="treeVisible()"
             class="w-64 flex-shrink-0 border-r border-border flex-col overflow-hidden">

        <!-- Panel header -->
        <div class="flex items-center justify-between px-4 py-3 border-b border-border">
          <h2 class="text-sm font-semibold text-text-primary">Source</h2>
          <div class="flex items-center gap-1">
            <!-- Hide tree on mobile -->
            <button (click)="treeVisible.set(false)"
                    class="sm:hidden p-1 rounded text-text-secondary hover:text-text-primary hover:bg-accent/10 transition-colors"
                    title="Hide tree">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
            <button
              (click)="refresh()"
              [disabled]="treeLoading()"
              title="Refresh"
              class="p-1 rounded text-text-secondary hover:text-text-primary hover:bg-accent/10 transition-colors disabled:opacity-50">
              <svg class="w-4 h-4" [class.animate-spin]="treeLoading()" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
            </button>
          </div>
        </div>

        <!-- Ref selector (branch / task) -->
        <div class="px-3 py-2 border-b border-border">
          <select
            [(value)]="selectedRef"
            (change)="onRefChange($event)"
            class="w-full text-xs bg-surface text-text-primary rounded px-2 py-1 border border-border focus:outline-none focus:ring-1 focus:ring-accent">
            <option [value]="defaultBranch()">{{ defaultBranch() }}</option>
            @if (regularBranches().length > 0) {
              <optgroup label="Branches">
                @for (branch of regularBranches(); track branch) {
                  <option [value]="branch">{{ branch }}</option>
                }
              </optgroup>
            }
            @if (agentBranches().length > 0) {
              <optgroup label="Agent tasks">
                @for (ab of agentBranches(); track ab.branch) {
                  <option [value]="ab.branch">{{ ab.label }}</option>
                }
              </optgroup>
            }
          </select>
        </div>

        <!-- Search / filter input -->
        <div class="px-3 py-2 border-b border-border">
          <div class="relative">
            <svg class="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-text-secondary pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
            <input
              type="text"
              [ngModel]="filterText()"
              (ngModelChange)="filterText.set($event)"
              placeholder="Filter files…"
              class="w-full pl-6 pr-2 py-1 text-xs bg-surface text-text-primary rounded border border-border focus:outline-none focus:ring-1 focus:ring-accent placeholder-text-secondary"
              (keydown.escape)="filterText.set('')"
            />
            @if (filterText()) {
              <button
                (click)="filterText.set('')"
                class="absolute right-1.5 top-1/2 -translate-y-1/2 text-text-secondary hover:text-text-primary">
                <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            }
          </div>
        </div>

        <!-- Tree error -->
        @if (treeError() && !noRepoConfigured()) {
          <div class="px-3 py-2 text-xs text-ctp-red">{{ treeError() }}</div>
        }

        <!-- No repo configured notice in sidebar -->
        @if (noRepoConfigured()) {
          <div class="flex-1 flex flex-col items-center justify-center gap-2 px-4 py-6 text-center">
            <svg class="w-8 h-8 text-text-secondary opacity-40" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
              <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 9.776c.112-.017.227-.026.344-.026h15.812c.117 0 .232.009.344.026m-16.5 0a2.25 2.25 0 00-1.883 2.542l.857 6a2.25 2.25 0 002.227 1.932H19.05a2.25 2.25 0 002.227-1.932l.857-6a2.25 2.25 0 00-1.883-2.542m-16.5 0V6A2.25 2.25 0 016 3.75h3.879a1.5 1.5 0 011.06.44l2.122 2.12a1.5 1.5 0 001.06.44H18A2.25 2.25 0 0120.25 9v.776" />
            </svg>
            <p class="text-xs text-text-secondary">No repository configured</p>
          </div>
        }

        <!-- Tree loading skeleton -->
        @if (treeLoading() && displayNodes().length === 0 && !noRepoConfigured()) {
          <div class="flex-1 overflow-auto px-3 py-2 space-y-1.5">
            @for (_ of [1,2,3,4,5]; track $index) {
              <div class="h-4 bg-surface rounded animate-pulse"></div>
            }
          </div>
        }

        <!-- Tree nodes -->
        @if ((!treeLoading() || displayNodes().length > 0) && !noRepoConfigured()) {
          <div class="flex-1 overflow-auto py-1 font-mono text-xs">
            @for (node of filteredDisplayNodes(); track node.entry.path; let i = $index) {
              <button
                #treeBtn
                (click)="onNodeClick(node)"
                (focus)="focusedNodeIndex.set(i)"
                (keydown.arrowdown)="$event.preventDefault(); moveFocus(i + 1)"
                (keydown.arrowup)="$event.preventDefault(); moveFocus(i - 1)"
                (keydown.enter)="$event.preventDefault(); onNodeClick(node)"
                (keydown.arrowright)="$event.preventDefault(); onExpandDir(node)"
                (keydown.arrowleft)="$event.preventDefault(); onCollapseDir(node)"
                [tabindex]="focusedNodeIndex() === i ? 0 : -1"
                [style.padding-left.px]="12 + node.depth * 14"
                class="w-full flex items-center gap-1.5 pr-2 py-0.5 text-left hover:bg-accent/10 transition-colors focus:outline-none focus:bg-accent/10"
                [class.text-accent]="selectedPath() === node.entry.path && node.entry.kind === 'file'"
                [class.text-text-primary]="selectedPath() !== node.entry.path || node.entry.kind === 'dir'"
                [class.bg-accent]="selectedPath() === node.entry.path && node.entry.kind === 'file'">

                <!-- Directory icon -->
                @if (node.entry.kind === 'dir') {
                  <svg class="w-3.5 h-3.5 flex-shrink-0 text-ctp-yellow" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    @if (node.expanded) {
                      <path stroke-linecap="round" stroke-linejoin="round" d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
                    } @else {
                      <path stroke-linecap="round" stroke-linejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                    }
                  </svg>
                } @else {
                  <!-- File icon coloured by extension -->
                  <svg
                    class="w-3.5 h-3.5 flex-shrink-0"
                    [class]="fileIconColour(node.entry.name)"
                    fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                  </svg>
                }
                <span class="truncate">{{ node.entry.name }}</span>
              </button>
            }

            @if (!treeLoading() && filteredDisplayNodes().length === 0 && !treeError()) {
              <div class="px-3 py-4 text-text-secondary text-center">
                @if (filterText()) { No files match "{{ filterText() }}". }
                @else { No files found. }
              </div>
            }
          </div>
        }
      </aside>

      <!-- ─── File content panel ────────────────────────────────── -->
      <main class="flex-1 flex flex-col overflow-hidden min-w-0">
        <!-- Show tree toggle (mobile only, when tree is hidden) -->
        @if (!treeVisible()) {
          <div class="sm:hidden flex items-center gap-2 px-3 py-2 border-b border-border">
            <button (click)="treeVisible.set(true)"
                    class="flex items-center gap-1.5 text-xs text-text-secondary hover:text-text-primary">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 10h16M4 14h8" />
              </svg>
              Files
            </button>
          </div>
        }
        @if (noRepoConfigured()) {
          <!-- No repository configured empty state -->
          <div class="flex-1 flex flex-col items-center justify-center text-text-secondary gap-4 px-8 max-w-lg mx-auto text-center">
            <svg class="w-14 h-14 opacity-25" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
              <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 9.776c.112-.017.227-.026.344-.026h15.812c.117 0 .232.009.344.026m-16.5 0a2.25 2.25 0 00-1.883 2.542l.857 6a2.25 2.25 0 002.227 1.932H19.05a2.25 2.25 0 002.227-1.932l.857-6a2.25 2.25 0 00-1.883-2.542m-16.5 0V6A2.25 2.25 0 016 3.75h3.879a1.5 1.5 0 011.06.44l2.122 2.12a1.5 1.5 0 001.06.44H18A2.25 2.25 0 0120.25 9v.776" />
            </svg>
            <div class="space-y-1">
              <p class="text-base font-medium text-text-primary">No repository configured</p>
              <p class="text-sm">This project doesn't have a git repository path set up yet.</p>
            </div>
            <div class="w-full rounded-lg border border-border bg-surface p-4 text-left space-y-3 text-xs font-mono">
              <p class="text-text-secondary font-sans font-medium text-xs uppercase tracking-wide">How to configure</p>
              <div class="space-y-2">
                <p class="text-text-secondary">Option 1 — per-project path (recommended):</p>
                <div class="space-y-1">
                  <p class="text-text-primary">Set <span class="text-ctp-peach">PROJECTS_PATH</span> to your projects base directory@if (projectsPath()) { (<code class="font-mono bg-surface/50 px-1 rounded text-xs">{{ projectsPath() }}</code>)}, then update this project's <span class="text-ctp-peach">git_root</span> field to the relative path of this repo.</p>
                </div>
              </div>
              <div class="space-y-1">
                <p class="text-text-secondary">Option 2 — global fallback:</p>
                <p class="text-text-primary">Set <span class="text-ctp-peach">REPO_ROOT</span> to the absolute path of the git repository.</p>
              </div>
            </div>
          </div>
        } @else if (!selectedPath()) {
          <!-- Empty state -->
          <div class="flex-1 flex flex-col items-center justify-center text-text-secondary gap-3">
            <svg class="w-12 h-12 opacity-30" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
              <path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
            <p class="text-sm">Select a file to view its contents</p>
          </div>
        } @else {
          <!-- File header -->
          <div class="flex items-center px-4 py-2 border-b border-border gap-2 min-h-[37px]">
            <span class="text-xs font-mono text-text-secondary truncate flex-1">{{ selectedPath() }}</span>
            <div class="flex items-center gap-2 flex-shrink-0 ml-auto">
              @if (blobLoading()) {
                <svg class="w-3.5 h-3.5 animate-spin text-text-secondary" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
              }
              @if (blobContent() !== null && !blobLoading()) {
                <!-- Copy to clipboard button -->
                <button
                  (click)="copyToClipboard()"
                  [title]="copySuccess() ? 'Copied!' : 'Copy to clipboard'"
                  class="flex items-center gap-1 px-2 py-0.5 rounded text-xs text-text-secondary hover:text-text-primary hover:bg-accent/10 transition-colors border border-transparent hover:border-border">
                  @if (copySuccess()) {
                    <svg class="w-3.5 h-3.5 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                    </svg>
                    <span class="text-green-400">Copied</span>
                  } @else {
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                    </svg>
                    <span>Copy</span>
                  }
                </button>
              }
            </div>
          </div>

          <!-- File content -->
          <div class="flex-1 overflow-auto">
            @if (blobError()) {
              <div class="p-4 text-sm text-ctp-red">{{ blobError() }}</div>
            } @else if (blobLoading()) {
              <div class="p-4 space-y-2">
                @for (_ of [1,2,3,4,5,6,7,8]; track $index) {
                  <div class="h-4 bg-surface rounded animate-pulse" [style.width.%]="60 + $index * 3 % 40"></div>
                }
              </div>
            } @else if (isMarkdown() && highlightedHtml() !== null) {
              <!-- Markdown: rendered HTML -->
              <div
                class="md-prose p-5 text-sm text-text-primary leading-relaxed"
                [innerHTML]="highlightedHtml()">
              </div>
            } @else if (blobContent() !== null) {
              <!-- Code: line numbers + syntax highlighted -->
              <div class="flex overflow-auto min-h-full">
                <!-- Line numbers gutter -->
                <div
                  class="select-none text-right py-4 pr-3 pl-4 text-xs font-mono text-text-secondary leading-[1.6] border-r border-border flex-shrink-0 min-w-[3rem]"
                  aria-hidden="true">
                  @for (n of lineNumbers(); track n) {
                    <div>{{ n }}</div>
                  }
                </div>
                <!-- Highlighted code -->
                <pre
                  class="flex-1 py-4 px-4 text-xs font-mono text-text-primary leading-[1.6] whitespace-pre overflow-visible m-0"
                ><code class="hljs" [innerHTML]="highlightedHtml()"></code></pre>
              </div>
            }
          </div>
        }
      </main>
    </div>
  `,
})
export class SourcePage {
  private http = inject(HttpClient);
  private readonly baseUrl = environment.apiServer;
  private readonly projectId = localStorage.getItem('diraigent-project') ?? '';

  @ViewChildren('treeBtn') treeBtns!: QueryList<ElementRef<HTMLButtonElement>>;

  // ── UI state ──────────────────────────────────────────────────────────────
  /** The active git ref for source browsing. Initialised to the project's
   *  default_branch (fetched from API), falling back to 'main'. */
  selectedRef = 'main';
  defaultBranch = signal('main');
  selectedPath = signal<string | null>(null);
  filterText = signal('');
  copySuccess = signal(false);
  treeVisible = signal(true);
  focusedNodeIndex = signal(0);

  // ── Remote data ───────────────────────────────────────────────────────────
  branches = signal<string[]>([]);
  taskTitleMap = signal<Record<string, string>>({});
  treeLoading = signal(false);
  treeError = signal<string | null>(null);
  blobLoading = signal(false);
  blobError = signal<string | null>(null);
  blobContent = signal<string | null>(null);
  projectsPath = signal<string | null>(null);

  private allEntries = signal<TreeEntry[]>([]);
  private expandedDirs = signal<Set<string>>(new Set());

  // ── Computed ──────────────────────────────────────────────────────────────

  private childrenByParent = computed(() => {
    const children = new Map<string, TreeEntry[]>();

    for (const entry of this.allEntries()) {
      const parent = entry.path.substring(0, entry.path.lastIndexOf('/')) || '';
      const siblings = children.get(parent);
      if (siblings) {
        siblings.push(entry);
      } else {
        children.set(parent, [entry]);
      }
    }

    for (const siblings of children.values()) {
      siblings.sort((a, b) => {
        if (a.kind !== b.kind) return a.kind === 'dir' ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
    }

    return children;
  });

  displayNodes = computed<DisplayNode[]>(() => {
    const childrenByParent = this.childrenByParent();
    const expanded = this.expandedDirs();
    const result: DisplayNode[] = [];

    const buildVisible = (parentPath: string, depth: number) => {
      const children = childrenByParent.get(parentPath) ?? [];

      for (const entry of children) {
        const isExpanded = expanded.has(entry.path);
        const hasChildren = childrenByParent.has(entry.path);
        result.push({ entry, depth, expanded: isExpanded, hasChildren });
        if (entry.kind === 'dir' && isExpanded) {
          buildVisible(entry.path, depth + 1);
        }
      }
    };

    buildVisible('', 0);
    return result;
  });

  filteredDisplayNodes = computed<DisplayNode[]>(() => {
    const filter = this.filterText().toLowerCase().trim();
    if (!filter) return this.displayNodes();
    return this.displayNodes().filter(n =>
      n.entry.name.toLowerCase().includes(filter) ||
      n.entry.path.toLowerCase().includes(filter),
    );
  });

  /** Syntax-highlighted HTML (or rendered markdown) for the current file. */
  highlightedHtml = computed<string | null>(() => {
    const content = this.blobContent();
    const path = this.selectedPath();
    if (content === null || !path) return null;
    return getHighlightedHtml(content, path);
  });

  /** 1-based line number array for non-markdown files. */
  lineNumbers = computed<number[]>(() => {
    const content = this.blobContent();
    const path = this.selectedPath();
    if (!content || !path) return [];
    const ext = path.includes('.') ? (path.split('.').pop()?.toLowerCase() ?? '') : '';
    if (ext === 'md' || ext === 'markdown') return [];
    return Array.from({ length: content.split('\n').length }, (_, i) => i + 1);
  });

  isMarkdown = computed(() => {
    const path = this.selectedPath();
    if (!path) return false;
    const ext = path.includes('.') ? (path.split('.').pop()?.toLowerCase() ?? '') : '';
    return ext === 'md' || ext === 'markdown';
  });

  /** True when the error indicates no git repository is configured for this project. */
  noRepoConfigured = computed(() => {
    const err = this.treeError();
    return !!err && err.toLowerCase().includes('no repository path configured');
  });

  /** Branches that are not agent/* task branches. */
  regularBranches = computed(() =>
    this.branches().filter(b => !b.startsWith('agent/')),
  );

  /** Agent task branches with optional task title label. */
  agentBranches = computed(() => {
    const titleMap = this.taskTitleMap();
    return this.branches()
      .filter(b => b.startsWith('agent/task-'))
      .map(b => {
        const shortId = b.slice('agent/task-'.length); // first 12 chars of task uuid
        const title = titleMap[shortId];
        return {
          branch: b,
          label: title ? `${b} — ${title}` : b,
        };
      });
  });

  // ── Lifecycle ─────────────────────────────────────────────────────────────

  constructor() {
    this.loadProjectAndTree();
    this.loadBranches();
    this.loadTasks();
    this.loadSettings();
  }

  // ── Public template helpers ───────────────────────────────────────────────

  fileIconColour(name: string): string {
    return fileIconColour(name);
  }

  // ── Event handlers ────────────────────────────────────────────────────────

  onRefChange(event: Event): void {
    this.selectedRef = (event.target as HTMLSelectElement).value;
    this.selectedPath.set(null);
    this.blobContent.set(null);
    this.expandedDirs.set(new Set());
    this.allEntries.set([]);
    this.filterText.set('');
    this.loadTree('');
  }

  onNodeClick(node: DisplayNode): void {
    if (node.entry.kind === 'dir') {
      this.toggleDir(node.entry.path);
      const hasChildren = this.childrenByParent().has(node.entry.path);
      if (!hasChildren) {
        this.loadTree(node.entry.path);
      }
    } else {
      this.selectedPath.set(node.entry.path);
      this.loadBlob(node.entry.path);
      // On mobile, hide the tree to show file content
      if (window.innerWidth < 640) {
        this.treeVisible.set(false);
      }
    }
  }

  /** Expand a directory node (keyboard right-arrow). */
  onExpandDir(node: DisplayNode): void {
    if (node.entry.kind !== 'dir') return;
    if (!this.expandedDirs().has(node.entry.path)) {
      this.toggleDir(node.entry.path);
      const hasChildren = this.childrenByParent().has(node.entry.path);
      if (!hasChildren) {
        this.loadTree(node.entry.path);
      }
    }
  }

  /** Collapse a directory node (keyboard left-arrow). */
  onCollapseDir(node: DisplayNode): void {
    if (node.entry.kind !== 'dir') return;
    if (this.expandedDirs().has(node.entry.path)) {
      this.toggleDir(node.entry.path);
    }
  }

  refresh(): void {
    this.expandedDirs.set(new Set());
    this.allEntries.set([]);
    this.selectedPath.set(null);
    this.blobContent.set(null);
    this.filterText.set('');
    this.loadTree('');
  }

  /** Move keyboard focus to the given tree-node index. */
  moveFocus(index: number): void {
    const nodes = this.filteredDisplayNodes();
    const clamped = Math.max(0, Math.min(index, nodes.length - 1));
    this.focusedNodeIndex.set(clamped);
    // Give Angular a tick to update tabindex before focusing
    setTimeout(() => {
      const btn = this.treeBtns.get(clamped);
      btn?.nativeElement.focus();
    }, 0);
  }

  async copyToClipboard(): Promise<void> {
    const content = this.blobContent();
    if (!content) return;
    try {
      await navigator.clipboard.writeText(content);
      this.copySuccess.set(true);
      setTimeout(() => this.copySuccess.set(false), 2000);
    } catch {
      // Clipboard API unavailable — silently ignore
    }
  }

  // ── Private helpers ───────────────────────────────────────────────────────

  private toggleDir(path: string): void {
    this.expandedDirs.update(set => {
      const next = new Set(set);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }

  /** Fetch the project record to learn its default_branch, then load the tree. */
  private loadProjectAndTree(): void {
    if (!this.projectId) {
      this.loadTree('');
      return;
    }
    this.http
      .get<{ default_branch?: string }>(`${this.baseUrl}/${this.projectId}`)
      .subscribe({
        next: resp => {
          const branch = resp.default_branch || 'main';
          this.defaultBranch.set(branch);
          this.selectedRef = branch;
          this.loadTree('');
        },
        error: () => {
          // Fall back to 'main' if project fetch fails
          this.loadTree('');
        },
      });
  }

  private loadTree(dirPath: string): void {
    if (!this.projectId) return;
    this.treeLoading.set(true);
    this.treeError.set(null);

    let params = new HttpParams().set('ref', this.selectedRef);
    if (dirPath) params = params.set('path', dirPath);

    this.http
      .get<{ entries: TreeEntry[] }>(`${this.baseUrl}/${this.projectId}/source/tree`, { params })
      .subscribe({
        next: resp => {
          this.allEntries.update(existing => {
            const map = new Map(existing.map(e => [e.path, e]));
            for (const e of resp.entries) map.set(e.path, e);
            return Array.from(map.values());
          });
          if (dirPath) {
            this.expandedDirs.update(s => new Set([...s, dirPath]));
          }
          this.treeLoading.set(false);
        },
        error: err => {
          this.treeError.set(err?.error?.error || err?.message || 'Failed to load file tree');
          this.treeLoading.set(false);
        },
      });
  }

  private loadBlob(filePath: string): void {
    if (!this.projectId) return;
    this.blobLoading.set(true);
    this.blobError.set(null);
    this.blobContent.set(null);

    const params = new HttpParams().set('ref', this.selectedRef).set('path', filePath);

    this.http
      .get<{ content: string; encoding: string }>(`${this.baseUrl}/${this.projectId}/source/blob`, {
        params,
      })
      .subscribe({
        next: resp => {
          if (resp.encoding === 'base64') {
            try {
              this.blobContent.set(atob(resp.content));
            } catch {
              this.blobContent.set(resp.content);
            }
          } else {
            this.blobContent.set(resp.content);
          }
          this.blobLoading.set(false);
        },
        error: err => {
          this.blobError.set(err?.error?.error || err?.message || 'Failed to load file');
          this.blobLoading.set(false);
        },
      });
  }

  private loadBranches(): void {
    if (!this.projectId) return;
    this.http
      .get<{ branches: { name: string }[] }>(`${this.baseUrl}/${this.projectId}/git/branches?prefix=`)
      .subscribe({
        next: resp => {
          const defBranch = this.defaultBranch();
          const filtered = (resp.branches ?? []).map(b => b.name).filter(b => b !== defBranch);
          this.branches.set(filtered);
        },
        error: () => {
          // Branches are optional; ignore errors
        },
      });
  }

  /** Load server settings (e.g. PROJECTS_PATH) for display in hints. */
  private loadSettings(): void {
    this.http
      .get<{ projects_path: string | null }>(`${this.baseUrl}/settings`)
      .subscribe({
        next: resp => this.projectsPath.set(resp.projects_path),
        error: () => {
          // Settings are optional; ignore errors
        },
      });
  }

  /** Load project tasks and build a shortId → title lookup for agent/* branches. */
  private loadTasks(): void {
    if (!this.projectId) return;
    this.http
      .get<TaskSummary[] | { tasks: TaskSummary[] }>(`${this.baseUrl}/${this.projectId}/tasks`)
      .subscribe({
        next: resp => {
          const list: TaskSummary[] = Array.isArray(resp) ? resp : (resp as { tasks: TaskSummary[] }).tasks ?? [];
          const map: Record<string, string> = {};
          for (const task of list) {
            // Branch short id = first 12 chars of the UUID
            const shortId = task.id.slice(0, 12);
            map[shortId] = `#${task.number} ${task.title}`;
          }
          this.taskTitleMap.set(map);
        },
        error: () => {
          // Task metadata is optional; ignore errors
        },
      });
  }
}
