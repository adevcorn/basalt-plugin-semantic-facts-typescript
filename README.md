# basalt-plugin-semantic-facts-typescript

Semantic facts provider for the ECMAScript family: TypeScript, TSX,
JavaScript, and JSX.

Consumes parser-derived information to provide high-level semantic facts
such as Call Hierarchy, Symbol Declarations, Import edges, and Async boundaries.

The grammar domain is selected from the request path:
- `.ts`, `.mts`, `.cts` → `parse.*@ts/v1`
- `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` → `parse.*@tsx/v1`

If the host does not register the TSX capabilities (older core), the plugin
retries the TypeScript domain.

## Provides
- `semantic-facts@ts/v1`

## Requires
- `parse.call-sites@ts/v1`
- `parse.retrieval@ts/v1`

## Future nice-to-haves
- `parse.type-refs@ts/v1` (optional) -- for import/type-import cross-package edges
- React component prop-types facts
- `Extends`/`Implements` edges from class declarations