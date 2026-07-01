import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-causal-feature-table',
  templateUrl: './causal-feature-table.component.html',
})
export class CausalFeatureTableComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
