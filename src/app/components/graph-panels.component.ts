import { Component, input } from '@angular/core';
import { ProcessGraphComponent } from '../process-graph.component';

@Component({
  standalone: true,
  selector: 'app-graph-panels',
  imports: [ProcessGraphComponent],
  templateUrl: './graph-panels.component.html',
})
export class GraphPanelsComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
