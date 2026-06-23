import { presetsForFile, STATE_QUERY_PRESETS } from './state-query-presets';

describe('state query presets', () => {
  it('provides three presets for each fixture log', () => {
    expect(presetsForFile('ocel20_example.json').length).toBe(3);
    expect(presetsForFile('container_logistics.xml').length).toBe(3);
    expect(presetsForFile('order-management.json').length).toBe(3);
    expect(presetsForFile('inventory_management_simulated.xml').length).toBe(3);
    expect(presetsForFile('order-management.json.gz').length).toBe(3);
    expect(presetsForFile('inventory_management_simulated.xml.gz').length).toBe(3);
  });

  it('uses named SQL-like state queries', () => {
    for (const preset of STATE_QUERY_PRESETS) {
      expect(preset.name.length).toBeGreaterThan(4);
      expect(preset.leadingObjectType.length).toBeGreaterThan(0);
      expect(preset.query).toContain(
        `STATE state FOR LEADING OBJECT TYPE '${preset.leadingObjectType}' AS CASE`,
      );
      expect(preset.query).toContain('END');
    }
  });
});
