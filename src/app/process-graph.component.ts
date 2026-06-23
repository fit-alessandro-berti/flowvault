import { Component, Input } from '@angular/core';
import { ProcessGraph, ProcessGraphEdge, ProcessGraphNode } from './ocel-wasm.service';

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

  protected edgeColor(edge: ProcessGraphEdge): string {
    return edge.color || '#31544e';
  }

  protected nodeFill(node: ProcessGraphNode): string {
    if (node.shape === 'ellipse') {
      return node.color || '#31544e';
    }

    if (node.kind === 'state-change') {
      return '#fff0da';
    }

    if (node.kind === 'state-activity') {
      return '#eaf2ff';
    }

    return '#e9f5f1';
  }

  protected nodeFillOpacity(node: ProcessGraphNode): string {
    return node.shape === 'ellipse' ? '0.14' : '1';
  }

  protected nodeStroke(node: ProcessGraphNode): string {
    if (node.shape === 'ellipse') {
      return node.color || '#31544e';
    }

    if (node.kind === 'state-change') {
      return '#b45f1a';
    }

    return node.color || '#9cafaa';
  }

  protected nodeStrokeWidth(node: ProcessGraphNode): number {
    return node.kind === 'state-change' || node.shape === 'ellipse' ? 2 : 1;
  }
}
