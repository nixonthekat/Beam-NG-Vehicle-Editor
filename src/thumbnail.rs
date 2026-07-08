use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use egui::{ColorImage, TextureHandle, TextureOptions};

use crate::vehicle_source::{read_zip_entry_bytes, ThumbnailSource};

pub struct ThumbnailCache {
    textures: HashMap<String, TextureHandle>,
    pending: HashMap<String, ColorImage>,
    loading: HashSet<String>,
    failed: HashSet<String>,
    tx: Sender<(String, Option<ColorImage>)>,
    rx: Receiver<(String, Option<ColorImage>)>,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            textures: HashMap::new(),
            pending: HashMap::new(),
            loading: HashSet::new(),
            failed: HashSet::new(),
            tx,
            rx,
        }
    }

    pub fn queue_load(&mut self, id: &str, source: &ThumbnailSource) {
        if self.textures.contains_key(id)
            || self.pending.contains_key(id)
            || self.loading.contains(id)
            || self.failed.contains(id)
        {
            return;
        }

        self.loading.insert(id.to_string());
        let tx = self.tx.clone();
        let id_owned = id.to_string();
        let source = source.clone();

        thread::spawn(move || {
            let result = load_thumbnail_image(&source);
            let _ = tx.send((id_owned, result));
        });
    }

    pub fn poll(&mut self) {
        while let Ok((id, result)) = self.rx.try_recv() {
            self.loading.remove(&id);
            match result {
                Some(img) => {
                    self.pending.insert(id, img);
                }
                None => {
                    self.failed.insert(id);
                }
            }
        }
    }

    pub fn upload_pending(&mut self, ctx: &egui::Context) {
        let pending: Vec<_> = self.pending.drain().collect();
        for (id, color) in pending {
            let texture = ctx.load_texture(
                format!("thumb_{id}"),
                color,
                TextureOptions::LINEAR,
            );
            self.textures.insert(id, texture);
        }
    }

    pub fn get(&self, id: &str) -> Option<&TextureHandle> {
        self.textures.get(id)
    }

    pub fn is_loading(&self, id: &str) -> bool {
        self.loading.contains(id)
    }

    pub fn clear(&mut self) {
        self.textures.clear();
        self.pending.clear();
        self.loading.clear();
        self.failed.clear();
        while self.rx.try_recv().is_ok() {}
    }

    pub fn has_pending(&self) -> bool {
        !self.loading.is_empty()
    }
}

fn load_thumbnail_image(source: &ThumbnailSource) -> Option<ColorImage> {
    let bytes = match source {
        ThumbnailSource::File(path) => std::fs::read(path).ok()?,
        ThumbnailSource::Zip {
            archive_path,
            entry,
        } => read_zip_entry_bytes(archive_path, entry).ok()?,
    };
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, &img))
}
