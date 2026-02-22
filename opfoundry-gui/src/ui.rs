use bevy::ecs::system::{NonSendMut, Res, ResMut};
use bevy::prelude::{Handle, Image, Query, Resource, Sprite, Transform, Visibility};
use bevy_egui::{egui, EguiContexts};
use bus::interrupts::InterruptSnapshot;
use bus::mmio::SystemReg;

use crate::console_layout_job;
use crate::input::{
    render_controller_pad, render_keyboard_section, ControllerState, KeyboardTracker,
};
use crate::interrupts::InterruptBindings;
use crate::{
    sprite_world_transform, DisplayPalette, DisplaySettings, EmulatorState, ProgramSource,
    SpriteCatalog, SpriteSlot, SpriteViewport, SpriteVirtualResolution,
};

#[cfg(all(feature = "native-file-dialog", not(target_arch = "wasm32")))]
use rfd::FileDialog;

#[cfg(feature = "native-service")]
use crate::service_listener::ServiceListener;

fn interrupt_row(
    ui: &mut egui::Ui,
    name: &str,
    mask: Option<u32>,
    snapshot: &InterruptSnapshot,
    is_nmi: bool,
) {
    if let Some(mask) = mask {
        let pending = if is_nmi {
            snapshot.nmi_pending
        } else {
            snapshot.irq_pending
        };
        let enabled = if is_nmi {
            snapshot.nmi_enabled
        } else {
            snapshot.irq_enabled
        };
        let line = if is_nmi {
            snapshot.nmi_line
        } else {
            snapshot.irq_line
        };
        let pending_set = (pending & mask) != 0;
        let enabled_set = (enabled & mask) != 0;
        ui.horizontal(|ui| {
            ui.label(name);
            ui.label(format!(
                "mask=0x{mask:08X} pending={} enabled={} line={}",
                pending_set, enabled_set, line
            ));
        });
    }
}

#[derive(Resource)]
pub(super) struct UiState {
    pub(super) status: Option<String>,
    pub(super) console_open: bool,
    pub(super) bridge_connected: Option<bool>,
    pub(super) active_tab: ConsoleTab,
}

impl UiState {
    pub(super) fn with_status(status: Option<String>) -> Self {
        Self {
            status,
            console_open: true,
            bridge_connected: None,
            active_tab: ConsoleTab::Console,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ConsoleTab {
    Console,
    Interrupts,
    Input,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn ui_system(
    mut contexts: EguiContexts,
    #[allow(unused_mut)] mut emulator: NonSendMut<EmulatorState>,
    mut ui_state: ResMut<UiState>,
    mut controller_state: ResMut<ControllerState>,
    sprite_catalog: Res<SpriteCatalog>,
    sprite_viewport: Res<SpriteViewport>,
    sprite_virtual: Res<SpriteVirtualResolution>,
    display: Res<DisplaySettings>,
    mut palette: ResMut<DisplayPalette>,
    mut sprite_query: Query<(
        &SpriteSlot,
        &mut Transform,
        &mut Visibility,
        &mut Sprite,
        &mut Handle<Image>,
    )>,
    keyboard_tracker: Res<KeyboardTracker>,
    bindings: Option<Res<InterruptBindings>>,
    #[cfg(feature = "native-service")] service_listener: Option<Res<ServiceListener>>,
    #[cfg(target_arch = "wasm32")] mut web_service: Option<
        NonSendMut<crate::web::WebSocketBridgeManager>,
    >,
) {
    #[cfg(feature = "native-service")]
    if let Some(listener) = service_listener {
        while let Ok(envelope) = listener.receiver.try_recv() {
            let response = emulator.handle_service_command(envelope.command);
            ui_state.status = Some(response.message.clone());
            let _ = envelope.respond_to.send(response);
            controller_state.sync_backend(emulator.input_backend(), emulator.input_snapshot());
        }
    }

    #[cfg(target_arch = "wasm32")]
    let mut wasm_bridge_connected = false;

    #[cfg(target_arch = "wasm32")]
    if let Some(mut manager) = web_service.as_deref_mut() {
        manager.ensure_connected();
        if let Some(service) = manager.bridge_mut() {
            wasm_bridge_connected = true;
            for pending in service.drain_commands() {
                let mut response = emulator.handle_service_command(pending.command);
                ui_state.status = Some(response.message.clone());
                if let Some(id) = pending.bridge_id {
                    response.bridge_id = Some(id);
                }
                service.send_response(response);
                controller_state.sync_backend(emulator.input_backend(), emulator.input_snapshot());
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    for (data, name) in crate::web::drain_pending_files() {
        let source = ProgramSource::Inline { name, data };
        let result = emulator.run_program(source, None, None, None, None);
        ui_state.status = Some(match result {
            Ok(msg) => msg,
            Err(err) => err,
        });
        controller_state.sync_backend(emulator.input_backend(), emulator.input_snapshot());
    }

    #[cfg(target_arch = "wasm32")]
    {
        ui_state.bridge_connected = Some(wasm_bridge_connected);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        ui_state.bridge_connected = None;
    }

    let ctx = contexts.ctx_mut();

    egui::TopBottomPanel::top("top_panel")
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                #[cfg(target_arch = "wasm32")]
                {
                    if ui.button("Load PRG...").clicked() {
                        crate::web::request_file_dialog();
                    }
                }

                #[cfg(all(feature = "native-file-dialog", not(target_arch = "wasm32")))]
                {
                    if ui.button("Load PRG...").clicked() {
                        if let Some(path) = FileDialog::new()
                            .add_filter("PRG/BIN", &["prg", "bin"])
                            .pick_file()
                        {
                            let result = emulator.run_program(
                                ProgramSource::File(path.clone()),
                                None,
                                None,
                                None,
                                None,
                            );
                            ui_state.status = Some(match result {
                                Ok(msg) => msg,
                                Err(err) => err,
                            });
                            controller_state
                                .sync_backend(emulator.input_backend(), emulator.input_snapshot());
                        }
                    }
                }

                #[cfg(all(not(target_arch = "wasm32"), not(feature = "native-file-dialog")))]
                {
                    ui.add_enabled(false, egui::Button::new("Load PRG..."));
                }

                if let Some(status) = &ui_state.status {
                    ui.label(status);
                }

                #[cfg(target_arch = "wasm32")]
                if let Some(connected) = ui_state.bridge_connected {
                    let text = if connected {
                        "Bridge: connected"
                    } else {
                        "Bridge: offline"
                    };
                    ui.label(text);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let toggle_label = if ui_state.console_open {
                        "Hide Developer Tools"
                    } else {
                        "Show Developer Tools"
                    };
                    if ui.button(toggle_label).clicked() {
                        ui_state.console_open = !ui_state.console_open;
                    }
                });
            });
        });

    let console_snapshot = emulator.snapshot();
    let display_snapshot = emulator.display_snapshot();
    let sprite_snapshot = emulator.sprite_snapshot();

    if let Some(snapshot) = display_snapshot.as_ref() {
        palette.apply_snapshot(snapshot, &display);
    } else {
        *palette = DisplayPalette::from_settings(&display);
    }
    for (slot, mut transform, mut visibility, mut sprite, mut texture) in sprite_query.iter_mut() {
        let state = sprite_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.sprite(slot.index));
        if let Some(state) = state {
            if state.number == 0 {
                *visibility = Visibility::Hidden;
                continue;
            }
            if let Some((position, size)) =
                sprite_world_transform(state, &sprite_viewport, &sprite_virtual, &display)
            {
                *visibility = Visibility::Visible;
                transform.translation.x = position.x;
                transform.translation.y = position.y;
                sprite.custom_size = Some(size);
                let texture_index = (state.number.saturating_sub(1)) as usize;
                let desired_texture = sprite_catalog
                    .handles
                    .get(texture_index)
                    .cloned()
                    .or_else(|| sprite_catalog.handles.first().cloned());
                if let Some(handle) = desired_texture {
                    if *texture != handle {
                        *texture = handle;
                    }
                }
                sprite.color = bevy::prelude::Color::WHITE;
            } else {
                *visibility = Visibility::Hidden;
                continue;
            }
        } else {
            *visibility = Visibility::Hidden;
        }
    }

    egui::TopBottomPanel::bottom("console_panel")
        .resizable(true)
        .default_height(260.0)
        .min_height(160.0)
        .show_animated(ctx, ui_state.console_open, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.active_tab, ConsoleTab::Console, "Console");
                ui.selectable_value(
                    &mut ui_state.active_tab,
                    ConsoleTab::Interrupts,
                    "Interrupts",
                );
                ui.selectable_value(&mut ui_state.active_tab, ConsoleTab::Input, "Input");
            });

            ui.separator();

            match ui_state.active_tab {
                ConsoleTab::Console => {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if let Some(snapshot) = &console_snapshot {
                                let job = console_layout_job(snapshot);
                                ui.label(job);
                            } else {
                                ui.label("Console unavailable");
                            }
                        });
                }
                ConsoleTab::Interrupts => {
                    if let Some(bindings) = bindings.as_ref() {
                        let snapshot = bindings.snapshot();
                        ui.label(format!(
                            "IRQ: pending=0x{pending:08X} enabled=0x{enabled:08X} line={}",
                            snapshot.irq_line,
                            pending = snapshot.irq_pending,
                            enabled = snapshot.irq_enabled
                        ));
                        ui.label(format!(
                            "NMI: pending=0x{pending:08X} enabled=0x{enabled:08X} edge={} line={}",
                            snapshot.nmi_line,
                            snapshot.nmi_edge_latched,
                            pending = snapshot.nmi_pending,
                            enabled = snapshot.nmi_enabled
                        ));

                        ui.separator();
                        interrupt_row(ui, "frame_start", bindings.frame_start, &snapshot, true);
                        interrupt_row(ui, "frame_end", bindings.frame_end, &snapshot, false);
                        interrupt_row(ui, "timer0", bindings.timer0, &snapshot, false);
                        interrupt_row(ui, "keyboard_event", bindings.keyboard, &snapshot, false);
                        interrupt_row(ui, "gamepad_event", bindings.gamepad, &snapshot, false);
                        if let Ok(video_state) = emulator.video_state().lock() {
                            if let Some(raster) = video_state.register_value(SystemReg::RasterLo) {
                                ui.label(format!("Raster (lo): 0x{raster:02X}"));
                            }
                            if let Some(pending) = video_state.register_value(SystemReg::IrqPending)
                            {
                                ui.label(format!("IRQ Pending (VIC): 0x{pending:02X}"));
                            }
                            if let Some(enable) = video_state.register_value(SystemReg::IrqEnable) {
                                ui.label(format!("IRQ Enable (VIC): 0x{enable:02X}"));
                            }
                        }
                        let overlay_snapshot = emulator.video_overlay().snapshot();
                        ui.separator();
                        ui.label(format!("Raster line: {}", overlay_snapshot.raster));
                        ui.label(format!(
                            "Sprite collisions: 0x{:02X}",
                            overlay_snapshot.sprite_collisions
                        ));
                        ui.label(format!(
                            "Background collisions: 0x{:02X}",
                            overlay_snapshot.background_collisions
                        ));
                    } else {
                        ui.label("Interrupt bindings unavailable.");
                    }
                }
                ConsoleTab::Input => {
                    ui.heading("Keyboard");
                    render_keyboard_section(ui, &keyboard_tracker);
                    ui.separator();

                    if controller_state.has_backend() {
                        if let Some((snapshot, previous_snapshot)) =
                            controller_state.snapshot_pair()
                        {
                            if snapshot.pads.is_empty() {
                                ui.label("No controller data available.");
                            } else {
                                let labels: Vec<String> = (0..snapshot.pads.len())
                                    .map(|pad| controller_state.pad_gamepad_label(pad))
                                    .collect();
                                let _ = previous_snapshot;
                                ui.columns(snapshot.pads.len(), |columns| {
                                    for (offset, column) in columns.iter_mut().enumerate() {
                                        let pad_index = offset;
                                        let label = labels
                                            .get(pad_index)
                                            .map(|s| s.as_str())
                                            .unwrap_or("None");
                                        let modern = snapshot.modern.pads.get(pad_index);
                                        render_controller_pad(column, pad_index, label, modern);
                                    }
                                });
                            }
                        } else {
                            ui.label("Controller snapshot unavailable.");
                        }
                    } else {
                        ui.label("Controller adapter not attached for this personality.");
                    }
                }
            }
        });
}
