use crate::RasterView;
use crate::icon;
use crate::raster::loading::build_pyramid;
use egui::{RichText, Ui};

impl RasterView {
    pub(crate) fn ui_top_panel(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            let button_file_name = if let Some(path) = &self.raster_path {
                format!(
                    "File: {}",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                )
            } else {
                "File".to_string()
            };
            if ui
                .button(button_file_name)
                .on_hover_text("Select a raster file")
                .clicked()
                && let Some(path) = rfd::FileDialog::new().pick_file()
            {
                let _ = self.update_path(path.as_path(), ui.ctx().clone());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Light Dark mode switch
                egui::widgets::global_theme_preference_switch(ui);

                // GDAL pyramid
                if let Some(view) = &mut self.viewer {
                    let has_overviews = view
                        .raster_handler
                        .has_all_overviews(view.parameters.tile_size);

                    // Poll any in-flight pyramid build first, so the UI reflects completion this frame.
                    if let Some(promise) = &self.app_state.pyramid_promise
                        && let Some(result) = promise.ready()
                    {
                        match result {
                            Ok(()) => {
                                let _ = view.raster_handler.refresh_dataset_only();
                            }
                            Err(e) => {
                                eprintln!("pyramid build failed: {e:?}");
                            }
                        }
                        self.app_state.pyramid_promise = None;
                    }

                    if self.app_state.pyramid_promise.is_some() {
                        // Build in progress: show spinner instead of the button.
                        ui.add(egui::Spinner::new())
                            .on_hover_text("Building GDAL pyramid…");
                    } else {
                        let button = ui
                            .add_enabled(
                                !has_overviews,
                                egui::Button::new(RichText::new(icon::regular::TORNADO)),
                            )
                            .on_hover_text(if has_overviews {
                                "Overviews already available"
                            } else {
                                "Generate GDAL pyramid"
                            });

                        if button.clicked() {
                            let local_dataset = view.raster_handler.as_owned_dataset();
                            let tile_size = view.parameters.tile_size;

                            if let Ok(mut ds) = local_dataset {
                                self.app_state.pyramid_promise =
                                    Some(poll_promise::Promise::spawn_thread(
                                        "build_pyramid",
                                        move || {
                                            build_pyramid(&mut ds, "AVERAGE", tile_size)
                                                .map_err(anyhow::Error::from)
                                        },
                                    ));
                            }
                        }
                    }
                }

                // Refresh button
                let button_response = ui.button("Refresh").on_hover_text("Refresh cache");
                if button_response.clicked()
                    && let Some(view) = &mut self.viewer
                {
                    view.refresh_cache();
                }
                button_response.context_menu(|ui| {
                    if ui
                        .button("Reload")
                        .on_hover_text("Reload the file and reset the viewer")
                        .clicked()
                        && let Some(path) = &mut self.raster_path.clone()
                    {
                        let _ = self.update_path_force(path.as_path(), ui.ctx().clone());
                    }
                });

                // Loading spin
                if let Some(view) = &self.viewer {
                    let loading = &view.state.background_loading;

                    if loading.any_loading() {
                        let mut items = Vec::new();

                        if loading.loading_tile {
                            items.push("tile");
                        }

                        if loading.loading_stat {
                            items.push("stat");
                        }

                        let message = format!("Loading: {}", items.join(", "));

                        ui.add(egui::Spinner::new()).on_hover_text(message);
                    } else {
                        ui.label(egui::RichText::from(icon::regular::CHECK_FAT))
                            .on_hover_text("Nothing to load");
                    }
                }
            });
        });
    }
}
