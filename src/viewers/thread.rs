use crate::viewers::ViewMode;
use crate::viewers::tiler::{Tile, TileDescriptor};
use anyhow::Result;
use gdal::Dataset;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// Handler of the texture thread
///
/// * `texture_worker.request_load(worker)` to ask to generate new texture
/// * `texture_worker.poll_request()` to check if a newer texture is available
pub(crate) struct TextureWorker {
    job_texture_thread: Sender<(TileDescriptor, ViewMode)>,
    result_texture_thread: Receiver<Tile>,
}

impl TextureWorker {
    /// Initialize the texture worker thread
    pub(super) fn new(ctx: egui::Context, dataset: Dataset) -> Self {
        let (job_texture_thread, result_texture_thread) = spawn_worker(ctx, dataset);
        Self {
            job_texture_thread,
            result_texture_thread,
        }
    }

    /// Send a request for a texture refresh
    pub(crate) fn request_load(&mut self, worker: (TileDescriptor, ViewMode)) -> Result<()> {
        self.job_texture_thread.send(worker)?;
        Ok(())
    }

    /// Check if a new texture is available
    pub(crate) fn poll_result(&mut self) -> Option<Tile> {
        let mut latest = None;
        while let Ok(res) = self.result_texture_thread.try_recv() {
            // in case results queued up
            latest = Some(res);
        }
        latest
    }
}

/// Create the separate thread for non-blocking image texture generation
pub fn spawn_worker(
    ctx: egui::Context,
    dataset: Dataset,
) -> (Sender<(TileDescriptor, ViewMode)>, Receiver<Tile>) {
    let (job_tx, job_rx) = mpsc::channel::<(TileDescriptor, ViewMode)>();
    let (result_tx, result_rx) = mpsc::channel::<Tile>();

    thread::spawn(move || {
        loop {
            // Block here with zero CPU until a job arrives
            let Ok(job) = job_rx.recv() else { break }; // breaks if Sender dropped

            // Drain any newer jobs that arrived while we were busy
            let (tile_descriptor, view): (TileDescriptor, ViewMode) =
                job_rx.try_iter().last().unwrap_or(job);

            // Tile descriptor to buffer
            let buffer = tile_descriptor.read_buffer(&dataset);

            if let Ok(b) = buffer {
                // Buffer to ColorImage
                let image_color = view.color.buffer_to_colorimage(b);

                // ColorImage to Texture
                let texture_handle = ctx.load_texture(
                    format!("texture_tile_{}", tile_descriptor.name()),
                    image_color,
                    egui::TextureOptions::NEAREST,
                );

                // Texture as Tile
                let tile = Tile::new(tile_descriptor, texture_handle);
                let _ = result_tx.send(tile);
            }
        }
    });

    (job_tx, result_rx)
}
