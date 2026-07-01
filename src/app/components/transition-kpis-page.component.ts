import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-transition-kpis-page',
  templateUrl: './transition-kpis-page.component.html',
})
export class TransitionKpisPageComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
