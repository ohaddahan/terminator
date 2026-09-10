# egui_dock local patch

Vendored crates.io egui_dock 0.21.1, retaining upstream MIT license.
Adds default no-op `trailing_controls_width` and `trailing_controls` TabViewer callbacks.
The leaf reserves their width alongside the add button before clipping/scrolling tabs,
and renders the controls adjacent to it. Existing docking and scroll code is retained.

The flatter UI adds a default-true `show_tab_bar(NodePath)` callback. Terminator
uses split ancestry to hide headers below a vertical split's top edge. Policy-hidden
panes reserve no header height and do not show the upstream header-reveal control.
Tabs and focus remain in the dock state; lower-pane tab selection and creation are
available through the application context menu.
