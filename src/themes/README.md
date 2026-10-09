# themes

Theme tokens and window opacity.

`apply.ts` applies tokens and window opacity (`applyWindowOpacity` publishes `--bg-fill` and `--panel-fill`). `host.ts` is the system table (`systemLight` / `systemDark`).

`RailPage` and `KanbanPage` call `apply.ts`. Settings picks a theme id; it does not own this table.
