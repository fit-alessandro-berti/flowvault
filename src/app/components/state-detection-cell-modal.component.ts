import { Component, input } from '@angular/core';
import { ProcessGraphComponent } from '../process-graph.component';

@Component({
  standalone: true,
  selector: 'app-state-detection-cell-modal',
  imports: [ProcessGraphComponent],
  templateUrl: './state-detection-cell-modal.component.html',
})
export class StateDetectionCellModalComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
