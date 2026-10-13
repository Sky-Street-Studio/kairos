# Route testing

`kairos test-routes <project>` runs the project without graphics and explores each menu choice and both sides of every conditional. It reports unreachable labels, missing assets reached during play, routes that end in runtime errors and choices that fail to resolve.

```sh
kairos test-routes examples/hello-kairos
kairos test-routes examples/hello-kairos --json
```

The command exits non-zero when the report contains a problem or exploration was truncated. `--json` writes the same report to standard output. Set `--max-depth`, `--max-loop-revisits` or `--max-routes` to adjust the exploration limits. Defaults are 1,000 interactions per route, three visits to each label across route decisions and 10,000 explored route states. The VM also stops a script that runs one million instructions without an interaction, which bounds tight loops.

The report includes `reachableLabels`, `unreachableLabels`, `deadEnds`, `missingAssets`, `unresolvedChoices`, `truncated` and `routesExplored`. Exploration starts at the configured start label and also runs `splashscreen` and `main_menu` when present, matching the player's entrypoints. Asset checks cover effects produced on explored paths; unused assets and references hidden behind unreachable labels are not reported. Menu guards use their runtime values, while `if` conditions are explored on both sides. Every explored route starts with project defaults, so assignments on one route do not leak into another.

The editor binding `testRoutes(projectPath)` returns the same report fields for later presentation in the editor. It uses the default limits; editor UI is not part of this contract.

## Decisions

- Exploration uses cloned VM states at conditional and menu branches. This preserves normal VM execution for expressions, menu visibility, calls, returns and side effects while isolating each route.
- Conditional expressions are not forced to both values; the route runner branches at the compiled conditional instruction. This covers variable-dependent paths without inventing values for variables.
- Hitting a route cap is a reported truncation and a non-zero CLI result, since an incomplete pass cannot certify a project.
- Explicit `return` and `quit` are valid terminals. Falling off the script without either is a dead end; runtime errors and menu choices that reach those failures are reported too.
