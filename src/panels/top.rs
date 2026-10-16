use crate::RasterView;
use crate::icon;
use crate::raster::loading::build_pyramid;
use egui::{RichText, Ui};
use std::cell::Cell;

enum MenuAction {
    Refresh,
    Reload,
    Close,
    BuildPyramid,
    Reveal,
}

impl RasterView {
    pub(crate) fn ui_top_panel(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();

        // Deferred actions (applied after the layout closures finish)
        let mut picked_path: Option<std::path::PathBuf> = None;
        let menu_action: Cell<Option<MenuAction>> = Cell::new(None);

        // Extents used to place the centered label
        let mut left_end_x = 0.0_f32;
        let mut right_start_x = f32::MAX;

        egui::MenuBar::new().ui(ui, |ui| {
            egui::Sides::new().shrink_left().truncate().show(
                ui,
                // ---------- LEFT: menus ----------
                |ui| {
                    ui.menu_button(".", |ui| {
                        if ui.button("Parameters").clicked() {
                            self.app_state.settings_panel.is_open = true;
                        }
                        if ui.button("About").clicked() {}
                    });

                    ui.menu_button("Open", |ui| {
                        if ui
                            .button("GDAL raster")
                            .on_hover_text("Select a raster file")
                            .clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_file()
                        {
                            picked_path = Some(path);
                        }
                        if ui.button("REAL4 raster").clicked() {}
                        if ui.button("AMSTer raster").clicked() {}
                        if ui.button("Raw raster").clicked() {}
                    });

                    ui.menu_button("View", |ui| {
                        if ui.button("Refresh cache").clicked() {
                            menu_action.set(Some(MenuAction::Refresh));
                        }
                        if ui.button("Reload raster").clicked() {
                            menu_action.set(Some(MenuAction::Reload));
                        }
                        if ui.button("Close raster").clicked() {
                            menu_action.set(Some(MenuAction::Close));
                        }
                    });

                    ui.menu_button("Raster", |ui| {
                        if ui.button("Generate pyramid").clicked() {
                            menu_action.set(Some(MenuAction::BuildPyramid));
                        }
                        if ui.button("Build VRT").clicked() {}
                    });

                    left_end_x = ui.min_rect().right();
                },
                // ---------- RIGHT: controls (added right-to-left) ----------
                |ui| {
                    // Light/Dark mode switch
                    egui::widgets::global_theme_preference_switch(ui);

                    // GDAL pyramid
                    if let Some(view) = &mut self.viewer {
                        let has_overviews = view
                            .raster_handler
                            .has_all_overviews(view.parameters.tile_size);

                        // Poll any in-flight pyramid build first
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
                        {
                            menu_action.set(Some(MenuAction::Reload));
                            ui.close();
                        }

                        if ui
                            .button("Unload")
                            .on_hover_text("Unload current raster")
                            .clicked()
                        {
                            menu_action.set(Some(MenuAction::Close));
                            ui.close();
                        }
                    });

                    // Loading spinner
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

                    right_start_x = ui.min_rect().left();
                },
            );

            // ---------- CENTER: file name, only when a raster is loaded ----------
            if let Some(path) = &self.raster_path {
                let full = ui.max_rect();
                let center_x = full.center().x;
                let gap = 8.0;

                // Room available on each side of the window midpoint
                let half_room = (center_x - left_end_x).min(right_start_x - center_x) - gap;

                if half_room > 30.0 {
                    let label = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                        .to_string();

                    // Natural button width, capped to the available room
                    let text_w = ui
                        .painter()
                        .layout_no_wrap(
                            label.clone(),
                            egui::TextStyle::Button.resolve(ui.style()),
                            egui::Color32::WHITE,
                        )
                        .size()
                        .x;
                    let natural_w = text_w + ui.spacing().button_padding.x * 2.0;
                    let width = natural_w.min(half_room * 2.0);

                    let rect = egui::Rect::from_center_size(
                        egui::pos2(center_x, full.center().y),
                        egui::vec2(width, ui.spacing().interact_size.y),
                    );

                    let response = ui
                        .put(rect, egui::Button::new(label).truncate())
                        .on_hover_text(path.display().to_string());

                    let reveal_label = if cfg!(target_os = "macos") {
                        "Reveal in Finder"
                    } else if cfg!(target_os = "windows") {
                        "Show in Explorer"
                    } else {
                        "Show in file manager"
                    };

                    if response.clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_file() {
                            picked_path = Some(p);
                        }
                    }

                    response.context_menu(|ui| {
                        if ui.button(reveal_label).clicked() {
                            menu_action.set(Some(MenuAction::Reveal));
                            ui.close();
                        }
                    });
                }
            }
        });

        // ---------- Apply deferred actions (no closures borrow self now) ----------
        if let Some(path) = picked_path {
            let _ = self.update_path(path.as_path(), ctx.clone());
        }

        match menu_action.take() {
            Some(MenuAction::Reload) => {
                if let Some(path) = self.raster_path.clone() {
                    let _ = self.update_path_force(path.as_path(), ctx.clone());
                }
            }
            Some(MenuAction::Close) => {
                self.raster_path = None;
                self.viewer = None;
            }
            Some(MenuAction::Reveal) => {
                if let Some(path) = &self.raster_path {
                    reveal_in_file_manager(path);
                }
            }
            Some(MenuAction::Refresh) => {
                if let Some(view) = &mut self.viewer {
                    view.refresh_cache();
                }
            }
            Some(MenuAction::BuildPyramid) => {
                if let Some(view) = &mut self.viewer {
                    let local_dataset = view.raster_handler.as_owned_dataset();
                    let tile_size = view.parameters.tile_size;

                    if let Ok(mut ds) = local_dataset {
                        self.app_state.pyramid_promise = Some(poll_promise::Promise::spawn_thread(
                            "build_pyramid",
                            move || {
                                build_pyramid(&mut ds, "AVERAGE", tile_size)
                                    .map_err(anyhow::Error::from)
                            },
                        ));
                    }
                }
            }
            None => {}
        }
    }
}

fn reveal_in_file_manager(path: &std::path::Path) {
    if let Err(e) = opener::reveal(path) {
        eprintln!("failed to reveal {}: {e:?}", path.display());
    }
}
