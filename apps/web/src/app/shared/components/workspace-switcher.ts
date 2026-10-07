import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { TenantApiService, Tenant } from '../../core/services/tenant-api.service';

@Component({selector:'app-workspace-switcher',standalone:true,imports:[FormsModule],template:`
  @if (workspaces().length > 1) {
    <label for="workspace-selection" class="block text-xs text-text-secondary mb-1">Workspace</label>
    <select id="workspace-selection" [ngModel]="selected()" (ngModelChange)="select($event)"
      class="w-full rounded border border-border bg-surface p-2 text-sm">
      @for(w of workspaces();track w.id) { <option [value]="w.id">{{w.name}}</option> }
    </select>
  }
`})
export class WorkspaceSwitcher {
  private api=inject(TenantApiService);
  workspaces=signal<Tenant[]>([]);
  selected=signal(localStorage.getItem('diraigent-workspace')??'');
  constructor() { this.api.listTenants().subscribe({next:ws=>{
    this.workspaces.set(ws);
    if (this.selected() && !ws.some(w => w.id === this.selected())) {
      // A saved selection may belong to a previous login or revoked membership.
      if (ws.length) this.select(ws[0].id);
      else {
        localStorage.removeItem('diraigent-workspace');
        localStorage.removeItem('diraigent-project');
        window.location.assign('/work');
      }
      return;
    }
    if(!this.selected() && ws.length) this.selected.set(ws[0].id);
  }}); }
  select(id:string) {
    localStorage.setItem('diraigent-workspace',id);
    localStorage.removeItem('diraigent-project');
    window.location.assign('/work');
  }
}
