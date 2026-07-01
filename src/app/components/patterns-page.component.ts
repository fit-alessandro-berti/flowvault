import { Component, input } from '@angular/core';
import { PatternExplorerComponent } from './pattern-explorer.component';
import { IntraPatternPanelComponent } from './intra-pattern-panel.component';
import { InterPatternPanelComponent } from './inter-pattern-panel.component';

@Component({
  standalone: true,
  selector: 'app-patterns-page',
  imports: [PatternExplorerComponent, IntraPatternPanelComponent, InterPatternPanelComponent],
  templateUrl: './patterns-page.component.html',
})
export class PatternsPageComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
