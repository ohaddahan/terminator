//! Markdown images use the same bounded background decoder as image tabs.
use eframe::egui::{
    self,
    load::{ImageLoader, ImagePoll, LoadError},
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, mpsc},
    thread,
};

type Decoded = Result<Arc<egui::ColorImage>, String>;
struct Entry {
    ticket: u64,
    result: Option<Decoded>,
}
pub struct Images {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
    requests: mpsc::SyncSender<(String, u64, egui::Context)>,
    next: std::sync::atomic::AtomicU64,
}
impl Images {
    pub fn install(ctx: &egui::Context) -> Arc<Self> {
        let entries = Arc::new(Mutex::new(HashMap::<String, Entry>::new()));
        let (requests, incoming) = mpsc::sync_channel::<(String, u64, egui::Context)>(8);
        let cache = entries.clone();
        thread::spawn(move || {
            while let Ok((uri, ticket, ctx)) = incoming.recv() {
                let path = uri
                    .strip_prefix("markdown-image:")
                    .and_then(|s| url::Url::parse(s).ok())
                    .and_then(|u| u.to_file_path().ok());
                let result = path
                    .ok_or_else(|| "Invalid local image path".to_owned())
                    .and_then(|path| {
                        crate::image_preview::decode(&path)
                            .map(Arc::new)
                            .map_err(|e| format!("{e:#}"))
                    });
                let mut entries = cache.lock().unwrap();
                let used: usize = entries
                    .values()
                    .filter_map(|v| v.result.as_ref())
                    .filter_map(|r| r.as_ref().ok())
                    .map(|i| i.pixels.len() * 4)
                    .sum();
                if let Some(entry) = entries.get_mut(&uri).filter(|e| e.ticket == ticket) {
                    entry.result = Some(result.and_then(|image| {
                        if used + image.pixels.len() * 4 > 64 * 1024 * 1024 {
                            Err("Markdown images exceed the 64 MiB preview limit".into())
                        } else {
                            Ok(image)
                        }
                    }));
                    ctx.request_repaint();
                }
            }
        });
        let loader = Arc::new(Self {
            entries,
            requests,
            next: Default::default(),
        });
        ctx.add_image_loader(loader.clone());
        loader
    }
    pub fn clear(&self, ctx: &egui::Context) {
        self.retain(ctx, &HashSet::new());
    }
    pub fn retain(&self, ctx: &egui::Context, used: &HashSet<String>) {
        let unused: Vec<_> = self
            .entries
            .lock()
            .unwrap()
            .keys()
            .filter(|uri| !used.contains(*uri))
            .cloned()
            .collect();
        for uri in unused {
            ctx.forget_image(&uri);
        }
    }
}
impl ImageLoader for Images {
    fn id(&self) -> &str {
        concat!(module_path!(), "::Images")
    }
    fn load(
        &self,
        ctx: &egui::Context,
        uri: &str,
        _: egui::SizeHint,
    ) -> egui::load::ImageLoadResult {
        if !uri.starts_with("markdown-image:") {
            return Err(LoadError::NotSupported);
        }
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.get(uri) {
            return match &entry.result {
                Some(Ok(image)) => Ok(ImagePoll::Ready {
                    image: image.clone(),
                }),
                Some(Err(error)) => Err(LoadError::Loading(error.clone())),
                None => Ok(ImagePoll::Pending { size: None }),
            };
        }
        if entries.len() >= 32 {
            return Err(LoadError::Loading(
                "Markdown preview supports up to 32 local images".into(),
            ));
        }
        let ticket = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        match self.requests.try_send((uri.into(), ticket, ctx.clone())) {
            Ok(()) => {
                entries.insert(
                    uri.into(),
                    Entry {
                        ticket,
                        result: None,
                    },
                );
            }
            Err(mpsc::TrySendError::Full(_)) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100))
            }
            Err(_) => {
                return Err(LoadError::Loading(
                    "Image preview worker unavailable".into(),
                ));
            }
        }
        Ok(ImagePoll::Pending { size: None })
    }
    fn forget(&self, uri: &str) {
        self.entries.lock().unwrap().remove(uri);
    }
    fn forget_all(&self) {
        self.entries.lock().unwrap().clear();
    }
    fn byte_size(&self) -> usize {
        self.entries
            .lock()
            .unwrap()
            .values()
            .filter_map(|v| v.result.as_ref())
            .filter_map(|r| r.as_ref().ok())
            .map(|i| i.pixels.len() * 4)
            .sum()
    }
    fn has_pending(&self) -> bool {
        self.entries
            .lock()
            .unwrap()
            .values()
            .any(|v| v.result.is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_images_decode_off_thread_and_can_be_reloaded_and_released() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("picture.png");
        image::RgbaImage::from_pixel(12, 8, image::Rgba([210, 60, 40, 255]))
            .save(&path)
            .unwrap();
        let ctx = egui::Context::default();
        let images = Images::install(&ctx);
        let uri = format!(
            "markdown-image:{}",
            url::Url::from_file_path(&path).unwrap()
        );
        let wait = || {
            let start = std::time::Instant::now();
            loop {
                match images.load(&ctx, &uri, egui::SizeHint::default()) {
                    Ok(ImagePoll::Pending { .. }) => {
                        assert!(start.elapsed() < std::time::Duration::from_secs(3));
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    result => return result,
                }
            }
        };
        let ImagePoll::Ready { image } = wait().unwrap() else {
            panic!("Image not decoded")
        };
        assert_eq!(image.size, [12, 8]);
        assert_eq!(images.byte_size(), 12 * 8 * 4);
        std::fs::write(&path, "broken image").unwrap();
        images.clear(&ctx);
        assert!(matches!(wait(), Err(LoadError::Loading(_))));
        assert!(matches!(
            images.load(&ctx, "https://example.com/a.png", egui::SizeHint::default()),
            Err(LoadError::NotSupported)
        ));
        images.clear(&ctx);
        assert_eq!(images.byte_size(), 0);
    }
}
