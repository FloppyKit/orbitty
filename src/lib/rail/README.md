# src/lib/rail

Rail UI pieces. `RailPage.svelte` mounts `Chrome.svelte`, `List.svelte`, and `ClaimUi.svelte`. `api.ts` is `RailApi`.

`stageArch.ts` keeps arch geometry and `archLayer2Items`, and re-exports what moved to `stagePlace.ts` (preview, beside-rail, mirror, layout, edge, `claimLanded`, `stageContrast`). `rail.css` is four `@import`s: `rail-chrome.css`, `rail-rows.css`, `rail-stage.css`, `rail-arch.css`.

`pageRows.svelte.ts`, `pageStage.svelte.ts`, `pagePointer.svelte.ts`, `pageRing.svelte.ts`, `pageClaim.svelte.ts`, `pageBeing.svelte.ts`, and `pageDrag.svelte.ts` hold function bodies moved out of `RailPage`. Each takes the existing `rail` object or a scope of the lets the page already had. Effects, `onMount`, `onDestroy`, `$derived`, `const rail`, and the template stay in the page.
