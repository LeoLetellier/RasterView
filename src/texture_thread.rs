use crate::view_mode::{ColorInterpretationWorker, RasterViewHandle};
use anyhow::Result;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// Handler of the texture thread
///
/// * `texture_worker.request_load(worker)` to ask to generate new texture
/// * `texture_worker.poll_request()` to check if a newer texture is available
pub(crate) struct TextureWorker {
    job_texture_thread: Sender<ColorInterpretationWorker>,
    result_texture_thread: Receiver<RasterViewHandle>,
}

impl TextureWorker {
    /// Initialize the texture worker thread
    pub(super) fn new(ctx: egui::Context) -> Self {
        let (job_texture_thread, result_texture_thread) = spawn_worker(ctx);
        Self {
            job_texture_thread,
            result_texture_thread,
        }
    }

    /// Send a request for a texture refresh
    pub(crate) fn request_load(&mut self, worker: ColorInterpretationWorker) -> Result<()> {
        self.job_texture_thread.send(worker)?;
        Ok(())
    }

    /// Check if a new texture is available
    pub(crate) fn poll_result(&mut self) -> Option<RasterViewHandle> {
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
) -> (
    Sender<ColorInterpretationWorker>,
    Receiver<RasterViewHandle>,
) {
    let (job_tx, job_rx) = mpsc::channel::<ColorInterpretationWorker>();
    let (result_tx, result_rx) = mpsc::channel::<RasterViewHandle>();

    thread::spawn(move || {
        loop {
            // Block here with zero CPU until a job arrives
            let Ok(job) = job_rx.recv() else { break }; // breaks if Sender dropped

            // Drain any newer jobs that arrived while we were busy
            let job: ColorInterpretationWorker = job_rx.try_iter().last().unwrap_or(job);

            // Compute the new texture
            if let Ok(handle) = job.texture(&ctx) {
                let _ = result_tx.send(handle);
                //ctx.request_repaint(); but dont know if the new texture was effectively applied at the time the repaint is done ?
            }
        }
    });

    (job_tx, result_rx)
}
