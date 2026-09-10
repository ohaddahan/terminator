use eframe::egui;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAction {
    Open,
    Text,
    Split,
    External,
    Browser,
    Copy,
    StagedDiff,
    WorkingDiff,
}
pub fn menu(ui: &mut egui::Ui, file: bool, browser: bool, git: bool) -> Option<FileAction> {
    for (available, label, action) in [
        (file, "Open file", FileAction::Open),
        (file, "Open as text", FileAction::Text),
        (file, "Open in editor split", FileAction::Split),
        (file, "Open externally", FileAction::External),
        (browser, "Open in browser", FileAction::Browser),
        (git, "Staged diff", FileAction::StagedDiff),
        (git, "Working tree diff", FileAction::WorkingDiff),
        (true, "Copy target", FileAction::Copy),
    ] {
        if available {
            let icon = match action {
                FileAction::Open | FileAction::Text => "FileCode",
                FileAction::Split => "PanelRightClose",
                FileAction::External | FileAction::Browser => "ExternalLink",
                FileAction::Copy => "Copy",
                FileAction::StagedDiff | FileAction::WorkingDiff => "FileDiff",
            };
            let response = crate::appearance::menu_item(ui, label, icon, "");
            #[cfg(feature = "test-support")]
            crate::diagnostics::record(ui.ctx(), label, response.rect);
            if response.clicked() {
                ui.close();
                return Some(action);
            }
        }
    }
    None
}
