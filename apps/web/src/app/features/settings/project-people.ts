import { Component, effect, inject, input, signal, untracked } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { FormsModule } from '@angular/forms';
import { environment } from '../../../environments/environment';

interface Person { user_id:string; role:string; }
@Component({selector:'app-project-people',standalone:true,imports:[FormsModule],template:`
  @if (role()==='manager') {
    <section class="my-6 rounded-lg border border-border p-4">
      <h2 class="text-lg font-semibold">People with project access</h2>
      <p class="text-sm text-text-secondary my-2">Workspace owners and administrators manage all projects. Other people need an explicit role here.</p>
      @for(person of people();track person.user_id) {
        <div class="flex gap-3 items-center py-2 text-sm">
          <span class="break-all">{{person.user_id}}</span><span>{{person.role}}</span>
          <button (click)="remove(person.user_id)" class="text-ctp-red">Remove access</button>
        </div>
      }
      <form (ngSubmit)="grant()" class="flex flex-wrap gap-2 mt-3">
        <label>User ID <input name="user" [(ngModel)]="userId" required class="border border-border rounded bg-surface p-2" /></label>
        <label>Project role <select name="role" [(ngModel)]="newRole" class="border border-border rounded bg-surface p-2">
          <option value="viewer">Viewer — read only</option><option value="editor">Editor — work on tasks</option><option value="manager">Manager — manage project and access</option>
        </select></label>
        <button type="submit" class="bg-accent rounded px-3 py-2 text-white">Save access</button>
      </form>
      <p class="text-xs text-text-secondary mt-2">Add the user to this workspace first. Their internal user ID is returned by their account API.</p>
      @if(error()) { <p role="alert" class="text-ctp-red mt-2">{{error()}}</p> }
    </section>
  }
`})
export class ProjectPeople {
  projectId=input.required<string>();
  private http=inject(HttpClient);
  role=signal('');people=signal<Person[]>([]);error=signal('');
  userId='';newRole='viewer';
  private base() { return `${environment.apiServer}/${this.projectId()}/people`; }
  constructor() { effect(()=>{
    const id=this.projectId(); untracked(()=>{this.role.set('');this.people.set([]);
    this.http.get<{role:string}>(`${this.base()}/me`).subscribe({next:a=>{
      if(this.projectId()!==id)return;
      this.role.set(a.role); if(a.role==='manager')this.load();
    }});
  }); }); }
  load() {this.http.get<Person[]>(this.base()).subscribe({next:p=>this.people.set(p)});}
  grant() {this.error.set('');this.http.post(this.base(),{user_id:this.userId,role:this.newRole}).subscribe({next:()=>this.load(),error:e=>this.error.set(e.error?.error??'Could not save project access')});}
  remove(id:string) {this.http.delete(`${this.base()}/${id}`).subscribe({next:()=>this.load(),error:()=>this.error.set('Could not remove project access')});}
}
