# Flowvault

Flowvault is a mixed Angular and Rust/WebAssembly OCEL 2.0 inspector. It imports OCEL 2.0 JSON/XML files in the browser, stores them in a compact Rust data structure, shows event/object/E2O/O2O counts, and exports the loaded log back to OCEL 2.0 JSON or XML.

The implementation follows the official OCEL 2.0 JSON and XML format shape: top-level event type, object type, event, and object collections; scalar attributes; ISO 8601 timestamps; Event-to-Object relationships; and Object-to-Object relationships.

## Project Layout

```text
src/                    Angular application
src/app/ocel-wasm...    Runtime WebAssembly loader and TypeScript API
rust/ocel_wasm/         Rust OCEL parser/exporter compiled with wasm-pack
files/ocel2/            Example OCEL 2.0 JSON/XML files
public/wasm/            Generated wasm-pack output during builds
dist/flowvault/browser/ Static production build output
```

## Compact OCEL Representation

The Rust core parses standard OCEL 2.0 files into a memory-oriented representation:

- repeated strings are interned once in a string pool;
- event IDs, object IDs, type names, attribute names, and qualifiers are stored as integer symbols;
- event timestamps and object attribute timestamps are stored as Unix timestamps in milliseconds;
- attribute values are stored as typed values: string symbol, timestamp, integer, float, or boolean;
- relationships store object references as symbols rather than repeated strings;
- every object owns a timestamp-ordered lifecycle index of related event positions.

This keeps large logs more compact in memory while preserving standard JSON/XML export.

## Supported Import/Export

- JSON extensions: `.json`, `.jsonocel`
- XML extensions: `.xml`, `.xmlocel`
- Attribute types: `string`, `time`, `integer`, `float`, `boolean`
- XML boolean values are exported as `1` or `0`
- ISO 8601 timestamps with offsets are normalized to UTC on export
- ISO 8601 timestamps without offsets are treated as UTC
- XML relationships accept `qualifier` and the older/example `relationship` attribute name on import; export uses `qualifier`

The importer validates duplicate IDs/types, declared attribute types, unknown event/object types, unknown relationship targets, scalar JSON attributes, and timestamp parsing.

## State-Aware Event Enrichment

Flowvault implements the state-aware OCEL idea from Kretzschmann, Berti, and van der Aalst by adding a derived string event attribute named `state`. The paper models states as discrete values derived from dynamic object attributes, then enriches events with state context and transition information. Flowvault focuses on the event enrichment part: a SQL-like query is evaluated against each event and its related objects, then the resulting state is exported as a normal OCEL 2.0 event attribute.

The state query syntax is:

```sql
STATE state AS CASE
  WHEN object.status IS NOT NULL THEN object.status
  WHEN object.state IS NOT NULL THEN object.state
  WHEN object.is_blocked = 'Yes' THEN 'Blocked'
  WHEN event.type LIKE '%cancel%' THEN 'Exception'
  ELSE 'Normal'
END
```

Supported fields:

- `event.id`, `event.type`, `event.time`, and `event.<attribute>`
- `object.id`, `object.type`, and `object.<attribute>` for objects related to the event

Object attributes are resolved at the event timestamp using the latest object attribute value at or before that time. A condition containing object fields is true if any related object satisfies the full condition. The result after `THEN` or `ELSE` can be a string/number/boolean literal or a field reference such as `object.status`.

Supported predicates:

- comparisons: `=`, `!=`, `<>`, `<`, `<=`, `>`, `>=`
- `LIKE` with `%` wildcards
- `IS NULL` and `IS NOT NULL`
- boolean composition with `AND`, `OR`, `NOT`, and parentheses

Reapplying a query replaces the existing `state` attribute on every assigned event. The exporter also adds the `state` attribute definition to every event type so the generated JSON/XML remains self-describing.

The UI opens the state query editor as an overlay after import. For the bundled fixture logs it proposes three named presets on the left:

- `ocel20_example`: Payment Block Status, Purchase Size, Actor and Automation
- `container_logistics`: Shipment Status, Load Planning, Process Phase
- `order-management`: Fulfillment Stage, Value and Weight, Exception Risk
- `inventory_management_simulated`: Stock Status, Activity Phase, Inventory Risk Band

Selecting a preset writes its query into the editor on the right. `OK` applies the edited query to the in-memory log; `Cancel` closes the overlay without changing the imported log.

## State Pattern Detection

After a state query has been applied, Flowvault runs the pattern detection core in Rust/WebAssembly. It follows the state-determined segmentation described by Kretzschmann, Berti, and van der Aalst:

- each object lifecycle is treated as a candidate leading object lifecycle;
- consecutive lifecycle events with the same event `state` form an intra-state episode;
- neighboring episodes with different states form an inter-state transition segment;
- each segment is represented as a small graph with directly-follows control-flow edges, event-to-object-type edges, and leading-object-type context edges;
- structurally equal segment graphs are grouped and ranked by descending support, then by control-flow mass.

The browser UI shows separate intra-state and inter-state sections after state enrichment. Each section has a frequency-sorted pattern dropdown and a `Text`/`Graph` view switch. The graphical view uses deterministic native SVG layout instead of a graph dependency, because the WASM API already returns the compact graph model needed by the page.

The WASM-facing method is `statePatternsJson()`, which returns:

```json
{
  "intra": [{ "support": 5, "sequence": ["START Normal", "..."] }],
  "inter": [{ "support": 3, "from_state": "Normal", "to_state": "Understock" }]
}
```

Calling pattern detection before applying a state query returns an error.

## Commands

Install JavaScript dependencies:

```bash
npm install
```

Run all tests:

```bash
npm test
```

Build the Rust WebAssembly package only:

```bash
npm run build:wasm
```

Build the Apache-ready static application:

```bash
npm run build
```

Start the local Angular dev server:

```bash
npm start
```

The npm scripts set `CARGO_HOME=$PWD/.cargo-home` so Cargo does not need to write to a global system cache.

## Testing

`npm test` runs:

- Rust unit tests in `rust/ocel_wasm`, including JSON/XML imports, relationship counts, object lifecycles, timestamp conversion, validation errors, state enrichment, state pattern detection, and JSON/XML round trips;
- Angular unit tests for the app shell, file helper behavior, state preset dialog, and state pattern text/graph rendering.

The bundled examples in `files/ocel2/` are used by the Rust tests.

## Deployment

After `npm run build`, serve the contents of:

```text
dist/flowvault/browser/
```

with Apache or any static web server. The build uses a relative `<base href="./">`, so it can be served from a root path or a subdirectory.

Apache should serve WebAssembly with the correct MIME type:

```apache
AddType application/wasm .wasm
```

No server-side API is required. The OCEL file is parsed locally in the browser by WebAssembly.

## References

- OCEL 2.0 JSON format: https://www.ocel-standard.org/specification/formats/json/
- OCEL 2.0 XML format: https://www.ocel-standard.org/specification/formats/xml/
- OCEL 2.0 specification: https://www.ocel-standard.org/2.0/ocel20_specification.pdf
- State-Aware Object-Centric Process Mining: Enhancing OCEL 2.0 with Explicit State Transitions: https://www.alessandroberti.it/new_papers/2025_Dina_SAOCPM.pdf
