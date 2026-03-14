import { Injectable, inject, signal, computed, effect, untracked } from '@angular/core';
import { AuthService } from './auth.service';
import { ProjectContext } from './project-context.service';
import { environment } from '../../../environments/environment';

export interface ChatMessage {
  role: 'user' | 'assistant';
  content: string;
}

interface ActiveTool {
  toolId: string;
  toolName: string;
}

const STORAGE_PREFIX = 'diraigent-chat-';
const MODEL_STORAGE_KEY = 'diraigent-chat-model';

/** Available chat models. */
export const CHAT_MODELS = ['sonnet', 'opus', 'haiku'] as const;
export type ChatModel = (typeof CHAT_MODELS)[number];

@Injectable({ providedIn: 'root' })
export class ChatService {
  private auth = inject(AuthService);
  private project = inject(ProjectContext);

  readonly messages = signal<ChatMessage[]>([]);
  readonly streaming = signal(false);
  readonly streamingText = signal('');
  readonly activeTools = signal<ActiveTool[]>([]);
  readonly toolsCompleted = signal(0);
  readonly error = signal<string | null>(null);
  readonly canSend = computed(() => !!this.project.projectId());
  readonly isOpen = signal(false);
  /** Emits true when the parent layout should scroll the chat panel into view (mobile). */
  readonly scrollToChat = signal(false);
  /** Whether the chat panel is collapsed to just the header. */
  readonly collapsed = signal(localStorage.getItem('diraigent-chat-collapsed') === 'true');
  /** The chat model name — user-selected or from server config. */
  readonly chatModel = signal<string>(localStorage.getItem(MODEL_STORAGE_KEY) || '');
  /** Whether the model selector dropdown is open. */
  readonly modelSelectorOpen = signal(false);

  private abortController: AbortController | null = null;
  private generation = 0;

  constructor() {
    this.fetchChatModel();
    // Load stored messages on init and when project changes
    effect(() => {
      const pid = this.project.projectId();
      untracked(() => {
        this.cancel();
        const stored = pid ? localStorage.getItem(STORAGE_PREFIX + pid) : null;
        let msgs: ChatMessage[] = [];
        if (stored) {
          try { msgs = JSON.parse(stored); } catch { /* ignore corrupt data */ }
        }
        this.messages.set(msgs);
        this.streaming.set(false);
        this.streamingText.set('');
        this.activeTools.set([]);
        this.toolsCompleted.set(0);
        this.error.set(null);
      });
    });
  }

  private persist(): void {
    const pid = this.project.projectId();
    if (!pid) return;
    const msgs = this.messages();
    if (msgs.length === 0) {
      localStorage.removeItem(STORAGE_PREFIX + pid);
    } else {
      localStorage.setItem(STORAGE_PREFIX + pid, JSON.stringify(msgs));
    }
  }

  async send(text: string): Promise<void> {
    const projectId = this.project.projectId();
    if (!projectId) return;
    const token = this.auth.getAccessToken();

    const userMsg: ChatMessage = { role: 'user', content: text };
    this.messages.update(msgs => [...msgs, userMsg]);
    this.persist();
    this.streaming.set(true);
    this.streamingText.set('');
    this.activeTools.set([]);
    this.toolsCompleted.set(0);
    this.error.set(null);

    const gen = ++this.generation;
    this.abortController = new AbortController();

    // Build history for the API (just role + content strings)
    const history = this.messages().map(m => ({ role: m.role, content: m.content }));

    try {
      const headers: Record<string, string> = { 'Content-Type': 'application/json' };
      if (token) headers['Authorization'] = `Bearer ${token}`;

      const body: Record<string, unknown> = { messages: history };
      const selectedModel = this.chatModel();
      if (selectedModel) body['model'] = selectedModel;

      const resp = await fetch(`${environment.apiServer}/${projectId}/chat`, {
        method: 'POST',
        headers,
        body: JSON.stringify(body),
        signal: this.abortController.signal,
      });

      if (!resp.ok) {
        const body = await resp.text();
        throw new Error(`HTTP ${resp.status}: ${body}`);
      }

      const reader = resp.body?.getReader();
      if (!reader) throw new Error('No response body');

      const decoder = new TextDecoder();
      let buffer = '';
      let accumulated = '';

      const processEvent = (part: string) => {
        const lines = part.split('\n');
        const eventLine = lines.find(l => l.startsWith('event: '));
        // SSE spec: multi-line data uses multiple "data:" lines joined by newlines
        const dataLines = lines.filter(l => l.startsWith('data: '));
        if (dataLines.length === 0) return;

        const eventType = eventLine?.slice(7) ?? '';
        const rawData = dataLines.map(l => l.slice(6)).join('\n');

        let data: Record<string, unknown>;
        try {
          data = JSON.parse(rawData);
        } catch {
          return; // skip malformed events
        }

        switch (eventType) {
          case 'text':
            accumulated += data['content'];
            this.streamingText.set(accumulated);
            break;

          case 'tool_start':
            this.activeTools.update(tools => [
              ...tools,
              {
                toolId: data['tool_id'] as string,
                toolName: data['tool_name'] as string,
              },
            ]);
            break;

          case 'tool_end':
            this.activeTools.update(tools =>
              tools.filter(t => t.toolId !== (data['tool_id'] as string)),
            );
            this.toolsCompleted.update(n => n + 1);
            break;

          case 'done':
            this.messages.update(msgs => [
              ...msgs,
              {
                role: 'assistant' as const,
                content: (data['message'] as Record<string, string>)['content'],
              },
            ]);
            this.persist();
            this.streamingText.set('');
            break;

          case 'error':
            this.error.set(data['message'] as string);
            break;
        }
      };

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });

        // Process complete SSE events (separated by blank lines)
        const parts = buffer.split('\n\n');
        buffer = parts.pop() ?? '';

        for (const part of parts) {
          if (part.trim()) processEvent(part);
        }
      }

      // Process any remaining buffered event after stream ends
      if (buffer.trim()) {
        processEvent(buffer);
      }
    } catch (e: unknown) {
      if (e instanceof DOMException && e.name === 'AbortError') {
        // User cancelled
      } else {
        this.error.set(e instanceof Error ? e.message : 'Unknown error');
      }
    } finally {
      // Guard: only clear streaming state if this is still the active generation
      // (prevents cancel+resend race where old finally clobbers new streaming flag)
      if (gen === this.generation) {
        if (this.streamingText() && !this.messages().some(m => m.content === this.streamingText())) {
          const text = this.streamingText();
          if (text) {
            this.messages.update(msgs => [...msgs, { role: 'assistant', content: text }]);
            this.persist();
          }
        }
        this.streaming.set(false);
        this.streamingText.set('');
        this.activeTools.set([]);
        this.toolsCompleted.set(0);
        this.abortController = null;
      }
    }
  }

  toggleCollapsed(): void {
    this.collapsed.update(v => !v);
    localStorage.setItem('diraigent-chat-collapsed', String(this.collapsed()));
  }

  setModel(model: string): void {
    this.chatModel.set(model);
    localStorage.setItem(MODEL_STORAGE_KEY, model);
    this.modelSelectorOpen.set(false);
  }

  toggleModelSelector(): void {
    this.modelSelectorOpen.update(v => !v);
  }

  /** Send a message (chat is always visible). Emits scrollToChat for mobile scroll-into-view. */
  openWithMessage(text?: string): void {
    this.isOpen.set(true);
    if (this.collapsed()) {
      this.collapsed.set(false);
      localStorage.setItem('diraigent-chat-collapsed', 'false');
    }
    this.scrollToChat.set(true);
    if (text) {
      this.send(text);
    }
  }

  cancel(): void {
    this.abortController?.abort();
  }

  clear(): void {
    this.messages.set([]);
    this.persist();
    this.streaming.set(false);
    this.streamingText.set('');
    this.activeTools.set([]);
    this.toolsCompleted.set(0);
    this.error.set(null);
    this.abortController?.abort();
    this.abortController = null;
  }

  private async fetchChatModel(): Promise<void> {
    try {
      const res = await fetch(`${environment.apiServer}/config`);
      if (!res.ok) return;
      const data = await res.json();
      // Only use server default if user hasn't explicitly selected a model
      if (data.chat_model && !localStorage.getItem(MODEL_STORAGE_KEY)) {
        this.chatModel.set(data.chat_model);
      }
    } catch {
      // Config endpoint unavailable — leave model blank
    }
  }
}
