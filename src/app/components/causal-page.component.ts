import { Component, input } from '@angular/core';
import { CausalWorkbenchComponent } from './causal-workbench.component';
import { CausalEdgePanelComponent } from './causal-edge-panel.component';
import { CausalFeatureTableComponent } from './causal-feature-table.component';
import { CausalFitPanelComponent } from './causal-fit-panel.component';

@Component({
  standalone: true,
  selector: 'app-causal-page',
  imports: [CausalWorkbenchComponent, CausalEdgePanelComponent, CausalFeatureTableComponent, CausalFitPanelComponent],
  templateUrl: './causal-page.component.html',
})
export class CausalPageComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
