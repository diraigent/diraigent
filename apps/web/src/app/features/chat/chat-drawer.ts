import { Component, inject, viewChild, ElementRef, effect, untracked, HostListener, Pipe, PipeTransform } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ChatService, CHAT_MODELS } from '../../core/services/chat.service';
import { Marked, type MarkedExtension } from 'marked';
import hljs from 'highlight.js/lib/common';
import DOMPurify from 'dompurify';

/* ── Markdown renderer configuration ─────────────────────── */

const highlightExtension: MarkedExtension = {
  renderer: {
    code({ text, lang }: { text: string; lang?: string }) {
      let highlighted: string;
      try {
        if (lang && hljs.getLanguage(lang)) {
          highlighted = hljs.highlight(text, { language: lang }).value;
        } else {
          highlighted = hljs.highlightAuto(text).value;
        }
      } catch {
        highlighted = text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
      }
      const langLabel = lang ? `<span class="chat-code-lang">${lang}</span>` : '';
      return `<pre class="chat-code-block">${langLabel}<code class="hljs">${highlighted}</code></pre>`;
    },
  },
};

const chatMarked = new Marked(highlightExtension);

function renderMarkdown(content: string): string {
  const raw = chatMarked.parse(content, { async: false }) as string;
  return DOMPurify.sanitize(raw);
}

/* ── Pipe: converts markdown string → sanitised HTML ─────── */

@Pipe({ name: 'chatMarkdown', standalone: true })
export class ChatMarkdownPipe implements PipeTransform {
  transform(value: string): string {
    if (!value) return '';
    return renderMarkdown(value);
  }
}

@Component({
  selector: 'app-chat-drawer',
  standalone: true,
  imports: [FormsModule, ChatMarkdownPipe],
  styles: [`
    :host { display: block; height: 100%; }

    @import 'highlight.js/styles/atom-one-dark.css';

    /* ── Chat markdown prose styles ── */
    .chat-md :is(h1, h2, h3, h4) {
      font-weight: 600;
      margin-top: 0.75rem;
      margin-bottom: 0.25rem;
    }
    .chat-md h1 { font-size: 1.25rem; }
    .chat-md h2 { font-size: 1.125rem; }
    .chat-md h3 { font-size: 1rem; }
    .chat-md p { margin-bottom: 0.5rem; line-height: 1.55; }
    .chat-md p:last-child { margin-bottom: 0; }
    .chat-md ul, .chat-md ol { padding-left: 1.25rem; margin-bottom: 0.5rem; }
    .chat-md ul { list-style-type: disc; }
    .chat-md ol { list-style-type: decimal; }
    .chat-md li { margin-bottom: 0.15rem; }
    .chat-md li > p { margin-bottom: 0.25rem; }

    /* Inline code */
    .chat-md code {
      background: rgba(255, 255, 255, 0.07);
      border-radius: 0.25rem;
      padding: 0.1rem 0.35rem;
      font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
      font-size: 0.85em;
    }

    /* Code blocks */
    .chat-md .chat-code-block {
      position: relative;
      background: rgba(0, 0, 0, 0.25);
      border-radius: 0.5rem;
      padding: 0.75rem 1rem;
      overflow-x: auto;
      margin: 0.5rem 0;
      font-size: 0.8125rem;
      line-height: 1.5;
    }
    .chat-md .chat-code-block code {
      background: transparent;
      padding: 0;
      border-radius: 0;
      font-size: inherit;
    }
    .chat-md .chat-code-block code.hljs {
      background: transparent !important;
      padding: 0 !important;
    }
    .chat-md .chat-code-lang {
      position: absolute;
      top: 0.25rem;
      right: 0.5rem;
      font-size: 0.625rem;
      text-transform: uppercase;
      opacity: 0.4;
      pointer-events: none;
    }

    .chat-md blockquote {
      border-left: 3px solid rgba(255, 255, 255, 0.2);
      padding-left: 0.75rem;
      margin: 0.5rem 0;
      opacity: 0.8;
    }
    .chat-md a {
      color: var(--color-accent, #60a5fa);
      text-decoration: underline;
    }
    .chat-md table { border-collapse: collapse; margin: 0.5rem 0; width: 100%; font-size: 0.8125rem; }
    .chat-md th, .chat-md td { border: 1px solid rgba(255,255,255,0.1); padding: 0.35rem 0.5rem; text-align: left; }
    .chat-md th { font-weight: 600; background: rgba(255,255,255,0.04); }
    .chat-md hr { border-color: rgba(255,255,255,0.1); margin: 0.75rem 0; }
    .chat-md img { max-width: 100%; border-radius: 0.375rem; }
  `],
  template: `
      <div class="flex flex-col bg-surface overflow-hidden"
           [class.h-full]="!chat.collapsed()">

        <!-- Header -->
        <div class="flex items-center justify-between px-4 py-3 border-b border-border bg-bg-subtle">
          <div class="flex items-center gap-2">
            <span class="font-semibold text-text-primary">AI Assistant</span>
            <div class="relative">
              <button (click)="chat.toggleModelSelector(); $event.stopPropagation()"
                      class="text-xs text-text-secondary font-normal hover:text-accent transition-colors
                             flex items-center gap-0.5 cursor-pointer">
                {{ chat.chatModel() || 'model' }}
                <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                </svg>
              </button>
              @if (chat.modelSelectorOpen()) {
                <div class="absolute top-full left-0 mt-1 bg-surface border border-border rounded-lg shadow-lg z-50 py-1 min-w-[120px]">
                  @for (model of models; track model) {
                    <button (click)="chat.setModel(model); $event.stopPropagation()"
                            class="w-full text-left px-3 py-1.5 text-xs transition-colors"
                            [class]="model === chat.chatModel()
                              ? 'text-accent bg-accent/10 font-medium'
                              : 'text-text-secondary hover:text-text-primary hover:bg-bg-subtle'">
                      {{ model }}
                    </button>
                  }
                </div>
              }
            </div>
          </div>
          <div class="flex items-center gap-2">
            @if (!chat.collapsed()) {
              <button (click)="chat.clear()"
                      class="text-xs text-text-secondary hover:text-text-primary transition-colors">
                Clear
              </button>
            }
            @if (!chat.collapsed()) {
              <button (click)="chat.toggleFullscreen()"
                      class="p-1 text-text-secondary hover:text-text-primary transition-colors"
                      [attr.aria-label]="chat.fullscreen() ? 'Exit full screen' : 'Full screen'">
                @if (chat.fullscreen()) {
                  <!-- Minimize icon (arrows inward) -->
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M9 9V4.5M9 9H4.5M9 9L3.5 3.5M9 15v4.5M9 15H4.5M9 15l-5.5 5.5M15 9h4.5M15 9V4.5M15 9l5.5-5.5M15 15h4.5M15 15v4.5m0-4.5l5.5 5.5" />
                  </svg>
                } @else {
                  <!-- Maximize icon (arrows outward) -->
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M3.75 3.75v4.5m0-4.5h4.5m-4.5 0L9 9M3.75 20.25v-4.5m0 4.5h4.5m-4.5 0L9 15M20.25 3.75h-4.5m4.5 0v4.5m0-4.5L15 9M20.25 20.25h-4.5m4.5 0v-4.5m0 4.5L15 15" />
                  </svg>
                }
              </button>
            }
            @if (!chat.fullscreen()) {
              <button (click)="chat.toggleCollapsed()"
                      class="p-1 text-text-secondary hover:text-text-primary transition-colors"
                      [attr.aria-label]="chat.collapsed() ? 'Expand chat' : 'Collapse chat'">
                <svg class="w-4 h-4 transition-transform duration-300"
                     [class.rotate-180]="chat.collapsed()"
                     fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                </svg>
              </button>
            }
          </div>
        </div>

        <!-- Collapsible content -->
        <div class="grid min-h-0 transition-[grid-template-rows] duration-300 ease-in-out"
             [style.gridTemplateRows]="chat.collapsed() ? '0fr' : '1fr'">
          <div class="overflow-hidden flex flex-col min-h-0">

            <!-- Messages -->
            <div #messageList class="flex-1 overflow-y-auto min-h-0 p-4" (scroll)="onScroll()">
              <div class="max-w-3xl mx-auto space-y-3">
              @if (!chat.canSend()) {
                <div class="flex flex-col items-center justify-center h-full text-center px-6">
                  <div class="text-text-muted text-sm">
                    <p class="font-medium text-text-secondary mb-1">No project selected</p>
                    <p>Create or select a project to start chatting with the AI assistant.</p>
                  </div>
                </div>
              }
              @for (msg of chat.messages(); track $index) {
                <div [class]="msg.role === 'user' ? 'flex justify-end' : 'flex justify-start'">
                  @if (msg.role === 'user') {
                    <div class="max-w-[85%] rounded-2xl rounded-br-md px-4 py-2 bg-accent text-bg text-sm whitespace-pre-wrap">
                      {{ msg.content }}
                    </div>
                  } @else {
                    <div class="max-w-[85%] rounded-2xl rounded-bl-md px-4 py-2 bg-bg-subtle text-text-primary text-sm chat-md"
                         [innerHTML]="msg.content | chatMarkdown">
                    </div>
                  }
                </div>
              }

              <!-- Streaming text -->
              @if (chat.streaming() && chat.streamingText()) {
                <div class="flex justify-start">
                  <div class="max-w-[85%] rounded-2xl rounded-bl-md px-4 py-2 bg-bg-subtle text-text-primary text-sm chat-md">
                    <span [innerHTML]="chat.streamingText() | chatMarkdown"></span>
                    <span class="inline-block w-1.5 h-4 bg-accent animate-pulse ml-0.5 align-text-bottom"></span>
                  </div>
                </div>
              }

              <!-- Thinking indicator -->
              @if (chat.streaming() && !chat.streamingText() && chat.activeTools().length === 0 && chat.toolsCompleted() === 0) {
                <div class="flex justify-start">
                  @if (chat.thinkingText()) {
                    <details class="max-w-[85%] rounded-2xl rounded-bl-md bg-bg-subtle text-sm group" [attr.open]="thinkingExpanded ? '' : null">
                      <summary (click)="thinkingExpanded = !thinkingExpanded; $event.preventDefault()"
                               class="flex items-center gap-2 px-4 py-2 cursor-pointer select-none text-text-secondary list-none">
                        <svg class="w-3.5 h-3.5 animate-spin text-accent shrink-0" viewBox="0 0 24 24" fill="none">
                          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                        </svg>
                        Thinking...
                        <svg class="w-3 h-3 transition-transform" [class.rotate-90]="thinkingExpanded" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                        </svg>
                      </summary>
                      @if (thinkingExpanded) {
                        <div class="px-4 pb-3 text-text-muted text-xs whitespace-pre-wrap max-h-48 overflow-y-auto border-t border-border/50 pt-2 mt-1">
                          {{ chat.thinkingText() }}
                        </div>
                      }
                    </details>
                  } @else {
                    <div class="rounded-2xl rounded-bl-md px-4 py-2 bg-bg-subtle text-text-secondary text-sm animate-pulse">
                      Thinking...
                    </div>
                  }
                </div>
              }

              <!-- Tool activity indicator (collapsed spinner) -->
              @if (chat.streaming() && (chat.activeTools().length > 0 || chat.toolsCompleted() > 0)) {
                <div class="flex items-center gap-2 text-xs text-text-secondary px-1 py-1">
                  @if (chat.activeTools().length > 0) {
                    <svg class="w-3.5 h-3.5 animate-spin text-accent" viewBox="0 0 24 24" fill="none">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                    </svg>
                    <span>{{ chat.activeTools()[chat.activeTools().length - 1].toolName }}</span>
                  }
                  @if (chat.toolsCompleted() > 0) {
                    <span class="text-text-muted">({{ chat.toolsCompleted() }} tool calls)</span>
                  }
                </div>
              }

              <!-- Error -->
              @if (chat.error()) {
                <div class="text-sm text-ctp-red px-1">{{ chat.error() }}</div>
              }
              </div>
            </div>

            <!-- Input -->
            <div class="border-t border-border p-3 pb-[max(0.75rem,env(safe-area-inset-bottom))]">
              <div class="max-w-3xl mx-auto flex gap-2">
                <textarea
                  #inputEl
                  [(ngModel)]="inputText"
                  (keydown.enter)="onEnter($event)"
                  [placeholder]="chat.canSend() ? 'Ask about your project...' : 'Select a project first'"
                  [disabled]="!chat.canSend()"
                  rows="1"
                  class="flex-1 resize-none rounded-xl border border-border bg-bg-subtle px-3 py-2
                         min-h-[44px] text-sm text-text-primary placeholder:text-text-secondary
                         focus:outline-none focus:ring-1 focus:ring-accent"></textarea>
                @if (chat.streaming()) {
                  <button (click)="chat.cancel()"
                          class="self-end px-3 py-2 rounded-xl bg-ctp-red/10 text-ctp-red text-sm hover:bg-ctp-red/20 transition-colors">
                    Stop
                  </button>
                }
                <button (click)="sendMessage()"
                        [disabled]="!inputText.trim() || !chat.canSend()"
                        class="self-end px-3 py-2 rounded-xl bg-accent text-bg text-sm hover:opacity-90 transition-opacity
                               disabled:opacity-40 disabled:cursor-not-allowed">
                  Send
                </button>
              </div>
            </div>

          </div>
        </div>
      </div>
  `,
})
export class ChatDrawerComponent {
  chat = inject(ChatService);
  inputText = '';
  thinkingExpanded = false;
  readonly models = CHAT_MODELS;

  private messageList = viewChild<ElementRef<HTMLDivElement>>('messageList');

  @HostListener('document:click')
  onDocumentClick(): void {
    if (this.chat.modelSelectorOpen()) {
      this.chat.modelSelectorOpen.set(false);
    }
  }
  /** Set to true when the user manually scrolls up to read history. */
  private userScrolledUp = false;

  constructor() {
    // Auto-scroll whenever messages or streaming content change,
    // unless the user has deliberately scrolled up to read history.
    effect(() => {
      this.chat.messages();
      this.chat.streamingText();
      this.chat.thinkingText();

      untracked(() => {
        if (!this.userScrolledUp) {
          // Defer one microtask so Angular has flushed DOM changes first.
          setTimeout(() => this.scrollToBottom(), 0);
        }
      });
    });
  }

  /**
   * Track manual scrolling so we don't hijack the user while they read history.
   * Resets automatically when they send a new message.
   */
  onScroll(): void {
    const el = this.messageList()?.nativeElement;
    if (!el) return;
    // Consider "at the bottom" if within 80px of the scroll end.
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    this.userScrolledUp = distanceFromBottom > 80;
  }

  onEnter(event: Event): void {
    const ke = event as KeyboardEvent;
    if (ke.shiftKey) return; // allow newlines
    ke.preventDefault();
    this.sendMessage();
  }

  async sendMessage(): Promise<void> {
    const text = this.inputText.trim();
    if (!text) return;
    this.chat.cancel();
    this.inputText = '';
    // Sending a new message always resets scroll tracking.
    this.userScrolledUp = false;
    await this.chat.send(text);
  }

  private scrollToBottom(): void {
    const el = this.messageList()?.nativeElement;
    if (el) el.scrollTop = el.scrollHeight;
  }
}
