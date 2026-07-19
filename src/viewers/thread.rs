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
pub struct TextureWorker {
    job_texture_thread: Sender<(TileDescriptor, ViewMode)>,
    result_texture_thread: Receiver<Tile>,
    /// Check if the current vec tile to loaded is outdated
    wanted: Arc<Mutex<HashSet<TileDescriptor>>>,
}

impl TextureWorker {
    /// Initialize the texture worker thread
    pub fn new(ctx: egui::Context, dataset: Dataset) -> Self {
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
    pub fn set_wanted(&self, tiles: impl IntoIterator<Item = TileDescriptor>) {
        let mut w = self.wanted.lock().unwrap();
        w.clear();
        w.extend(tiles);
    }

    /// Send a request for a texture refresh
    pub fn request_load(&mut self, worker: (TileDescriptor, ViewMode)) -> Result<()> {
        self.job_texture_thread.send(worker)?;
        Ok(())
    }

    /// Check if a new texture is available
    pub fn poll_results(&mut self) -> Vec<Tile> {
        self.result_texture_thread.try_iter().collect()
    }
}

/// Create the separate thread for non-blocking image texture generation
pub fn spawn_worker(
    ctx: egui::Context,
    dataset: Dataset,
    wanted: Arc<Mutex<HashSet<TileDescriptor>>>,
) -> (Sender<(TileDescriptor, ViewMode)>, Receiver<Tile>) {
    let (job_tx, job_rx) = mpsc::channel::<(TileDescriptor, ViewMode)>();
    let (result_tx, result_rx) = mpsc::channel::<Tile>();

    thread::spawn(move || {
        // Process every queued job in order — don't collapse to "latest only"
        // anymore, since jobs now represent a real backlog of missing tiles.
        while let Ok((tile_descriptor, view)) = job_rx.recv() {
            if cfg!(debug_assertions) {
                println!("Loading tile: {}", tile_descriptor.name());
            }
            // Check if list is outdated
            if !wanted.lock().unwrap().contains(&tile_descriptor) {
                continue;
            }

            let image_color = match view.active_viewer {
                ActiveViewer::Panchro => {
                    // Read tile from file
                    let Ok(buffer) = tile_descriptor.read_buffer(&dataset, view.panchro_band)
                    else {
                        continue;
                    };
                    // Convert raw tile to RGBA
                    view.color_interpretation
                        .panchro_buffer_to_colorimage(buffer)
                }
                ActiveViewer::Color => {
                    let Ok(buffers) = tile_descriptor.read_3buffers(&dataset, view.rgb_bands)
                    else {
                        continue;
                    };
                    // Convert raw tile to RGBA
                    view.color_interpretation.rgb_buffers_to_colorimage(buffers)
                }
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
