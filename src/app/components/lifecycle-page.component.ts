import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-lifecycle-page',
  templateUrl: './lifecycle-page.component.html',
})
export class LifecyclePageComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
