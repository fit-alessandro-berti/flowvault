import { Component, input } from '@angular/core';
import { FilterDialogBodyComponent } from './filter-dialog-body.component';

@Component({
  standalone: true,
  selector: 'app-filter-modal',
  imports: [FilterDialogBodyComponent],
  templateUrl: './filter-modal.component.html',
})
export class FilterModalComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
