import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-feature-sidebar',
  templateUrl: './feature-sidebar.component.html',
})
export class FeatureSidebarComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
