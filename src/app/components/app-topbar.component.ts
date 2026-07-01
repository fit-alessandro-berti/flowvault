import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-topbar',
  templateUrl: './app-topbar.component.html',
})
export class AppTopbarComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
