export interface StateQueryPreset {
  id: string;
  logKey: string;
  name: string;
  query: string;
}

export const STATE_QUERY_PRESETS: StateQueryPreset[] = [
  {
    id: 'p2p-payment-block',
    logKey: 'ocel20_example',
    name: 'Payment Block Status',
    query: `STATE state AS CASE
  WHEN object.is_blocked = 'Yes' THEN 'Invoice Blocked'
  WHEN event.type LIKE '%Payment%' THEN 'Payment Execution'
  WHEN event.type LIKE '%Invoice%' THEN 'Invoice Handling'
  ELSE 'Procurement'
END`,
  },
  {
    id: 'p2p-purchase-size',
    logKey: 'ocel20_example',
    name: 'Purchase Size',
    query: `STATE state AS CASE
  WHEN object.po_quantity > 500 THEN 'Large PO'
  WHEN object.pr_quantity > 500 THEN 'Large Requisition'
  WHEN object.po_product = 'Notebooks' THEN 'Maverick Buying'
  ELSE 'Standard Purchase'
END`,
  },
  {
    id: 'p2p-actor-risk',
    logKey: 'ocel20_example',
    name: 'Actor and Automation',
    query: `STATE state AS CASE
  WHEN event.invoice_blocker IS NOT NULL OR event.invoice_block_rem IS NOT NULL THEN 'Manual Block Control'
  WHEN event.payment_inserter = 'Robot' THEN 'Automated Payment'
  WHEN event.po_creator = 'Mario' OR event.invoice_inserter = 'Mario' THEN 'Maverick Flow'
  ELSE 'Regular Work'
END`,
  },
  {
    id: 'container-shipment-status',
    logKey: 'container_logistics',
    name: 'Shipment Status',
    query: `STATE state AS CASE
  WHEN object.Status = 'shipped' THEN 'Shipped'
  WHEN object.Status = 'in transit' THEN 'In Transit'
  WHEN object.Status = 'full' THEN 'Loaded'
  WHEN object.Status = 'empty' THEN 'Empty'
  ELSE 'Planning'
END`,
  },
  {
    id: 'container-load-size',
    logKey: 'container_logistics',
    name: 'Load Size',
    query: `STATE state AS CASE
  WHEN object.AmountofContainers >= 6 THEN 'Large Transport'
  WHEN object.AmountofHandlingUnits >= 8 THEN 'Dense Container'
  WHEN object.AmountofGoods >= 900 THEN 'Large Order'
  ELSE 'Standard Load'
END`,
  },
  {
    id: 'container-process-phase',
    logKey: 'container_logistics',
    name: 'Process Phase',
    query: `STATE state AS CASE
  WHEN event.type LIKE '%Depart%' OR event.type LIKE '%Drive%' THEN 'Outbound'
  WHEN event.type LIKE '%Load%' OR event.type LIKE '%Weigh%' THEN 'Loading'
  WHEN event.type LIKE '%Order%' OR event.type LIKE '%Create%' OR event.type LIKE '%Book%' THEN 'Planning'
  ELSE 'Warehouse Handling'
END`,
  },
  {
    id: 'orders-fulfillment',
    logKey: 'order-management',
    name: 'Fulfillment Stage',
    query: `STATE state AS CASE
  WHEN event.type = 'failed delivery' THEN 'Delivery Failure'
  WHEN event.type = 'package delivered' THEN 'Delivered'
  WHEN event.type LIKE '%package%' OR event.type = 'send package' THEN 'Packaging'
  WHEN event.type LIKE '%pay%' OR event.type = 'payment reminder' THEN 'Payment'
  ELSE 'Order Handling'
END`,
  },
  {
    id: 'orders-value-weight',
    logKey: 'order-management',
    name: 'Value and Weight',
    query: `STATE state AS CASE
  WHEN object.price >= 1000 THEN 'High Value'
  WHEN object.price >= 250 THEN 'Medium Value'
  WHEN object.weight >= 10 THEN 'Heavy'
  ELSE 'Standard'
END`,
  },
  {
    id: 'orders-exception-risk',
    logKey: 'order-management',
    name: 'Exception Risk',
    query: `STATE state AS CASE
  WHEN event.type = 'item out of stock' THEN 'Stock Exception'
  WHEN event.type = 'reorder item' THEN 'Replenishment'
  WHEN event.type = 'payment reminder' THEN 'Payment Risk'
  WHEN event.type = 'failed delivery' THEN 'Delivery Risk'
  ELSE 'Nominal'
END`,
  },
];

export function presetsForFile(fileName: string): StateQueryPreset[] {
  const key = logKeyForFile(fileName);
  const presets = STATE_QUERY_PRESETS.filter((preset) => preset.logKey === key);

  return presets.length > 0 ? presets : STATE_QUERY_PRESETS.slice(0, 3);
}

function logKeyForFile(fileName: string): string {
  return fileName
    .trim()
    .toLowerCase()
    .replace(/\.(jsonocel|xmlocel|json|xml)$/i, '');
}
