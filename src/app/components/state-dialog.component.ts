import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-state-dialog',
  templateUrl: './state-dialog.component.html',
})
export class StateDialogComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
