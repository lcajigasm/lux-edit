use super::*;
use eframe::egui;

impl LuxApp {
    pub(super) fn show_tab_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(TAB_BAR_BG)
            .inner_margin(egui::Margin::same(0.0)) // No margin, full bleed
            .show(ui, |ui| {
                let mut tab_rects: Vec<(usize, egui::Rect, bool)> = Vec::new();
                let mut tab_action: Option<TabAction> = None;
                let pointer_pos = ui.ctx().input(|i| i.pointer.latest_pos());
                let pointer_released = ui.ctx().input(|i| i.pointer.any_released());
                let mut primary_drop_rect: Option<egui::Rect> = None;
                let mut secondary_drop_rect: Option<egui::Rect> = None;

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO; // Connect tabs

                    egui::ScrollArea::horizontal()
                        .id_salt("tabs_scroll")
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::Vec2::new(1.0, 0.0); // 1px gap

                                for i in 0..self.editors.len() {
                                    let (title, modified, pinned) = {
                                        let editor = &self.editors[i];
                                        (editor.title.clone(), editor.modified, editor.pinned)
                                    };
                                    let is_active = i == self.active_tab;
                                    let text = title.clone();

                                    let text_color = if is_active {
                                        egui::Color32::from_rgb(230, 230, 245)
                                    } else {
                                        egui::Color32::from_rgb(120, 120, 150)
                                    };

                                    // Calculate Width
                                    let font = egui::FontId::proportional(12.5); // Slightly larger
                                    let text_width = ui.fonts(|f| {
                                        f.layout_no_wrap(text.clone(), font.clone(), text_color)
                                            .rect
                                            .width()
                                    });
                                    let tab_width =
                                        text_width + TAB_PADDING_X * 2.0 + TAB_CLOSE_SIZE + 8.0;
                                    if self.editors.len() <= 1 {
                                        // Even single tab might want a close button in modern designs,
                                        // but let's keep it optional if space is tight?
                                        // Actuall VS code usually shows it on hover.
                                        // For now, let's include space for it to avoid jumping.
                                    }
                                    let tab_width = tab_width.clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH);

                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::Vec2::new(tab_width, TAB_HEIGHT),
                                        egui::Sense::click(),
                                    );
                                    tab_rects.push((i, rect, pinned));

                                    // Background
                                    let bg = if is_active {
                                        TAB_ACTIVE_BG
                                    } else if response.hovered() {
                                        TAB_HOVER_BG
                                    } else {
                                        TAB_INACTIVE_BG
                                    };

                                    // Custom Tab Painting
                                    let painter = ui.painter();

                                    // Main tab shape
                                    painter.rect_filled(rect, 0.0, bg);

                                    // Active top border
                                    if is_active {
                                        painter.rect_filled(
                                            egui::Rect::from_min_size(
                                                rect.min,
                                                egui::Vec2::new(rect.width(), 2.0),
                                            ),
                                            0.0,
                                            ACCENT_COLOR,
                                        );
                                    } else {
                                        // Separator line at bottom for inactive tabs implies the active one overlaps
                                        // But here we can just do nothing for now, simplest flat look
                                    }

                                    // Content Layout
                                    // Modified dot or Icon
                                    let mut text_x = rect.left() + TAB_PADDING_X;
                                    if modified {
                                        // A prettier "unsaved" circle
                                        painter.circle_filled(
                                            egui::Pos2::new(text_x + 3.0, rect.center().y),
                                            4.0,
                                            egui::Color32::WHITE, // Or generic fg
                                        );
                                        // And a smaller inner dot for "modified"
                                        painter.circle_filled(
                                            egui::Pos2::new(text_x + 3.0, rect.center().y),
                                            2.5,
                                            ACCENT_COLOR,
                                        );
                                        text_x += 14.0;
                                    } else {
                                        // Maybe an icon here? For now just text.
                                    }

                                    // File Name
                                    painter.text(
                                        egui::Pos2::new(text_x, rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        text,
                                        font.clone(),
                                        text_color,
                                    );

                                    // Interactions
                                    if response.clicked() {
                                        tab_action = Some(TabAction::Activate(i));
                                    }
                                    if response.middle_clicked() {
                                        tab_action = Some(TabAction::Close(i));
                                    }

                                    // Close Button - Show on hover or if active
                                    if response.hovered() || is_active {
                                        let close_rect = egui::Rect::from_min_size(
                                            egui::Pos2::new(
                                                rect.right() - TAB_CLOSE_SIZE - 6.0,
                                                rect.center().y - TAB_CLOSE_SIZE / 2.0,
                                            ),
                                            egui::Vec2::new(TAB_CLOSE_SIZE, TAB_CLOSE_SIZE),
                                        );

                                        // Check interaction specifically on the small close rect
                                        let close_resp = ui.interact(
                                            close_rect,
                                            ui.id().with(("tab_close", i)),
                                            egui::Sense::click(),
                                        );

                                        let close_hovered = close_resp.hovered();

                                        // Draw 'x'
                                        // Rotate 45deg +
                                        let center = close_rect.center();
                                        if close_hovered {
                                            painter.rect_filled(
                                                close_rect,
                                                2.0,
                                                egui::Color32::from_white_alpha(30),
                                            );
                                        }

                                        let stroke = egui::Stroke::new(
                                            1.0,
                                            if close_hovered {
                                                egui::Color32::WHITE
                                            } else {
                                                egui::Color32::from_gray(150)
                                            },
                                        );
                                        // Manually draw X for better control than text
                                        let r = TAB_CLOSE_SIZE / 3.0;
                                        painter.line_segment(
                                            [
                                                center + egui::Vec2::new(-r, -r),
                                                center + egui::Vec2::new(r, r),
                                            ],
                                            stroke,
                                        );
                                        painter.line_segment(
                                            [
                                                center + egui::Vec2::new(-r, r),
                                                center + egui::Vec2::new(r, -r),
                                            ],
                                            stroke,
                                        );

                                        if close_resp.clicked() {
                                            tab_action = Some(TabAction::Close(i));
                                        }
                                    }

                                    // Context Menu
                                    response.context_menu(|ui| {
                                        if ui.button("Close").clicked() {
                                            tab_action = Some(TabAction::Close(i));
                                            ui.close_menu();
                                        }
                                        if ui.button("Close Others").clicked() {
                                            tab_action = Some(TabAction::CloseOthers(i));
                                            ui.close_menu();
                                        }
                                        if ui.button("Reopen Closed Tab").clicked() {
                                            tab_action = Some(TabAction::ReopenClosed);
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        let pin_label = if pinned { "Unpin" } else { "Pin" };
                                        if ui.button(pin_label).clicked() {
                                            tab_action = Some(TabAction::TogglePin(i, !pinned));
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Move to Primary Split").clicked() {
                                            tab_action = Some(TabAction::MoveToSplit(i, false));
                                            ui.close_menu();
                                        }
                                        if ui.button("Move to Secondary Split").clicked() {
                                            tab_action = Some(TabAction::MoveToSplit(i, true));
                                            ui.close_menu();
                                        }
                                    });

                                    if response.drag_started() {
                                        self.dragging_tab = Some(i);
                                    }
                                }

                                // New Tab Button (small)
                                let new_tab_resp = ui.allocate_response(
                                    egui::Vec2::new(32.0, TAB_HEIGHT),
                                    egui::Sense::click(),
                                );
                                if new_tab_resp.hovered() {
                                    ui.painter()
                                        .rect_filled(new_tab_resp.rect, 0.0, TAB_HOVER_BG);
                                }
                                ui.painter().text(
                                    new_tab_resp.rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "+",
                                    egui::FontId::proportional(16.0),
                                    egui::Color32::GRAY,
                                );
                                if new_tab_resp.clicked() {
                                    tab_action = Some(TabAction::NewTab);
                                }
                            });
                        });

                    if self.split_mode != SplitMode::None || self.dragging_tab.is_some() {
                        ui.add_space(8.0);
                        let dragging = self.dragging_tab.is_some();
                        let (primary_rect, primary_resp) = ui.allocate_exact_size(
                            egui::Vec2::new(82.0, TAB_HEIGHT - 6.0),
                            egui::Sense::hover(),
                        );
                        primary_drop_rect = Some(primary_rect);
                        let primary_hovered = pointer_pos
                            .map(|pos| primary_rect.contains(pos))
                            .unwrap_or(false);
                        let primary_bg = if dragging && primary_hovered {
                            ACCENT_COLOR
                        } else {
                            TAB_INACTIVE_BG
                        };
                        ui.painter().rect_filled(primary_rect, 4.0, primary_bg);
                        ui.painter().text(
                            primary_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Primary",
                            egui::FontId::proportional(11.5),
                            egui::Color32::from_gray(220),
                        );
                        primary_resp.on_hover_text("Drop tab to move into primary split");

                        ui.add_space(4.0);
                        let (secondary_rect, secondary_resp) = ui.allocate_exact_size(
                            egui::Vec2::new(92.0, TAB_HEIGHT - 6.0),
                            egui::Sense::hover(),
                        );
                        secondary_drop_rect = Some(secondary_rect);
                        let secondary_hovered = pointer_pos
                            .map(|pos| secondary_rect.contains(pos))
                            .unwrap_or(false);
                        let secondary_bg = if dragging && secondary_hovered {
                            ACCENT_COLOR
                        } else {
                            TAB_INACTIVE_BG
                        };
                        ui.painter().rect_filled(secondary_rect, 4.0, secondary_bg);
                        ui.painter().text(
                            secondary_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Secondary",
                            egui::FontId::proportional(11.5),
                            egui::Color32::from_gray(220),
                        );
                        secondary_resp.on_hover_text("Drop tab to move into secondary split");
                    }
                });

                if pointer_released {
                    if let (Some(drag_idx), Some(pos)) = (self.dragging_tab, pointer_pos) {
                        let mut dropped_in_split_target = false;
                        if let Some(rect) = primary_drop_rect {
                            if rect.contains(pos) {
                                tab_action = Some(TabAction::MoveToSplit(drag_idx, false));
                                dropped_in_split_target = true;
                            }
                        }
                        if !dropped_in_split_target {
                            if let Some(rect) = secondary_drop_rect {
                                if rect.contains(pos) {
                                    tab_action = Some(TabAction::MoveToSplit(drag_idx, true));
                                    dropped_in_split_target = true;
                                }
                            }
                        }
                        if !dropped_in_split_target {
                            if let Some(target_idx) = find_drop_target(drag_idx, pos, &tab_rects) {
                                tab_action = Some(TabAction::Reorder(drag_idx, target_idx));
                            }
                        }
                    }
                    self.dragging_tab = None;
                }

                if let Some(action) = tab_action {
                    match action {
                        TabAction::Activate(idx) => self.active_tab = idx,
                        TabAction::Close(idx) => self.close_tab_idx(idx),
                        TabAction::CloseOthers(idx) => self.close_other_tabs(idx),
                        TabAction::ReopenClosed => self.reopen_closed_tab(),
                        TabAction::TogglePin(idx, pinned) => self.pin_tab(idx, pinned),
                        TabAction::Reorder(from, to) => self.move_tab(from, to),
                        TabAction::MoveToSplit(idx, secondary) => {
                            self.move_tab_to_split(idx, secondary)
                        }
                        TabAction::NewTab => self.new_tab(),
                    }
                }
            });
    }
}
