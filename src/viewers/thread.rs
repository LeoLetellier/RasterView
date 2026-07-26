use crate::viewers::tasking::ViewStyle;
use crate::viewers::tiler::{Tile, TileDescriptor};
use crate::viewers::{ActiveViewer, ViewMode};
use anyhow::Result;
use gdal::Dataset;
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

/// Handler of the texture thread
///
/// * `texture_worker.request_load(worker)` to ask to generate new texture
/// * `texture_worker.poll_request()` to check if a newer texture is available
#[derive(Debug)]
pub(crate) struct TextureWorker {
    job_texture_thread: Sender<TileDescriptor>,
    result_texture_thread: Receiver<Tile>,
    /// Check if the current vec tile to loaded is outdated
    wanted: Arc<Mutex<HashSet<TileDescriptor>>>,
}

impl TextureWorker {
    /// Initialize the texture worker thread
    pub(crate) fn new(ctx: egui::Context, dataset: Dataset) -> Self {
        let wanted = Arc::new(Mutex::new(HashSet::new()));
        let (job_texture_thread, result_texture_thread) =
            spawn_worker(ctx, dataset, wanted.clone());
        Self {
            job_texture_thread,
            result_texture_thread,
            wanted,
        }
    }

    /// Replace the set of tiles that are still relevant. Call once per frame
    /// before queuing new jobs, so the worker can drop stale ones.
    pub(crate) fn set_wanted(&self, tiles: impl IntoIterator<Item = TileDescriptor>) {
        let mut w = self.wanted.lock().unwrap();
        w.clear();
        w.extend(tiles);
    }

    /// Send a request for a texture refresh
    pub(crate) fn request_load(&mut self, worker: TileDescriptor) -> Result<()> {
        self.job_texture_thread.send(worker)?;
        Ok(())
    }

    /// Check if a new texture is available
    pub(crate) fn poll_results(&mut self) -> Vec<Tile> {
        self.result_texture_thread.try_iter().collect()
    }
}

/// Create the separate thread for non-blocking image texture generation
pub(crate) fn spawn_worker(
    ctx: egui::Context,
    dataset: Dataset,
    wanted: Arc<Mutex<HashSet<TileDescriptor>>>,
) -> (Sender<TileDescriptor>, Receiver<Tile>) {
    let (job_tx, job_rx) = mpsc::channel::<TileDescriptor>();
    let (result_tx, result_rx) = mpsc::channel::<Tile>();

    thread::spawn(move || {
        // Process every queued job in order
        while let Ok(tile_descriptor) = job_rx.recv() {
            if cfg!(debug_assertions) {
                tracing::info!("Loading tile: {}", tile_descriptor.name());
            }
            // Check if list is outdated
            if !wanted.lock().unwrap().contains(&tile_descriptor) {
                continue;
            }

            // Fetch the ColorImage
            let Some(image_color) = tile_descriptor.tile_to_colorimage(&dataset) else {
                continue;
            };

            // Check if list is outdated
            if !wanted.lock().unwrap().contains(&tile_descriptor) {
                continue;
            }
            // Register RGBA as texture
            let texture_handle = ctx.load_texture(
                format!("texture_tile_{}", tile_descriptor.name()),
                image_color,
                egui::TextureOptions::NEAREST,
            );

            // Create the tile with the texture and tile description
            let tile = Tile::new(tile_descriptor, texture_handle);

            // Send the resulting tile to main thread
            if result_tx.send(tile).is_ok() {
                ctx.request_repaint(); // wake the UI so it picks this up
            }
        }
    });

    (job_tx, result_rx)
}
