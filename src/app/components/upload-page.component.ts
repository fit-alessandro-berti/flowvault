import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-upload-page',
  templateUrl: './upload-page.component.html',
})
export class UploadPageComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}
