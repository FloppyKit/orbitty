# src/lib/board

Board UI pieces. `KanbanPage.svelte` mounts them.

`Chrome.svelte`, `Columns.svelte`, `PlanetColumn.svelte`, `Preview.svelte`, and `Needed.svelte` are the template children. `StartDrop.svelte` sits beside them.

`pagePlanets.svelte.ts`, `pageScope.svelte.ts`, `pageGrab.svelte.ts`, `pageChips.svelte.ts`, `pageMutate.svelte.ts`, `pageFields.svelte.ts`, and `pagePointer.svelte.ts` hold function bodies moved out of `KanbanPage`. Each takes a scope of the lets the page already had. The page keeps `$state`, `$effect`, `onMount`, and `onDestroy`.
