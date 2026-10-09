# src/lib/companion

Companion UI pieces. `CompanionPage.svelte` mounts them.

`Mouth.svelte`, `Spirit.svelte`, `Well.svelte`, `Notes.svelte`, and the dock pieces (`AgentLeft.svelte`, `AgentRight.svelte`, `Dests.svelte`, `Toast.svelte`, `CheatHint.svelte`) live here. `page.css` and `dock.css` are the styles.

`pageHug.svelte.ts`, `pageNeeded.svelte.ts`, `pageExpand.svelte.ts`, and `pageDrag.svelte.ts` hold function bodies moved out of `CompanionPage`. The page keeps effects, `onMount`, and `onDestroy`.
