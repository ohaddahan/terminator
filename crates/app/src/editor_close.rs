//! Close editor processes without treating file views as shell sessions.
use anyhow::{Context, Result, ensure};
use std::{
    process::Command,
    time::{Duration, Instant},
};
use terminator_core::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Workspace(String, String),
    Pane(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Check,
    Save,
    Discard,
}
fn snapshot(paths: &Paths) -> Result<Box<State>> {
    let Response::State(state) = rpc(paths, Request::Snapshot)? else {
        anyhow::bail!("Could not inspect editor state")
    };
    Ok(state)
}
pub fn close(paths: &Paths, ids: &[String], mode: Mode) -> Result<()> {
    let state = snapshot(paths)?;
    let live: Vec<_> = ids
        .iter()
        .filter(|id| {
            state
                .sessions
                .iter()
                .any(|s| &s.id == *id && s.lifecycle.live())
        })
        .collect();
    ensure!(
        live.iter().all(|id| state
            .sessions
            .iter()
            .any(|s| &s.id == *id && s.kind == SessionKind::Editor)),
        "This view contains a running terminal"
    );
    ensure!(
        !state
            .agents
            .iter()
            .any(|agent| ids.contains(&agent.session_id)
                && !matches!(
                    agent.state,
                    AgentState::Completed | AgentState::Failed | AgentState::Stopped
                )),
        "An agent is running in this editor. Use the terminal session close controls."
    );
    // Preflight all buffers before closing any member of a file-only tab.
    for id in &live {
        if mode == Mode::Save {
            rpc(
                paths,
                Request::EditorSave {
                    session: (*id).clone(),
                },
            )?;
        }
        if mode != Mode::Discard {
            let Response::Text(status) = rpc(
                paths,
                Request::EditorStatus {
                    session: (*id).clone(),
                },
            )?
            else {
                anyhow::bail!("Could not check unsaved changes")
            };
            ensure!(
                status
                    .trim()
                    .parse::<usize>()
                    .context("Could not check unsaved changes")?
                    == 0,
                "Unsaved changes"
            );
        }
    }
    for id in &live {
        if mode == Mode::Discard && !paths.editor_socket(id).exists() {
            rpc(
                paths,
                Request::Stop {
                    session: (*id).clone(),
                },
            )?;
            continue;
        }
        let editor = find_executable(if state.sessions.iter().any(|s| &s.id == *id && s.review) {
            "nvim"
        } else {
            &state.settings.editor_program
        })
        .context("Editor executable unavailable")?;
        let mut command = Command::new(editor);
        command
            .arg("--server")
            .arg(paths.editor_socket(id))
            .arg("--remote-send")
            .arg(if mode == Mode::Discard {
                "<C-\\><C-N>:qa!<CR>"
            } else {
                "<C-\\><C-N>:qa<CR>"
            });
        let output = bounded_output(command, Duration::from_secs(2))?;
        ensure!(
            output.status.success(),
            "Could not close editor: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // A normal :qa also protects edits made after the preflight check.
    let started = Instant::now();
    loop {
        let state = snapshot(paths)?;
        if !state
            .sessions
            .iter()
            .any(|s| ids.contains(&s.id) && s.lifecycle.live())
        {
            return Ok(());
        }
        ensure!(
            started.elapsed() < Duration::from_secs(2),
            "Editor did not close. Check for unsaved buffers or running editor jobs."
        );
        std::thread::sleep(Duration::from_millis(40));
    }
}
