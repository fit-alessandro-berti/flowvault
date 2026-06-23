import { Component, Input } from '@angular/core';
import { ProcessGraph, ProcessGraphEdge } from './ocel-wasm.service';

@Component({
  selector: 'app-process-graph',
  imports: [],
  templateUrl: './process-graph.component.html',
  styleUrl: './process-graph.component.css',
})
export class ProcessGraphComponent {
  @Input({ required: true }) graph!: ProcessGraph;

  protected markerId(edge: ProcessGraphEdge): string {
    return `arrow-${edge.id}`;
  }

  protected edgeStrokeWidth(edge: ProcessGraphEdge): number {
    return Math.min(5.5, 1.25 + Math.log2(edge.weight + 1));
  }
}
