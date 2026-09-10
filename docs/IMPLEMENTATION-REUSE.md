# Native reuse implementation

User-authorized follow-up to the UI/package review. Preserve the existing dirty
work, native Rust GUI, daemon-owned PTYs, explicit hook installation, and
capability-gated compatibility with older running daemons.

- [x] Native image preview tabs: bounded background raster/SVG decoding, fit/zoom,
  explicit text/external opening, persistence, no PTY allocation.
- [x] Rust xtask replaces Python package, PTY/native regression, load and provider
  fixture tooling, retaining the behavioral checks and isolated data directories.
- [x] Remove duplicate daemon terminal emulation using vt100 callbacks for replies
  and OSC notifications; retain the Alacritty-backed GUI widget and test replay.
- [x] Explicit Git worktree management/registry through the CLI; read-only UI
  metadata for branch, worktree, PR, listening ports and terminal notifications.
- [x] Extend documented CLI/control operations without replacing the working
  authenticated protocol or sending unsupported requests to older daemons.
- [x] Browser integration: preserve no-webview default unless the user explicitly
  selects an embedded browser; launch/navigation through supported native tools.
- [x] Validate native input/window controls where the desktop permits it; record
  any remaining host/Wayland/provider limitations without claiming coverage.

Existing eframe/egui, egui_dock, portable-pty, Alacritty, vt100, notify, rusqlite,
rfd and Git integrations are retained. Ghostty migration and Tree-sitter are not
required by the reviewed fixes. Native libraries used by dependencies and Neovim
are distinct from our Rust application and development tooling.
