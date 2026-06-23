export interface StateQueryPreset {
  id: string;
  logKey: string;
  name: string;
  leadingObjectType: string;
  query: string;
}

export const STATE_QUERY_PRESETS: StateQueryPreset[] = [
  {
    id: 'p2p-payment-block',
    logKey: 'ocel20_example',
    name: 'Payment Block Status',
    leadingObjectType: 'Invoice',
    query: `STATE state FOR LEADING OBJECT TYPE 'Invoice' AS CASE
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
    leadingObjectType: 'Purchase Order',
    query: `STATE state FOR LEADING OBJECT TYPE 'Purchase Order' AS CASE
  WHEN object.po_quantity > 500 THEN 'Large PO'
  WHEN object.po_product = 'Notebooks' THEN 'Maverick Buying'
  ELSE 'Standard Purchase'
END`,
  },
  {
    id: 'p2p-actor-risk',
    logKey: 'ocel20_example',
    name: 'Actor and Automation',
    leadingObjectType: 'Invoice',
    query: `STATE state FOR LEADING OBJECT TYPE 'Invoice' AS CASE
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
    leadingObjectType: 'Container',
    query: `STATE state FOR LEADING OBJECT TYPE 'Container' AS CASE
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
    name: 'Load Planning',
    leadingObjectType: 'Customer Order',
    query: `STATE state FOR LEADING OBJECT TYPE 'Customer Order' AS CASE
  WHEN event.type = 'Book Vehicles' THEN 'Vehicle Booking'
  WHEN event.type LIKE '%Load%' THEN 'Transport Loading'
  WHEN object.AmountofGoods >= 900 THEN 'Large Order'
  ELSE 'Standard Load'
END`,
  },
  {
    id: 'container-process-phase',
    logKey: 'container_logistics',
    name: 'Process Phase',
    leadingObjectType: 'Container',
    query: `STATE state FOR LEADING OBJECT TYPE 'Container' AS CASE
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
    leadingObjectType: 'packages',
    query: `STATE state FOR LEADING OBJECT TYPE 'packages' AS CASE
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
    leadingObjectType: 'items',
    query: `STATE state FOR LEADING OBJECT TYPE 'items' AS CASE
  WHEN object.weight >= 10 THEN 'Heavy'
  WHEN object.price >= 1000 THEN 'High Value'
  WHEN object.price >= 250 THEN 'Medium Value'
  ELSE 'Standard'
END`,
  },
  {
    id: 'orders-exception-risk',
    logKey: 'order-management',
    name: 'Exception Risk',
    leadingObjectType: 'orders',
    query: `STATE state FOR LEADING OBJECT TYPE 'orders' AS CASE
  WHEN event.type = 'item out of stock' THEN 'Stock Exception'
  WHEN event.type = 'reorder item' THEN 'Replenishment'
  WHEN event.type = 'payment reminder' THEN 'Payment Risk'
  WHEN event.type = 'failed delivery' THEN 'Delivery Risk'
ELSE 'Nominal'
END`,
  },
  {
    id: 'inventory-stock-status',
    logKey: 'inventory_management_simulated',
    name: 'Stock Status',
    leadingObjectType: 'MAT',
    query: `STATE state FOR LEADING OBJECT TYPE 'MAT' AS CASE
  WHEN event."Stock After" = 0 THEN 'Zero Stock'
  WHEN event."Stock After" < 30 THEN 'Low Stock'
  WHEN event."Stock After" >= 100 THEN 'High Stock'
  ELSE 'Available Stock'
END`,
  },
  {
    id: 'inventory-activity-phase',
    logKey: 'inventory_management_simulated',
    name: 'Activity Phase',
    leadingObjectType: 'MAT',
    query: `STATE state FOR LEADING OBJECT TYPE 'MAT' AS CASE
  WHEN event.type = 'Goods Receipt' THEN 'Goods Receipt'
  WHEN event.type = 'Goods Issue' THEN 'Goods Issue'
  WHEN event.type = 'Create Purchase Order Item' THEN 'Purchase Order'
  WHEN event.type = 'Create Purchase Suggestion Item' THEN 'Purchase Suggestion'
  WHEN event.type = 'Create Sales Order Item' THEN 'Sales Order'
  ELSE 'Inventory Activity'
END`,
  },
  {
    id: 'inventory-stock-movement',
    logKey: 'inventory_management_simulated',
    name: 'Stock Movement',
    leadingObjectType: 'MAT',
    query: `STATE state FOR LEADING OBJECT TYPE 'MAT' AS CASE
  WHEN event."Stock After" > event."Stock Before" THEN 'Stock Increase'
  WHEN event."Stock After" < event."Stock Before" THEN 'Stock Decrease'
  WHEN event."Stock After" = 0 THEN 'Zero Stable'
  ELSE 'No Stock Change'
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
