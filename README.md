# basalt-plugin-semantic-facts-typescript

Semantic facts provider for TypeScript/TSX.

Consumes parser-derived information to provide high-level semantic facts
such as Call Hierarchy, Symbol Declarations, Import edges, and Async boundaries.

## Provides
- `semantic-facts@ts/v1`

## Requires
- `parse.call-sites@ts/v1`
- `parse.retrieval@ts/v1`

## Future nice-to-haves
- `parse.type-refs@ts/v1` (optional) -- for import/type-import cross-package edges
- React component prop-types facts
- `Extends`/`Implements` edges from class declarations