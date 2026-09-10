use crate::Update;
use std::{
    path::PathBuf,
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub cwd: PathBuf,
    pub identity: Option<(u32, u64)>,
    pub include_pr: bool,
    pub generation: u64,
}
pub fn spawn(updates: Sender<Update>, ctx: eframe::egui::Context) -> Sender<Option<Request>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut active = None::<Request>;
        let mut cache = terminator_core::metadata::Cache::default();
        loop {
            match rx.recv_timeout(Duration::from_secs(3)) {
                Ok(mut next) => {
                    while let Ok(newer) = rx.try_recv() {
                        next = newer;
                    }
                    active = next;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(_) => {}
            }
            let Some(request) = &active else {
                continue;
            };
            let data = cache.collect(&request.cwd, request.identity, request.include_pr);
            if updates
                .send(Update::Metadata(request.generation, data))
                .is_err()
            {
                break;
            }
            ctx.request_repaint();
        }
    });
    tx
}
