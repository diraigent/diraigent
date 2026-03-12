import { Component, ElementRef, ViewChild, inject, signal, computed, effect, NgZone, AfterViewInit, OnDestroy } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { TranslocoModule, TranslocoService } from '@jsverse/transloco';
import { Router, RouterLink } from '@angular/router';
import { ProjectContext } from '../../core/services/project-context.service';
import { TasksApiService, CreateTaskRequest } from '../../core/services/tasks-api.service';
import { ScratchpadApiService } from '../../core/services/scratchpad-api.service';
import { TaskFormComponent } from '../tasks/components/task-form/task-form';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import Placeholder from '@tiptap/extension-placeholder';
import { Markdown } from 'tiptap-markdown';

type Tab = 'notes' | 'todos';

interface TodoItem {
  id: string;
  text: string;
  done: boolean;
  createdAt: string;
  taskId?: string;
}

interface ScratchpadData {
  notes: string;
  todos: TodoItem[];
}

const STORAGE_KEY = 'diraigent-scratchpad';

function loadData(projectId: string): ScratchpadData {
  try {
    const raw = localStorage.getItem(`${STORAGE_KEY}-${projectId}`);
    if (raw) return JSON.parse(raw) as ScratchpadData;
  } catch {
    // ignore
  }
  return { notes: '', todos: [] };
}

function saveData(projectId: string, data: ScratchpadData): void {
  try {
    localStorage.setItem(`${STORAGE_KEY}-${projectId}`, JSON.stringify(data));
  } catch {
    // ignore
  }
}

@Component({
  selector: 'app-scratchpad',
  standalone: true,
  imports: [FormsModule, TranslocoModule, RouterLink, TaskFormComponent],
  styles: [`
    :host ::ng-deep .tiptap-editor .ProseMirror {
      outline: none;
      padding: 0.75rem 1rem;
      min-height: 22rem;
      line-height: 1.6;
    }
    :host ::ng-deep .tiptap-editor .ProseMirror p.is-editor-empty:first-child::before {
      content: attr(data-placeholder);
      float: left;
      height: 0;
      pointer-events: none;
      color: var(--color-text-secondary, #8b949e);
    }
    :host ::ng-deep .tiptap-editor .ProseMirror h1 { font-size: 1.5rem; font-weight: 600; margin: 1rem 0 0.5rem; }
    :host ::ng-deep .tiptap-editor .ProseMirror h2 { font-size: 1.25rem; font-weight: 600; margin: 1rem 0 0.5rem; }
    :host ::ng-deep .tiptap-editor .ProseMirror h3 { font-size: 1.125rem; font-weight: 600; margin: 0.75rem 0 0.25rem; }
    :host ::ng-deep .tiptap-editor .ProseMirror p { margin-bottom: 0.5rem; }
    :host ::ng-deep .tiptap-editor .ProseMirror ul { list-style: disc; padding-left: 1.5rem; margin-bottom: 0.5rem; }
    :host ::ng-deep .tiptap-editor .ProseMirror ol { list-style: decimal; padding-left: 1.5rem; margin-bottom: 0.5rem; }
    :host ::ng-deep .tiptap-editor .ProseMirror li { margin-bottom: 0.25rem; }
    :host ::ng-deep .tiptap-editor .ProseMirror li p { margin-bottom: 0; }
    :host ::ng-deep .tiptap-editor .ProseMirror code {
      background: rgba(255,255,255,0.07); border-radius: 0.25rem;
      padding: 0.1rem 0.3rem; font-family: ui-monospace, monospace; font-size: 0.875em;
    }
    :host ::ng-deep .tiptap-editor .ProseMirror pre {
      background: rgba(255,255,255,0.05); border-radius: 0.375rem;
      padding: 1rem; overflow-x: auto; margin-bottom: 1rem;
    }
    :host ::ng-deep .tiptap-editor .ProseMirror pre code { background: transparent; padding: 0; display: block; }
    :host ::ng-deep .tiptap-editor .ProseMirror blockquote {
      border-left: 3px solid rgba(255,255,255,0.2);
      padding-left: 1rem; margin-left: 0; color: rgba(255,255,255,0.6);
    }
    :host ::ng-deep .tiptap-editor .ProseMirror hr {
      border: none; border-top: 1px solid rgba(255,255,255,0.15); margin: 1.5rem 0;
    }
    :host ::ng-deep .tiptap-editor .ProseMirror a { color: #60a5fa; text-decoration: underline; }
  `],
  template: `
    <div class="p-3 sm:p-6 max-w-4xl" *transloco="let t">
      <!-- Header -->
      <div class="flex items-center justify-between mb-3 sm:mb-6">
        <div>
          <h1 class="text-2xl font-semibold text-text-primary">{{ t('scratchpad.title') }}</h1>
          <p class="text-sm text-text-secondary mt-0.5">{{ t('scratchpad.subtitle') }}</p>
        </div>
        <div class="flex items-center gap-3">
          <!-- Auto-save indicator -->
          @if (saved()) {
            <span class="text-xs text-ctp-green flex items-center gap-1">
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path d="M5 13l4 4L19 7" />
              </svg>
              {{ t('scratchpad.saved') }}
            </span>
          }
          <!-- New Task button -->
          <button
            (click)="openNewTaskForm()"
            class="flex items-center gap-1.5 px-3 py-1.5 bg-accent text-bg rounded-lg text-xs font-medium hover:opacity-90 transition-opacity">
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
              <path d="M12 4v16m8-8H4" />
            </svg>
            {{ t('scratchpad.newTask') }}
          </button>
        </div>
      </div>

      <!-- New Task modal (shared with tasks page) -->
      <app-task-form
        [show]="showNewTaskForm()"
        [editing]="null"
        (submitCreate)="onCreateTask($event)"
        (closed)="closeNewTaskForm()" />

      <!-- Tabs -->
      <div class="flex gap-1 mb-6 bg-surface rounded-lg p-1 w-fit border border-border">
        <button
          (click)="activeTab.set('notes')"
          class="px-4 py-1.5 rounded-md text-sm font-medium transition-colors"
          [class]="activeTab() === 'notes'
            ? 'bg-accent text-bg shadow-sm'
            : 'text-text-secondary hover:text-text-primary'">
          {{ t('scratchpad.tabNotes') }}
        </button>
        <button
          (click)="activeTab.set('todos')"
          class="px-4 py-1.5 rounded-md text-sm font-medium transition-colors flex items-center gap-2"
          [class]="activeTab() === 'todos'
            ? 'bg-accent text-bg shadow-sm'
            : 'text-text-secondary hover:text-text-primary'">
          {{ t('scratchpad.tabTodos') }}
          @if (pendingCount() > 0) {
            <span class="text-xs rounded-full px-1.5 py-0.5 min-w-[1.25rem] text-center"
              [class]="activeTab() === 'todos' ? 'bg-bg/20' : 'bg-accent/20 text-accent'">
              {{ pendingCount() }}
            </span>
          }
        </button>
      </div>

      <!-- Notes Tab — tiptap editor (kept in DOM, hidden when inactive) -->
      <div [style.display]="activeTab() === 'notes' ? 'block' : 'none'" class="space-y-3">
        <div #editorEl
          class="tiptap-editor w-full bg-surface text-text-primary text-sm rounded-lg border border-border
                 focus-within:ring-1 focus-within:ring-accent overflow-auto max-h-[80vh]">
        </div>
        <p class="text-xs text-text-muted">{{ t('scratchpad.markdownHint') }}</p>
      </div>

      <!-- Todos Tab -->
      @if (activeTab() === 'todos') {
        <div class="space-y-4">
          <!-- Add todo -->
          <div class="flex gap-2">
            <input
              type="text"
              [(ngModel)]="newTodoText"
              (keydown.enter)="addTodo()"
              [placeholder]="t('scratchpad.todoPlaceholder')"
              class="flex-1 bg-surface text-text-primary text-sm rounded-lg px-3 py-2 border border-border
                     focus:outline-none focus:ring-1 focus:ring-accent placeholder:text-text-secondary" />
            <button
              (click)="addTodo()"
              [disabled]="!newTodoText.trim()"
              class="px-4 py-2 bg-ctp-mauve text-bg rounded-lg text-sm font-medium hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-opacity">
              {{ t('scratchpad.addTodo') }}
            </button>
          </div>

          <!-- Todo stats -->
          @if (todos().length > 0) {
            <div class="flex items-center justify-between">
              <p class="text-xs text-text-secondary">
                {{ doneCount() }}/{{ todos().length }} {{ t('scratchpad.todosComplete') }}
              </p>
              @if (doneCount() > 0) {
                <button (click)="clearDone()" class="text-xs text-text-secondary hover:text-ctp-red transition-colors">
                  {{ t('scratchpad.clearDone') }}
                </button>
              }
            </div>
            <!-- Progress bar -->
            <div class="h-1.5 bg-surface rounded-full overflow-hidden border border-border">
              <div class="h-full bg-ctp-green rounded-full transition-all duration-300"
                   [style.width.%]="progressPct()"></div>
            </div>
          }

          <!-- Todo list -->
          <div class="space-y-1.5">
            @for (todo of todos(); track todo.id) {
              <div class="flex items-center gap-3 p-3 rounded-lg border transition-colors group"
                   [class]="todo.done ? 'bg-surface/50 border-border/50' : 'bg-surface border-border'">
                <button
                  (click)="toggleTodo(todo.id)"
                  class="flex-shrink-0 w-5 h-5 rounded border-2 flex items-center justify-center transition-colors"
                  [class]="todo.done
                    ? 'bg-ctp-green border-ctp-green'
                    : 'border-border hover:border-accent'">
                  @if (todo.done) {
                    <svg class="w-3 h-3 text-bg" fill="none" stroke="currentColor" stroke-width="3" viewBox="0 0 24 24">
                      <path d="M5 13l4 4L19 7" />
                    </svg>
                  }
                </button>
                <span class="flex-1 text-sm"
                      [class]="todo.done ? 'line-through text-text-secondary' : 'text-text-primary'">
                  {{ todo.text }}
                </span>
                <!-- Linked task badge -->
                @if (todo.taskId) {
                  <a [routerLink]="['/tasks']" [queryParams]="{ id: todo.taskId }"
                     class="flex-shrink-0 flex items-center gap-1 text-xs text-accent hover:underline"
                     [title]="t('scratchpad.viewTask')">
                    <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                      <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
                    </svg>
                  </a>
                }
                <!-- Promote to task button -->
                @if (!todo.taskId) {
                  <button
                    (click)="promoteToTask(todo)"
                    [disabled]="promotingId() === todo.id"
                    class="opacity-0 group-hover:opacity-100 p-1 text-text-secondary hover:text-accent rounded transition-all disabled:opacity-50"
                    [title]="t('scratchpad.promoteToTask')">
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                      <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4" />
                    </svg>
                  </button>
                }
                <button
                  (click)="deleteTodo(todo.id)"
                  class="opacity-0 group-hover:opacity-100 p-1 text-text-secondary hover:text-ctp-red rounded transition-all"
                  [title]="t('scratchpad.deleteTodo')">
                  <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                    <path d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            } @empty {
              <div class="text-center py-12 text-text-secondary">
                <svg class="w-10 h-10 mx-auto mb-3 opacity-30" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                  <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4" />
                </svg>
                <p class="text-sm">{{ t('scratchpad.todosEmpty') }}</p>
              </div>
            }
          </div>
        </div>
      }
    </div>
  `,
})
export class ScratchpadPage implements AfterViewInit, OnDestroy {
  private ctx = inject(ProjectContext);
  private tasksApi = inject(TasksApiService);
  private scratchpadApi = inject(ScratchpadApiService);
  private router = inject(Router);
  private zone = inject(NgZone);
  private transloco = inject(TranslocoService);

  @ViewChild('editorEl') editorEl!: ElementRef<HTMLElement>;
  private editor: Editor | null = null;

  activeTab = signal<Tab>('notes');
  saved = signal(false);
  private saveTimer: ReturnType<typeof setTimeout> | null = null;

  // New Task modal
  showNewTaskForm = signal(false);

  // Promote to task
  promotingId = signal<string | null>(null);

  notesContent = '';
  newTodoText = '';
  todos = signal<TodoItem[]>([]);

  pendingCount = computed(() => this.todos().filter(t => !t.done).length);
  doneCount = computed(() => this.todos().filter(t => t.done).length);
  progressPct = computed(() => {
    const total = this.todos().length;
    if (total === 0) return 0;
    return Math.round((this.doneCount() / total) * 100);
  });

  constructor() {
    effect(() => {
      const pid = this.ctx.projectId();
      if (!pid) return;
      // Load from server first; fall back to localStorage if the server returns nothing.
      this.scratchpadApi.get().subscribe({
        next: sp => {
          if (sp) {
            this.notesContent = sp.notes;
            this.todos.set(sp.todos as TodoItem[]);
            this.setEditorContent(sp.notes);
            // Keep localStorage in sync so offline reads still work.
            saveData(pid, { notes: sp.notes, todos: sp.todos as TodoItem[] });
          } else {
            // No server record yet — seed from localStorage if available.
            const local = loadData(pid);
            this.notesContent = local.notes;
            this.todos.set(local.todos);
            this.setEditorContent(local.notes);
          }
        },
        error: () => {
          // Server unavailable — fall back to localStorage.
          const local = loadData(pid);
          this.notesContent = local.notes;
          this.todos.set(local.todos);
          this.setEditorContent(local.notes);
        },
      });
    });
  }

  ngAfterViewInit(): void {
    this.initEditor();
  }

  ngOnDestroy(): void {
    this.editor?.destroy();
    this.editor = null;
  }

  private initEditor(): void {
    const el = this.editorEl?.nativeElement;
    if (!el) return;

    this.editor = new Editor({
      element: el,
      extensions: [
        StarterKit,
        Placeholder.configure({
          placeholder: this.transloco.translate('scratchpad.notesPlaceholder'),
        }),
        Markdown.configure({
          html: false,
          transformPastedText: true,
          transformCopiedText: true,
        }),
      ],
      content: this.notesContent || '',
      onUpdate: ({ editor }: { editor: Editor }) => {
        this.zone.run(() => {
          this.notesContent = (editor.storage['markdown'] as { getMarkdown: () => string }).getMarkdown();
          this.scheduleSave();
        });
      },
    });
  }

  /** Update the tiptap editor content (if initialized). */
  private setEditorContent(markdown: string): void {
    if (this.editor && !this.editor.isDestroyed) {
      this.editor.commands.setContent(markdown || '');
    }
  }

  private scheduleSave(): void {
    if (this.saveTimer) clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => {
      this.persist();
    }, 600);
  }

  private persist(): void {
    const pid = this.ctx.projectId();
    if (!pid) return;
    const data = { notes: this.notesContent, todos: this.todos() };
    // Always keep localStorage updated for offline/fallback reads.
    saveData(pid, data);
    // Persist to server; show "Saved" only when server confirms.
    this.scratchpadApi.upsert(data).subscribe({
      next: () => {
        this.saved.set(true);
        setTimeout(() => this.saved.set(false), 2000);
      },
      error: () => {
        // Server save failed — localStorage already updated, show saved anyway.
        this.saved.set(true);
        setTimeout(() => this.saved.set(false), 2000);
      },
    });
  }

  addTodo(): void {
    const text = this.newTodoText.trim();
    if (!text) return;
    const item: TodoItem = {
      id: crypto.randomUUID(),
      text,
      done: false,
      createdAt: new Date().toISOString(),
    };
    this.todos.update(list => [...list, item]);
    this.newTodoText = '';
    this.persist();
  }

  toggleTodo(id: string): void {
    this.todos.update(list =>
      list.map(t => (t.id === id ? { ...t, done: !t.done } : t)),
    );
    this.persist();
  }

  deleteTodo(id: string): void {
    this.todos.update(list => list.filter(t => t.id !== id));
    this.persist();
  }

  clearDone(): void {
    this.todos.update(list => list.filter(t => !t.done));
    this.persist();
  }

  openNewTaskForm(): void {
    this.showNewTaskForm.set(true);
  }

  closeNewTaskForm(): void {
    this.showNewTaskForm.set(false);
  }

  onCreateTask(req: CreateTaskRequest): void {
    this.tasksApi.create(req).subscribe({
      next: task => {
        this.closeNewTaskForm();
        void this.router.navigate(['/tasks'], { queryParams: { id: task.id } });
      },
    });
  }

  promoteToTask(todo: TodoItem): void {
    if (this.promotingId()) return;
    this.promotingId.set(todo.id);
    this.tasksApi.create({ title: todo.text, kind: 'chore' }).subscribe({
      next: task => {
        this.promotingId.set(null);
        this.todos.update(list =>
          list.map(t => (t.id === todo.id ? { ...t, taskId: task.id } : t)),
        );
        this.persist();
      },
      error: () => {
        this.promotingId.set(null);
      },
    });
  }
}
