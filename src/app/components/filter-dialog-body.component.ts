import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-filter-dialog-body',
  templateUrl: './filter-dialog-body.component.html',
})
export class FilterDialogBodyComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
