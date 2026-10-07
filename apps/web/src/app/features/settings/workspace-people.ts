import { Component, inject, signal } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { FormsModule } from '@angular/forms';
import { TenantApiService, TenantMember } from '../../core/services/tenant-api.service';
import { environment } from '../../../environments/environment';

@Component({selector:'app-workspace-people',standalone:true,imports:[FormsModule],template:`
  <section class="my-4 rounded-lg border border-border p-4">
    <h2 class="text-lg font-semibold">Workspace people</h2>
    <p class="text-sm text-text-secondary mt-2">Your user ID: <code class="break-all">{{myId()}}</code></p>
    @if(owner()) {
      <p class="text-sm text-text-secondary mt-2">Add a registered user's ID to this workspace, then give them project access below.</p>
      @for(member of members();track member.id) {
        <p class="text-sm my-2 break-all">{{member.user_id}} — {{member.role}}</p>
      }
      <form (ngSubmit)="add()" class="flex flex-wrap gap-2 mt-3">
        <label>User ID <input name="workspace-user" [(ngModel)]="userId" required class="border border-border bg-surface rounded p-2" /></label>
        <label>Workspace role <select name="workspace-role" [(ngModel)]="newRole" class="border border-border bg-surface rounded p-2">
          <option value="member">Member — access assigned projects</option>
          <option value="viewer">Viewer — read-only account</option>
        </select></label>
        <button class="bg-accent rounded px-3 py-2 text-white" type="submit">Add to workspace</button>
      </form>
      @if(error()) { <p role="alert" class="text-ctp-red mt-2">{{error()}}</p> }
    }
  </section>
`})
export class WorkspacePeople {
  private api=inject(TenantApiService);private http=inject(HttpClient);
  myId=signal('');owner=signal(false);members=signal<TenantMember[]>([]);error=signal('');
  userId='';newRole='member';private tenantId='';
  constructor() {this.http.get<{user_id:string}>(`${environment.apiServer}/account`).subscribe({next:a=>{
    this.myId.set(a.user_id);
    this.api.getMyTenant().subscribe({next:t=>{if(t){this.tenantId=t.id;this.load();}}});
  }});}
  private load(){this.api.listMembers(this.tenantId).subscribe({next:m=>{
    this.members.set(m);this.owner.set(m.some(p=>p.user_id===this.myId() && p.role==='owner'));
  }});}
  add(){this.error.set('');this.api.addMember(this.tenantId,this.userId,this.newRole).subscribe({next:()=>this.load(),error:e=>this.error.set(e.error?.error??'Could not add workspace member')});}
}
