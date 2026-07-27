pub mod draw_list;
pub mod marking;
pub mod renderer;
pub mod search;

use ab_glyph::FontArc;
use softbuffer::{Context, Surface};
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId, WindowLevel},
};

use crate::config::SharedConfig;
use crate::matchers::Rect;
use draw_list::{OverlayMode, SharedDrawList};
use marking::MarkingOverlay;
use renderer::Renderer;
use search::SearchOverlay;

#[derive(Debug)]
pub struct RedrawTick;

pub fn run_overlay(
    draw_list: SharedDrawList,
    config: SharedConfig,
    assets: Arc<crate::shop::assets::ShopAssets>,
    cmd_rx: std::sync::mpsc::Receiver<crate::event_bus::AppCommand>,
) {
    let event_loop = EventLoop::<RedrawTick>::with_user_event()
        .build()
        .expect("failed to create event loop");

    let (render_delay_ms, is_marking) = {
        let cfg = config.lock().unwrap();
        let dl = draw_list.lock().unwrap();
        (cfg.render_delay_ms.max(10), dl.mode == OverlayMode::Marking)
    };

    event_loop.set_control_flow(ControlFlow::Wait);

    let proxy = event_loop.create_proxy();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(render_delay_ms));
            if proxy.send_event(RedrawTick).is_err() {
                break;
            }
        }
    });

    let initial_target_title = {
        let cfg = config.lock().unwrap();
        cfg.target_window_title.clone()
    };
    let initial_matcher = crate::config::TitleMatcher::new(&initial_target_title);

    let mut app = App {
        draw_list,
        config,
        font: assets.font.clone(),
        assets,
        cmd_rx,
        window: None,
        surface: None,
        is_drag_active: false,
        drag_start: (0, 0),
        last_cursor_pos: (0, 0),
        last_mode: if is_marking {
            OverlayMode::Marking
        } else {
            OverlayMode::Passive
        },
        cached_title: initial_target_title,
        title_matcher: initial_matcher,
    };

    event_loop
        .run_app(&mut app)
        .expect("event loop execution failed");
}

struct App {
    draw_list: SharedDrawList,
    config: SharedConfig,
    font: FontArc,
    assets: Arc<crate::shop::assets::ShopAssets>,
    cmd_rx: std::sync::mpsc::Receiver<crate::event_bus::AppCommand>,
    window: Option<Arc<Window>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    is_drag_active: bool,
    drag_start: (u32, u32),
    last_cursor_pos: (u32, u32),
    last_mode: OverlayMode,
    cached_title: String,
    title_matcher: crate::config::TitleMatcher,
}

impl App {
    fn process_commands(&mut self) {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd {
                crate::event_bus::AppCommand::ToggleOverlayMode(target_mode) => {
                    let mut dl = self.draw_list.lock().unwrap();
                    let new_mode = dl.mode.toggle(target_mode);
                    if new_mode == OverlayMode::Passive && dl.mode == OverlayMode::Search {
                        dl.search_query.clear();
                        dl.search_results.clear();
                    }
                    dl.mode = new_mode;
                    println!("[event_bus] toggled overlay mode to {:?}", new_mode);
                }

                crate::event_bus::AppCommand::ToggleDebugMode => {
                    let mut cfg = self.config.lock().unwrap();
                    cfg.debug_mode = !cfg.debug_mode;
                    crate::logger::set_debug(cfg.debug_mode);
                    println!("[event_bus] toggled debug_mode to {}", cfg.debug_mode);
                    let _ = cfg.save();
                }
                crate::event_bus::AppCommand::ToggleNgPlusMode => {
                    let mut cfg = self.config.lock().unwrap();
                    cfg.ng_plus_mode = !cfg.ng_plus_mode;
                    println!("[event_bus] toggled ng_plus_mode to {}", cfg.ng_plus_mode);
                    let _ = cfg.save();
                }
                crate::event_bus::AppCommand::RequestRedetect => {
                    println!("[event_bus] triggering on-demand window redetection...");
                    let mut dl = self.draw_list.lock().unwrap();
                    dl.item_cards.clear();
                    dl.debug_rects.clear();
                    dl.redetect_requested = true;
                }
                crate::event_bus::AppCommand::DumpTemplates => {
                    println!("[event_bus] dumping loaded asset templates...");
                    if let Err(e) = self.assets.dump_templates() {
                        eprintln!("[event_bus] error dumping templates: {e}");
                    }
                }
                crate::event_bus::AppCommand::SetOverlayMode(m) => {
                    let mut dl = self.draw_list.lock().unwrap();
                    dl.mode = m;
                }
                crate::event_bus::AppCommand::SaveMarkedSlots => {
                    let dl = self.draw_list.lock().unwrap();
                    let mut cfg = self.config.lock().unwrap();
                    cfg.marked_slots = dl
                        .marked_slots
                        .iter()
                        .map(|r| crate::config::CustomSlotConfig {
                            x: r.x,
                            y: r.y,
                            width: r.width,
                            height: r.height,
                        })
                        .collect();
                    let _ = cfg.save();
                }

                crate::event_bus::AppCommand::ClearMarkedSlots => {
                    let mut dl = self.draw_list.lock().unwrap();
                    dl.marked_slots.clear();
                    let mut cfg = self.config.lock().unwrap();
                    cfg.marked_slots.clear();
                    let _ = cfg.save();
                }
                crate::event_bus::AppCommand::AddMarkedSlot(rect) => {
                    let mut dl = self.draw_list.lock().unwrap();
                    dl.marked_slots.push(rect);
                }
                crate::event_bus::AppCommand::RemoveLastMarkedSlot => {
                    let mut dl = self.draw_list.lock().unwrap();
                    dl.marked_slots.pop();
                }
                crate::event_bus::AppCommand::UpdateCurrentDrag(drag) => {
                    let mut dl = self.draw_list.lock().unwrap();
                    dl.current_drag = drag;
                }
                crate::event_bus::AppCommand::UpdateSearchQuery(q) => {
                    let mut dl = self.draw_list.lock().unwrap();
                    dl.search_query = q;
                }
                crate::event_bus::AppCommand::Exit => {
                    println!("[event_bus] exit command received. Exiting application...");
                    std::process::exit(0);
                }
            }
        }
    }
}

impl ApplicationHandler<RedrawTick> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .expect("no monitor found");

        let size = monitor.size();

        let attrs = WindowAttributes::default()
            .with_title("Moonlighter Overlay")
            .with_inner_size(size)
            .with_position(winit::dpi::PhysicalPosition::new(0, 0))
            .with_decorations(false)
            .with_transparent(true)
            .with_active(false)
            .with_window_level(WindowLevel::AlwaysOnTop);

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        #[cfg(target_os = "windows")]
        set_click_through(&window, true);

        let ctx = Context::new(Arc::clone(&window)).expect("softbuffer ctx creation failed");
        let mut surface = Surface::new(&ctx, Arc::clone(&window)).expect("surface creation failed");
        surface
            .resize(
                size.width.try_into().unwrap(),
                size.height.try_into().unwrap(),
            )
            .ok();

        self.surface = Some(surface);
        self.window = Some(window);
    }

    fn user_event(&mut self, _: &ActiveEventLoop, _: RedrawTick) {
        self.process_commands();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        self.process_commands();
        let current_mode = {
            let dl = self.draw_list.lock().unwrap();
            dl.mode
        };

        if current_mode != self.last_mode {
            self.last_mode = current_mode;
            if let Some(w) = &self.window {
                #[cfg(target_os = "windows")]
                set_click_through(w, current_mode != OverlayMode::Marking);
            }
        }

        match event {
            WindowEvent::CloseRequested => el.exit(),

            WindowEvent::KeyboardInput { event, .. } if current_mode == OverlayMode::Marking => {
                if event.state == ElementState::Pressed {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Enter) => {
                            MarkingOverlay::save_marked_slots(&self.draw_list, &self.config);
                            let mut dl = self.draw_list.lock().unwrap();
                            dl.mode = OverlayMode::Passive;
                        }
                        PhysicalKey::Code(KeyCode::Escape) => {
                            let mut dl = self.draw_list.lock().unwrap();
                            dl.mode = OverlayMode::Passive;
                        }
                        PhysicalKey::Code(KeyCode::KeyC) => {
                            let mut dl = self.draw_list.lock().unwrap();
                            dl.marked_slots.clear();
                        }
                        _ => {}
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                let (cx, cy) = (position.x as u32, position.y as u32);
                self.last_cursor_pos = (cx, cy);
                if current_mode == OverlayMode::Marking && self.is_drag_active {
                    let mut dl = self.draw_list.lock().unwrap();
                    dl.current_drag = Some((self.drag_start.0, self.drag_start.1, cx, cy));
                }
            }

            WindowEvent::MouseInput { state, button, .. }
                if current_mode == OverlayMode::Marking =>
            {
                if button == MouseButton::Left {
                    if state == ElementState::Pressed {
                        self.is_drag_active = true;
                        self.drag_start = self.last_cursor_pos;
                    } else if state == ElementState::Released && self.is_drag_active {
                        self.is_drag_active = false;
                        let mut dl = self.draw_list.lock().unwrap();
                        if let Some((sx, sy, cx, cy)) = dl.current_drag.take() {
                            let min_x = sx.min(cx);
                            let min_y = sy.min(cy);
                            let w = sx.abs_diff(cx);
                            let h = sy.abs_diff(cy);
                            if w > 10 && h > 10 {
                                dl.marked_slots.push(Rect::new(min_x, min_y, w, h));
                            }
                        }
                    }
                } else if button == MouseButton::Right && state == ElementState::Pressed {
                    let mut dl = self.draw_list.lock().unwrap();
                    dl.marked_slots.pop();
                }
            }

            WindowEvent::RedrawRequested => {
                let (Some(surface), Some(window)) = (self.surface.as_mut(), self.window.as_ref())
                else {
                    return;
                };

                let size = window.inner_size();
                let w = size.width as usize;
                let h = size.height as usize;

                surface
                    .resize(
                        size.width.try_into().unwrap(),
                        size.height.try_into().unwrap(),
                    )
                    .ok();

                let mut buf = surface.buffer_mut().expect("surface buffer error");
                buf.fill(0x00_00_00_00);

                let mut renderer = Renderer::new(&mut buf, w, h, &self.font);

                let (target_title, is_debug) = {
                    let cfg = self.config.lock().unwrap();
                    (cfg.target_window_title.clone(), cfg.debug_mode)
                };

                if self.cached_title != target_title {
                    self.title_matcher = crate::config::TitleMatcher::new(&target_title);
                    self.cached_title = target_title;
                }

                let is_fg = crate::hotkeys::is_target_foreground_matcher(&self.title_matcher);

                if is_fg {
                    let state = {
                        let dl = self.draw_list.lock().unwrap();
                        dl.clone()
                    };

                    if state.mode == OverlayMode::Search {
                        SearchOverlay::render(&mut renderer, &state);
                    } else if state.mode == OverlayMode::Marking {
                        MarkingOverlay::render(&mut renderer, &state);
                    } else {
                        if is_debug {
                            for rect in &state.debug_rects {
                                renderer.draw_debug_rect(rect);
                            }
                        }

                        for card in &state.item_cards {
                            renderer.draw_item_card(card);
                        }
                    }
                }

                buf.present().ok();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {}
}

#[cfg(target_os = "windows")]
fn set_click_through(window: &Window, enable: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_APPWINDOW, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if let Ok(handle) = window.window_handle()
        && let RawWindowHandle::Win32(h) = handle.as_raw()
    {
        let hwnd = windows::Win32::Foundation::HWND(h.hwnd.get() as _);
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let mut new_style = (style | WS_EX_TOOLWINDOW.0 as isize | WS_EX_NOACTIVATE.0 as isize)
                & !(WS_EX_APPWINDOW.0 as isize);
            if enable {
                new_style |= WS_EX_LAYERED.0 as isize | WS_EX_TRANSPARENT.0 as isize;
            } else {
                new_style &= !(WS_EX_TRANSPARENT.0 as isize);
            }
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        }
    }
}
