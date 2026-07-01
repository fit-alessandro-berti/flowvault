import type { StateDetectionColorOption } from '../ocel-wasm.service';
import type { CausalOperation } from '../models/causal.models';
import type { StaticSampleLog } from '../models/pattern.models';

export const DEFAULT_STATE_DETECTION_COLOR_OPTIONS: StateDetectionColorOption[] = [
  {
    id: '__window_count',
    label: 'Assigned windows',
    kind: 'count',
  },
];

export const STATIC_SAMPLE_LOGS: StaticSampleLog[] = [
  {
    label: 'Purchase-to-Pay JSON',
    detail: 'Small example log',
    fileName: 'ocel20_example.json.gz',
    path: 'static/ocel2_compressed/ocel20_example.json.gz',
  },
  {
    label: 'Purchase-to-Pay XML',
    detail: 'Small example log',
    fileName: 'ocel20_example.xml.gz',
    path: 'static/ocel2_compressed/ocel20_example.xml.gz',
  },
  {
    label: 'Order Management JSON',
    detail: 'Order, item, and delivery flow',
    fileName: 'order-management.json.gz',
    path: 'static/ocel2_compressed/order-management.json.gz',
  },
  {
    label: 'Order Management XML',
    detail: 'Order, item, and delivery flow',
    fileName: 'order-management.xml.gz',
    path: 'static/ocel2_compressed/order-management.xml.gz',
  },
  {
    label: 'Container Logistics JSON',
    detail: 'Shipment and warehouse flow',
    fileName: 'container_logistics.json.gz',
    path: 'static/ocel2_compressed/container_logistics.json.gz',
  },
  {
    label: 'Container Logistics XML',
    detail: 'Shipment and warehouse flow',
    fileName: 'container_logistics.xml.gz',
    path: 'static/ocel2_compressed/container_logistics.xml.gz',
  },
  {
    label: 'Inventory Simulation JSON',
    detail: 'Stock and replenishment flow',
    fileName: 'inventory_management_simulated.json.gz',
    path: 'static/ocel2_compressed/inventory_management_simulated.json.gz',
  },
  {
    label: 'Inventory Simulation XML',
    detail: 'Stock and replenishment flow',
    fileName: 'inventory_management_simulated.xml.gz',
    path: 'static/ocel2_compressed/inventory_management_simulated.xml.gz',
  },
];

export const CAUSAL_OPERATIONS: Array<{ id: CausalOperation; label: string }> = [
  { id: 'identity', label: 'Identity' },
  { id: 'log10', label: 'log_10' },
  { id: 'log_e', label: 'log_e' },
  { id: 'sqrt', label: 'sqrt' },
];
