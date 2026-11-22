//! Interactive Chromium surfaces drawn inside the native D.A.D window.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use egui::{Color32, Context, Event, Key, Modifiers, PointerButton, Pos2, Rect, TextureHandle, vec2};

use super::{chrome::Chrome, livecdp::LiveTab};

const WIDTH: u32 = 1000;
const HEIGHT: u32 = 680;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Surface { Student, Transfer }

impl Surface {
    pub fn title(self) -> &'static str {
        match self {
            Self::Student => "Student preview",
            Self::Transfer => "MyOpenMath browser",
        }
    }

    fn profile(self) -> &'static str {
        match self {
            Self::Student => "student-browser-panel",
            Self::Transfer => "instructor-browser-panel",
        }
    }

    fn session(self) -> &'static Mutex<Option<Session>> {
        match self { Self::Student => &STUDENT, Self::Transfer => &TRANSFER }
    }
}

struct Session {
    chrome: Chrome,
    tab: Option<LiveTab>,
    _temporary_profile: Option<tempfile::TempDir>,
}

static STUDENT: Mutex<Option<Session>> = Mutex::new(None);
static TRANSFER: Mutex<Option<Session>> = Mutex::new(None);
static MANUAL_SIGN_IN: AtomicBool = AtomicBool::new(false);
static TRANSFER_OPEN: AtomicBool = AtomicBool::new(false);
static STOPPING: AtomicBool = AtomicBool::new(false);

pub fn manual_sign_in() -> bool { MANUAL_SIGN_IN.load(Ordering::Relaxed) }

pub fn transfer_open() -> bool { TRANSFER_OPEN.load(Ordering::Relaxed) }

pub fn set_manual_sign_in(active: bool) {
    MANUAL_SIGN_IN.store(active, Ordering::Relaxed);
}

pub fn disconnected(surface: Surface) -> bool {
    surface.session().try_lock().ok().and_then(|slot| slot.as_ref()?.tab.clone())
        .is_some_and(|tab| !tab.alive())
}

/// Called only on a worker. The UI uses try_lock so Chrome startup cannot block drawing.
pub fn tab(surface: Surface, dpr: f64) -> Result<LiveTab, String> {
    let mut slot = surface.session().lock().map_err(|_| "Could not lock the browser panel.")?;
    if STOPPING.load(Ordering::Relaxed) { return Err("D.A.D is closing.".into()); }
    if !slot.as_mut().is_some_and(|s| s.chrome.alive()) {
        let (chrome, temporary_profile) = match surface {
            Surface::Student => {
                let profile = tempfile::Builder::new().prefix("dad-student-").tempdir()
                    .map_err(|error| format!("Could not create the student browser profile: {error}"))?;
                (Chrome::launch_student_preview(profile.path())?, Some(profile))
            }
            Surface::Transfer => {
                let profile = crate::store::store_dir().join("mom").join(surface.profile());
                (Chrome::launch(&profile, true)?, None)
            }
        };
        *slot = Some(Session { chrome, tab: None, _temporary_profile: temporary_profile });
    }
    let session = slot.as_mut().unwrap();
    if let Some(tab) = session.tab.as_ref().filter(|t| t.alive()) {
        return Ok(tab.clone());
    }
    let tab = LiveTab::connect(&session.chrome.tab_url()?)?;
    tab.start_screencast(1920, 1200, dpr.min(2.0))?;
    session.tab = Some(tab.clone());
    Ok(tab)
}

pub fn stop() {
    STOPPING.store(true, Ordering::Relaxed);
    for slot in [&STUDENT, &TRANSFER] {
        if let Some(session) = slot.lock().ok().and_then(|mut slot| slot.take()) {
            session.chrome.close();
        }
    }
}

#[derive(Default)]
pub struct Panel {
    surface: Option<Surface>,
    texture: Option<TextureHandle>,
    frame: u64,
    connection: Option<LiveTab>,
    button_down: bool,
    size: Option<[u32; 2]>,
    focus_browser: bool,
}

impl Panel {
    pub fn open(&mut self, surface: Surface) {
        self.close();
        if surface == Surface::Transfer { TRANSFER_OPEN.store(true, Ordering::Relaxed); }
        *self = Self { surface: Some(surface), focus_browser: true, ..Self::default() };
    }

    pub fn surface(&self) -> Option<Surface> { self.surface }

    pub fn has_page(&self) -> bool { self.texture.is_some() }

    pub fn close(&mut self) {
        if self.surface == Some(Surface::Transfer) {
            TRANSFER_OPEN.store(false, Ordering::Relaxed);
            set_manual_sign_in(false);
        }
        if self.button_down && let Some(tab) = &self.connection {
            tab.mouse("up", 0.0, 0.0, "left", 1);
        }
        *self = Self::default();
    }

    pub fn show(&mut self, ui: &mut egui::Ui, busy: bool, status: Option<&str>) {
        let Some(surface) = self.surface else { return };
        let ctx = ui.ctx().clone();
        let browser = surface.session().try_lock().ok().and_then(|slot| slot.as_ref()?.tab.clone());
        if let Some(tab) = &browser {
            if !self.connection.as_ref().is_some_and(|old| old.same_connection(tab)) {
                self.texture = None;
                self.frame = 0;
                self.size = None;
                self.connection = Some(tab.clone());
            }
            let (number, frame) = tab.latest_frame();
            if number != self.frame {
                self.frame = number;
                if let Some(frame) = frame
                    && let Ok(image) = image::load_from_memory_with_format(&frame.jpeg, image::ImageFormat::Jpeg)
                {
                    let rgba = image.into_rgba8();
                    let pixels = egui::ColorImage::from_rgba_unmultiplied(
                        [rgba.width() as usize, rgba.height() as usize], rgba.as_raw(),
                    );
                    if let Some(texture) = &mut self.texture {
                        texture.set(pixels, egui::TextureOptions::LINEAR);
                    } else {
                        self.texture = Some(ctx.load_texture(surface.title(), pixels, egui::TextureOptions::LINEAR));
                    }
                }
            }
        }

        if let Some(status) = status { ui.label(status); }
        let available = ui.available_size().max(vec2(1.0, 1.0));
        let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        let size = [rect.width().round().max(1.0) as u32, rect.height().round().max(1.0) as u32];
        if let Some(tab) = &browser && self.size != Some(size) {
            self.size = Some(size);
            self.texture = None;
            tab.fire("Emulation.setDeviceMetricsOverride", serde_json::json!({
                "width": size[0], "height": size[1],
                "deviceScaleFactor": ctx.pixels_per_point().clamp(1.0, 2.0), "mobile": false
            }));
        }
        ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
        let interactive = !busy || (surface == Surface::Transfer && MANUAL_SIGN_IN.load(Ordering::Relaxed));
        if let Some(texture) = &self.texture {
            ui.painter().image(texture.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            if interactive && let Some(tab) = browser.as_ref().filter(|t| t.alive()) {
                if std::mem::take(&mut self.focus_browser) { response.request_focus(); }
                self.forward_input(&ctx, rect, &response, tab);
            }
        } else {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                if browser.as_ref().is_some_and(|tab| !tab.alive()) { "Browser disconnected" } else { "Opening browser…" },
                egui::FontId::proportional(15.0), Color32::DARK_GRAY);
        }
        ctx.request_repaint_after(Duration::from_millis(33));
    }

    fn forward_input(&mut self, ctx: &Context, rect: Rect, response: &egui::Response, tab: &LiveTab) {
        let (pointer, events) = ctx.input(|i| (i.pointer.hover_pos(), i.events.clone()));
        for event in events {
            match event {
                Event::PointerButton { pos, button: PointerButton::Primary, pressed, .. }
                    if (response.contains_pointer() && rect.contains(pos)) || (!pressed && self.button_down) =>
                {
                    if pressed { response.request_focus(); }
                    let (x, y) = page_point(rect, pos, self.size.unwrap_or([WIDTH, HEIGHT]));
                    tab.mouse(if pressed { "down" } else { "up" }, x, y, "left", 1);
                    self.button_down = pressed;
                }
                Event::PointerMoved(pos) if response.contains_pointer() || self.button_down => {
                    let (x, y) = page_point(rect, pos, self.size.unwrap_or([WIDTH, HEIGHT]));
                    tab.mouse("move", x, y, "left", 0);
                }
                Event::MouseWheel { delta, unit, .. } if response.contains_pointer() => {
                    if let Some(pos) = pointer {
                        let (x, y) = page_point(rect, pos, self.size.unwrap_or([WIDTH, HEIGHT]));
                        let scale = match unit {
                            egui::MouseWheelUnit::Point => 1.0,
                            egui::MouseWheelUnit::Line => 40.0,
                            egui::MouseWheelUnit::Page => rect.height(),
                        };
                        tab.wheel(x, y, (-delta.x * scale) as f64, (-delta.y * scale) as f64);
                    }
                }
                Event::Text(text) | Event::Paste(text) if response.has_focus() => tab.text(&text),
                Event::Key { key, pressed, modifiers, .. } if response.has_focus() => {
                    if let Some(key) = page_key(key, modifiers) {
                        let mask = u32::from(modifiers.alt)
                            | (u32::from(modifiers.ctrl) << 1)
                            | (u32::from(modifiers.mac_cmd) << 2)
                            | (u32::from(modifiers.shift) << 3);
                        tab.key(key, pressed, mask);
                        if key == "Tab" && pressed { response.request_focus(); }
                    }
                }
                _ => {}
            }
        }
        if response.has_focus() {
            ctx.memory_mut(|memory| memory.set_focus_lock_filter(response.id, egui::EventFilter {
                tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true,
            }));
        }
    }
}

fn page_point(rect: Rect, pos: Pos2, size: [u32; 2]) -> (f64, f64) {
    let x = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    let y = ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
    ((x * size[0] as f32) as f64, (y * size[1] as f32) as f64)
}

fn page_key(key: Key, modifiers: Modifiers) -> Option<&'static str> {
    Some(match key {
        Key::Enter => "Enter",
        Key::Backspace => "Backspace",
        Key::Tab => "Tab",
        Key::Space => "Space",
        Key::Delete => "Delete",
        Key::ArrowLeft => "ArrowLeft",
        Key::ArrowRight => "ArrowRight",
        Key::ArrowUp => "ArrowUp",
        Key::ArrowDown => "ArrowDown",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::A if modifiers.command => "a",
        Key::C if modifiers.command => "c",
        Key::X if modifiers.command => "x",
        Key::Z if modifiers.command => "z",
        _ => return None,
    })
}
