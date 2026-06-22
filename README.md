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

- Rust unit tests in `rust/ocel_wasm`, including JSON/XML imports, relationship counts, object lifecycles, timestamp conversion, validation errors, and JSON/XML round trips;
- Angular unit tests for the app shell and file helper behavior.

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
