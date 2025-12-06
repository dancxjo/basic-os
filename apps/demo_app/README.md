# Demo App

This app is a compact flex-layout showcase wired through the widget graph.

## Layout Graph

- Root window hosts a `container.vertical` with a 12px gap and column flex direction.
- Toolbar row: `container.toolbar` (kind `toolbar`) with four buttons for Text Editor, Graph Viewer, Thing Viewer, and Self Edit.
- Content row: another `container.vertical` set to `flex_direction=row`, holding:
  - Left pane: `primary_list` (kind `listbox_default`) bound to the selection node, `flex_grow=1`, width 320.
  - Right pane: `thing_tile` inspector with `flex_grow=2` showing the selected entry details.

## How to Run

- Build everything: `make`
- Run hosted compositor: `cargo run -p compositor --features host`

The demo app will open a single window showing the toolbar plus the list/inspector panes. Extend it by adding regions to the template in `apps/demo_app/src/app.rs`.
