import { Component, effect, inject, input, signal, untracked } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { FormsModule } from '@angular/forms';
import { firstValueFrom } from 'rxjs';
import { environment } from '../../../environments/environment';
import { SpAgent } from '../../core/services/agents-api.service';

@Component({
  selector: 'app-project-storage', standalone: true, imports: [FormsModule],
  template: `
    @if (manager()) {
      <section class="my-6 rounded-lg border border-border p-4">
        <h2 class="text-lg font-semibold">Project content storage</h2>
        <p class="text-sm text-text-secondary my-2">{{ owner() ? 'New logs, diffs, artifacts, and personal chat history are stored on Orchestra.' : 'Project content is stored by the API. Chat history stays on this device.' }}</p>
        @if (owner()) {
          <p class="text-sm">Storage owner: {{ ownerName() }}</p>
          <p class="text-sm text-text-secondary my-2">Detailed content needs this Orchestra online. Back up its data directory before moving existing content.</p>
          <button (click)="migrate()" [disabled]="busy()" class="rounded bg-accent px-3 py-2 text-white">Move existing content to Orchestra</button>
        } @else {
          <p class="text-sm text-text-secondary my-2">Choose an updated Orchestra with persistent storage. New content will be stored there; other workers can still execute tasks. Previous device chat histories are not imported.</p>
          <label class="block text-sm my-2">Storage owner
            <select [(ngModel)]="selected" [disabled]="busy()" aria-label="Storage owner" class="ml-2 border border-border rounded bg-surface p-2">
              <option value="">Choose Orchestra</option>
              @for (agent of candidates(); track agent.id) { <option [value]="agent.id">{{ agent.name }}</option> }
            </select>
          </label>
          @if (!candidates().length) { <p class="text-sm text-text-secondary my-2">Update Orchestra first to enable project content storage.</p> }
          <button (click)="assign()" [disabled]="busy() || !selected" class="rounded bg-accent px-3 py-2 text-white">Use Orchestra storage</button>
        }
        @if (message()) { <p role="status" class="text-sm mt-2">{{ message() }}</p> }
        @if (error()) { <p role="alert" class="text-ctp-red mt-2">{{ error() }}</p> }
      </section>
    }
  `,
})
export class ProjectStorage {
  projectId = input.required<string>();
  private http = inject(HttpClient);
  manager = signal(false);
  owner = signal<string | null>(null);
  agents = signal<SpAgent[]>([]);
  busy = signal(false);
  error = signal('');
  message = signal('');
  selected = '';
  candidates() { return this.agents().filter(a => a.status !== 'revoked' && a.metadata?.['content_protocol'] === 1); }
  ownerName() { return this.agents().find(a => a.id === this.owner())?.name ?? this.owner(); }
  private url(id: string) { return `${environment.apiServer}/${id}/storage`; }
  constructor() {
    effect(() => {
      const id = this.projectId();
      untracked(() => {
        this.manager.set(false); this.owner.set(null); this.agents.set([]); this.error.set(''); this.message.set(''); this.selected = '';
        void this.load(id);
      });
    });
  }
  private async load(id: string) {
    try {
      const access = await firstValueFrom(this.http.get<{ role: string }>(`${environment.apiServer}/${id}/people/me`));
      if (id !== this.projectId() || access.role !== 'manager') return;
      this.manager.set(true);
      const [storage, agents] = await Promise.all([
        firstValueFrom(this.http.get<{ agent_id: string | null }>(this.url(id))),
        firstValueFrom(this.http.get<SpAgent[]>(`${environment.apiServer}/agents`)),
      ]);
      if (id !== this.projectId()) return;
      this.owner.set(storage.agent_id); this.agents.set(agents);
    } catch { if (id === this.projectId()) this.error.set('Could not load project storage settings.'); }
  }
  async assign() {
    const id = this.projectId(); this.busy.set(true); this.error.set('');
    try {
      await firstValueFrom(this.http.put(this.url(id), { agent_id: this.selected }));
      await this.load(id);
      if (id === this.projectId()) this.message.set('New content will be stored on Orchestra. Reload the app to load its personal chat history.');
    } catch { if (id === this.projectId()) this.error.set('Could not enable storage. Check that the selected Orchestra is updated and online.'); }
    finally { this.busy.set(false); }
  }
  async migrate() {
    const id = this.projectId(); this.busy.set(true); this.error.set(''); let total = 0;
    try {
      while (id === this.projectId()) {
        const batch = await firstValueFrom(this.http.post<{ moved: number }>(this.url(id) + '/migrate', {}));
        total += batch.moved;
        if (id !== this.projectId()) return;
        this.message.set(`Moved ${total} records from the API to Orchestra.`);
        if (batch.moved === 0) { this.message.set(`Migration complete. Moved ${total} records in this run.`); break; }
      }
    } catch { if (id === this.projectId()) this.error.set('Migration paused. Verified records remain on Orchestra; retry to continue.'); }
    finally { this.busy.set(false); }
  }
}
