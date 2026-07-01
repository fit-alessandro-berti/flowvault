# Flowvault

Flowvault is a mixed Angular and Rust/WebAssembly OCEL 2.0 inspector. It imports OCEL 2.0 JSON/XML files in the browser, including gzip-compressed `.json.gz` and `.xml.gz` files, stores them in a compact Rust data structure, shows event/object/E2O/O2O counts, and exports the loaded log back to OCEL 2.0 JSON or XML.

The implementation follows the official OCEL 2.0 JSON and XML format shape: top-level event type, object type, event, and object collections; scalar attributes; ISO 8601 timestamps; Event-to-Object relationships; and Object-to-Object relationships.

## Project Layout

```text
src/                     Angular application
src/app/ocel-wasm...     Runtime WebAssembly loader and TypeScript API
src/landing/             Static root redirect page copied after production builds
rust/Cargo.toml          Rust workspace
rust/ocel_core/          Rust OCEL parser/exporter/filtering/analysis core
rust/ocel_core/tests/    Rust integration tests for the public core API
rust/ocel_wasm/          Thin wasm-bindgen adapter compiled with wasm-pack
files/ocel2/             Example OCEL 2.0 JSON/XML files
public/wasm/             Generated wasm-pack output during builds
dist/flowvault/browser/  Static production build output
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

The WebAssembly document keeps two compact logs in memory after import: the original log and the active log. Activity and object-type filters rebuild the active log from the original log, so changing a filter also clears derived event states and state-pattern results.

## Supported Import/Export

- JSON extensions: `.json`, `.jsonocel`
- XML extensions: `.xml`, `.xmlocel`
- Attribute types: `string`, `time`, `integer`, `float`, `boolean`
- XML boolean values are exported as `1` or `0`
- ISO 8601 timestamps with offsets are normalized to UTC on export
- ISO 8601 timestamps without offsets are treated as UTC
- XML relationships accept `qualifier` and the older/example `relationship` attribute name on import; export uses `qualifier`

The importer validates duplicate IDs/types, declared attribute types, unknown event/object types, unknown relationship targets, scalar JSON attributes, and timestamp parsing.

## Activity and Object-Type Filtering

The first screen is intentionally minimal: it asks for an OCEL 2.0 JSON/XML upload, either plain text or gzip-compressed, and also offers bundled compressed sample logs served from `public/static/ocel2_compressed`. After importing a log, Flowvault switches to a workspace with a persistent black toolbar and a left-side feature selector. The toolbar contains import/export/state actions and a filter menu for activities and object types. Selecting a subset filters the active OCEL log in memory while the original imported log remains available for comparison. When filters are active, the toolbar shows the number of filters; opening it reveals the filter chain and removal controls. The `Statistics` page shows plain numbers when no filter is active, and `filtered/original` fractions when any activity or object-type filter is active.

Filtered exports use the active filtered log. Changing any filter resets derived state enrichment and pattern analysis because those results belong to the previous active log.

## State-Aware Event Enrichment

Flowvault implements the state-aware OCEL idea from Kretzschmann, Berti, and van der Aalst by adding a derived string event attribute named `state`. The paper models states as discrete values derived from dynamic object attributes, then enriches events with state context and transition information. Flowvault focuses on the event enrichment part: a SQL-like query is evaluated against each event and its related objects, then the resulting state is exported as a normal OCEL 2.0 event attribute.

The state query syntax is:

```sql
STATE state FOR LEADING OBJECT TYPE 'Order' AS CASE
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

The `FOR LEADING OBJECT TYPE` clause selects the object lifecycle basis for the state notion. Only events related to at least one object of that type receive the derived state, and `object.*` fields are evaluated only against related objects of that leading type. Object attributes are resolved at the event timestamp using the latest object attribute value at or before that time. A condition containing object fields is true if any related leading object satisfies the full condition. The result after `THEN` or `ELSE` can be a string/number/boolean literal or a field reference such as `object.status`.

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
- `inventory_management_simulated`: Stock Status, Activity Phase, Stock Movement

Selecting a preset writes its query into the editor on the right and selects its leading object type. The leading object type can be changed from the dropdown above the editor; the query header is updated accordingly. `OK` applies the edited query to the in-memory log; `Cancel` closes the overlay without changing the imported log.

## State Pattern Detection

State-based feature pages are visible in the left selector but disabled until a state query has been applied. After state enrichment, Flowvault runs the pattern detection core in Rust/WebAssembly. It follows the state-determined segmentation described by Kretzschmann, Berti, and van der Aalst:

- each object of the selected leading object type is treated as a candidate leading object lifecycle;
- consecutive lifecycle events with the same event `state` form an intra-state episode;
- neighboring episodes with different states form an inter-state transition segment;
- each segment is represented as a small graph with directly-follows control-flow edges, event-to-object-type edges, and leading-object-type context edges;
- structurally equal segment graphs are grouped and ranked by descending support, then by control-flow mass.

The `Patterns` page shows intra-state and inter-state tabs after state enrichment. Each tab has a frequency-sorted pattern dropdown and a `Text`/`Graph` view switch. The graphical view uses deterministic native SVG layout instead of a graph dependency, because the WASM API already returns the compact graph model needed by the page.

In the graph view, directly-follows and event-to-object-type edges are drawn with direction. Object-object context edges are drawn as undirected type links because they summarize co-participating object types in the segment, not a causal order.

The WASM-facing method is `statePatternsJson()`, which returns:

```json
{
  "intra": [{ "support": 5, "sequence": ["START Normal", "..."] }],
  "inter": [{ "support": 3, "from_state": "Normal", "to_state": "Understock" }]
}
```

Calling pattern detection before applying a state query returns an error.

## State Detection

The `General > State Detection` page implements an unsupervised execution-state abstraction for one selected object type. Flowvault first builds a numerical object-level feature table:

- activity-count columns, such as how many times `Confirm Order` appears in each selected object's lifecycle;
- distinct related-object counts per object type, based on lifecycle co-participation and object-object links;
- numerical object attributes using the latest value available for the object;
- one-hot columns for categorical object attributes with fewer than 50 distinct latest values.

The feature table preview shows the first 15 objects, and the full feature table can be downloaded as CSV from the table header. For state abstraction, Flowvault encodes sliding lifecycle windows with the same numerical feature space, applies a deterministic two-component PCA implementation, trains a deterministic self-organizing map over the PCA coordinates, and treats SOM cells as discovered execution states. Consecutive windows of the same object that move to a different cell are shown as state transitions; nearby transitions are highlighted in the transition list.

SOM cells can be colored by assigned-window density or by a selected object attribute. Numerical attributes color cells by average value; categorical attributes color cells by dominant category count. Clicking a SOM cell opens a full-screen detail overlay with a directly-follows graph for the selected object type and two boundary-condition tabs: windows entering the cell and windows exiting the cell.

## Directly-Follows Graphs

Flowvault exposes three layout-ready graph computations from the Rust/WebAssembly core:

- `directlyFollowsGraphJson(objectType)`: flattens the active OCEL over one object type and counts directly-follows activity pairs along those object lifecycles.
- `objectCentricDirectlyFollowsGraphJson()`: flattens over every object type, creates separate typed edges for each object type, and adds typed `START`/`END` lifecycle nodes.
- `stateAwareObjectCentricDirectlyFollowsGraphJson()`: uses the enriched event `state` attribute, labels activities as `Activity [State]`, inserts explicit `CHANGE previous -> next` transition nodes when consecutive stateful lifecycle events change state, and keeps the same typed OC-DFG start/end and edge semantics.

Each method returns a shared `ProcessGraph` JSON shape with positioned nodes, curved routed edge paths, labels, weights, object-type colors, and node shape metadata. OC-DFG start/end nodes are rendered as ellipses; typed edges use the same color as their object type, so parallel edges of different object types between the same activities remain visually distinct. The layout uses wider layer and row spacing so larger directly-follows graphs read as left-to-right flows rather than dense grids. The Angular `app-process-graph` component renders that shape as SVG and embeds a left-side panel for choosing visualized object types and minimum activity/path frequencies. Draft changes are applied only when the `Apply` button is pressed. The `Object-Centric DFG` feature shows the traditional OCDFG, and the `State-Aware OC-DFG` feature becomes available after state enrichment.

For a DOT-compatible WebAssembly renderer, the strongest fit is Graphviz compiled to WASM. The `@hpcc-js/wasm` project packages Graphviz and exposes `Graphviz.load().dot(...)`; Viz.js similarly provides `@viz-js/viz` as a WebAssembly Graphviz wrapper. Flowvault does not currently add either package because the graph DTOs are already computed in the Rust/WASM core and rendered directly as SVG, avoiding an extra WASM download and a DOT-to-SVG string pipeline.

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

- Rust workspace tests, including integration tests in `rust/ocel_core/tests` for JSON/XML imports, relationship counts, object lifecycles, validation errors, state enrichment, filtering, graph endpoints, execution-state detection, causal feature tables, compressed imports, fixture imports, and JSON/XML round trips;
- Angular unit tests for the app shell, file helper behavior, state preset dialog, state pattern text/graph rendering, and the State Detection page.

The bundled examples in `files/ocel2/` are used by the Rust tests.

## Deployment

After `npm run build`, serve the contents of:

```text
dist/flowvault/
```

with Apache or any static web server. The build writes a small `dist/flowvault/index.html` landing page for State-Aware Object-Centric Process Mining with an `EXPLORE` button that opens `browser/index.html`, where the Angular application lives. The Angular build uses a relative `<base href="./">`, so it can be served from a root path or a subdirectory.

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
