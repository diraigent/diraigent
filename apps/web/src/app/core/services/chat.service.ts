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

interface ChatModelCatalog {
  provider: string;
  default_model: string | null;
  models: string[];
  agent_id: string;
}

import { STORAGE_KEYS } from '../../shared/ui-constants';

const STORAGE_PREFIX = STORAGE_KEYS.CHAT_PREFIX;
const MODEL_STORAGE_KEY = STORAGE_KEYS.CHAT_MODEL;

@Injectable({ providedIn: 'root' })
export class ChatService {
  private auth = inject(AuthService);
  private project = inject(ProjectContext);

  readonly messages = signal<ChatMessage[]>([]);
  readonly streaming = signal(false);
  readonly streamingText = signal('');
  readonly thinkingText = signal('');
  readonly activeTools = signal<ActiveTool[]>([]);
  readonly toolsCompleted = signal(0);
  readonly error = signal<string | null>(null);
  readonly canSend = computed(() => !!this.project.projectId());
  readonly isOpen = signal(false);
  /** Emits true when the parent layout should scroll the chat panel into view (mobile). */
  readonly scrollToChat = signal(false);
  /** Whether the chat panel is collapsed to just the header. */
  readonly collapsed = signal(localStorage.getItem('diraigent-chat-collapsed') === 'true');
  /** An explicit override; empty means the project/worker default. */
  readonly chatModel = signal('');
  readonly chatProvider = computed(() =>
    this.project.project()?.metadata?.['chat_provider'] as string || 'opencode');
  private readonly serverModel = signal('');
  private readonly catalog = signal<ChatModelCatalog | null>(null);
  readonly modelsLoading = signal(false);
  readonly modelsError = signal<string | null>(null);
  readonly modelSearch = signal('');
  private modelRequest: AbortController | null = null;
  private modelGeneration = 0;
  private modelContext = '';
  readonly defaultModel = computed(() => {
    const model = this.project.project()?.metadata?.['chat_model'];
    if (typeof model === 'string' && this.isValidModel(model)) return model;
    if (this.catalog()?.default_model) return this.catalog()!.default_model!;
    return this.chatProvider() === 'claude-code' ? this.serverModel() : '';
  });
  readonly modelLabel = computed(() => this.chatModel() || this.defaultModel() || 'Worker default');
  readonly modelOptions = computed(() => {
    const models = this.catalog()?.models ?? (this.chatProvider() === 'claude-code' ? ['sonnet', 'opus', 'haiku'] : []);
    const search = this.modelSearch().trim().toLowerCase();
    return [...new Set(['', ...(this.defaultModel() ? [this.defaultModel()] : []), ...models])]
      .filter(model => !model || model.toLowerCase().includes(search));
  });
  readonly modelPlaceholder = computed(() =>
    this.chatProvider() === 'opencode' ? 'provider/model' : 'Model ID');
  /** Whether the model selector dropdown is open. */
  readonly modelSelectorOpen = signal(false);
  /** Whether the chat panel is in full-screen mode. */
  readonly fullscreen = signal(localStorage.getItem('diraigent-chat-fullscreen') === 'true');
  /** Whether any orchestra agent has an active WebSocket connection to the API. */
  readonly orchestraConnected = signal(true); // assume connected initially to avoid flicker

  private abortController: AbortController | null = null;
  private generation = 0;
  private configPollTimer: ReturnType<typeof setInterval> | null = null;

  constructor() {
    this.fetchChatModel();
    effect(() => {
      const pid = this.project.projectId();
      const provider = this.chatProvider();
      const context = `${pid}:${provider}`;
      if (context === this.modelContext) return;
      this.modelContext = context;
      const stored = localStorage.getItem(`${MODEL_STORAGE_KEY}:${pid}:${provider}`) || '';
      untracked(() => {
        this.chatModel.set(this.isValidModel(stored) ? stored : '');
        this.modelSelectorOpen.set(false);
        this.catalog.set(null);
        this.modelSearch.set('');
        void this.loadModels();
      });
    });
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
        this.thinkingText.set('');
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
    this.thinkingText.set('');
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
      if (selectedModel && this.catalog()) body['agent_id'] = this.catalog()!.agent_id;

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

          case 'thinking':
            this.thinkingText.update(t => t + (data['content'] as string));
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
            this.thinkingText.set('');
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
        this.thinkingText.set('');
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
    const selected = model.trim();
    if (selected && !this.isValidModel(selected)) return;
    this.chatModel.set(selected);
    localStorage.setItem(`${MODEL_STORAGE_KEY}:${this.project.projectId()}:${this.chatProvider()}`, selected);
    this.modelSelectorOpen.set(false);
  }

  isValidModel(model: string): boolean {
    if (!model.trim()) return false;
    return this.chatProvider() === 'opencode'
      ? /^[^/\s]+\/\S+$/.test(model.trim())
      : !/\s/.test(model.trim());
  }

  toggleModelSelector(): void {
    this.modelSelectorOpen.update(v => !v);
    if (this.modelSelectorOpen()) {
      this.modelSearch.set('');
      if (!this.catalog() && !this.modelsLoading()) void this.loadModels();
    }
  }

  async loadModels(refresh = false): Promise<void> {
    const pid = this.project.projectId();
    const provider = this.chatProvider();
    const generation = ++this.modelGeneration;
    this.modelRequest?.abort();
    this.modelRequest = new AbortController();
    this.modelsError.set(null);
    this.modelsLoading.set(!!pid);
    if (!pid) return;
    try {
      const token = this.auth.getAccessToken();
      const response = await fetch(`${environment.apiServer}/${pid}/chat/models?refresh=${refresh}`, {
        headers: token ? { Authorization: `Bearer ${token}` } : {},
        signal: this.modelRequest.signal,
      });
      if (!response.ok) throw new Error('Model list unavailable. Refresh or enter a model manually.');
      const catalog: ChatModelCatalog = await response.json();
      if (generation !== this.modelGeneration) return;
      if (catalog.provider !== provider || !Array.isArray(catalog.models) || typeof catalog.agent_id !== 'string') {
        throw new Error('Chat provider changed. Reload the project to refresh its models.');
      }
      catalog.models = catalog.models.filter(model => typeof model === 'string' && this.isValidModel(model));
      this.catalog.set(catalog);
    } catch (error) {
      if (generation !== this.modelGeneration) return;
      this.catalog.set(null);
      this.modelsError.set(error instanceof Error ? error.message : 'Model list unavailable.');
    } finally {
      if (generation === this.modelGeneration) this.modelsLoading.set(false);
    }
  }

  toggleFullscreen(): void {
    this.fullscreen.update(v => !v);
    localStorage.setItem('diraigent-chat-fullscreen', String(this.fullscreen()));
    // Entering fullscreen should always expand collapsed chat
    if (this.fullscreen() && this.collapsed()) {
      this.collapsed.set(false);
      localStorage.setItem('diraigent-chat-collapsed', 'false');
    }
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
    this.thinkingText.set('');
    this.activeTools.set([]);
    this.toolsCompleted.set(0);
    this.error.set(null);
    this.abortController?.abort();
    this.abortController = null;
  }

  private async fetchChatModel(): Promise<void> {
    await this.fetchConfig();
    // Poll config every 30s to keep ws_connected status current
    this.configPollTimer = setInterval(() => this.fetchConfig(), 30_000);
  }

  private async fetchConfig(): Promise<void> {
    try {
      const res = await fetch(`${environment.apiServer}/config`);
      if (!res.ok) return;
      const data = await res.json();
      if (typeof data.chat_model === 'string') this.serverModel.set(data.chat_model);
      if (typeof data.ws_connected === 'boolean') {
        this.orchestraConnected.set(data.ws_connected);
      }
    } catch {
      // Config endpoint unavailable — assume disconnected
      this.orchestraConnected.set(false);
    }
  }
}
