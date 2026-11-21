//! A DevTools connection to one Chrome tab that can do three things at once: answer commands, stream
//! the page as JPEG frames (`Page.startScreencast`), and take a person's mouse and keyboard input.
//! [`super::chrome::Tab`] is the simple request/response client the desktop uses; this one has a
//! thread that owns the socket, so frames keep flowing while a command waits.
//!
//! Commands and their answers are matched by id; screencast frames are acknowledged to Chrome at
//! once and kept as "latest frame", which the server sends to the viewer when the viewer asks for
//! the next one, so a slow connection gets fewer frames instead of a growing backlog.

use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

const CALL_TIMEOUT: Duration = Duration::from_secs(60);

/// The newest frame: JPEG bytes and the page's size in CSS pixels.
#[derive(Clone)]
pub struct Frame {
    pub jpeg: Vec<u8>,
    pub width: f64,
    pub height: f64,
}

struct Shared {
    pending: Mutex<HashMap<u64, Sender<Value>>>,
    latest: Mutex<Option<Frame>>,
    /// Bumped whenever a new frame arrives, so a waiting viewer can tell.
    frame_no: AtomicU64,
    alive: AtomicBool,
}

/// One tab, shared by the agent's steps and the viewer.
#[derive(Clone)]
pub struct LiveTab {
    out: Sender<String>,
    shared: Arc<Shared>,
    next: Arc<AtomicU64>,
}

impl LiveTab {
    /// Connects to a tab's DevTools WebSocket address and starts the socket thread.
    pub fn connect(ws_url: &str) -> Result<LiveTab, String> {
        let (ws, _) = tungstenite::connect(ws_url).map_err(|e| format!("Could not reach the Chrome tab: {e}"))?;
        if let MaybeTlsStream::Plain(s) = ws.get_ref() {
            let _ = s.set_read_timeout(Some(Duration::from_millis(20)));
        }
        let (out, rx) = channel::<String>();
        let shared = Arc::new(Shared {
            pending: Mutex::new(HashMap::new()),
            latest: Mutex::new(None),
            frame_no: AtomicU64::new(0),
            alive: AtomicBool::new(true),
        });
        let tab = LiveTab { out: out.clone(), shared: shared.clone(), next: Arc::new(AtomicU64::new(1_000_000)) };
        std::thread::spawn(move || pump(ws, rx, out, shared));
        tab.call("Page.enable", json!({}))?;
        tab.call("Runtime.enable", json!({}))?;
        Ok(tab)
    }

    pub fn alive(&self) -> bool { self.shared.alive.load(Ordering::Relaxed) }

    /// One command and its result.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = channel();
        self.shared.pending.lock().map_err(|_| "lock")?.insert(id, tx);
        self.out.send(json!({"id": id, "method": method, "params": params}).to_string()).map_err(|_| "The Chrome tab is gone")?;
        let v = rx.recv_timeout(CALL_TIMEOUT).map_err(|_| format!("Chrome didn't answer {method} in time"))?;
        if let Some(e) = v.get("error") {
            return Err(format!("{method}: {}", e["message"].as_str().unwrap_or("failed")));
        }
        Ok(v["result"].clone())
    }

    /// Sends a command without waiting (input events: their answers carry nothing).
    pub fn fire(&self, method: &str, params: Value) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let _ = self.out.send(json!({"id": id, "method": method, "params": params}).to_string());
    }

    pub fn eval(&self, js: &str) -> Result<Value, String> {
        let r = self.call("Runtime.evaluate", json!({"expression": js, "returnByValue": true, "awaitPromise": true}))?;
        if let Some(ex) = r.get("exceptionDetails") {
            let msg = ex["exception"]["description"].as_str().or(ex["text"].as_str()).unwrap_or("script error");
            return Err(format!("The page script failed: {}", msg.lines().next().unwrap_or(msg)));
        }
        Ok(r["result"]["value"].clone())
    }

    pub fn navigate(&self, url: &str) -> Result<(), String> {
        self.call("Page.navigate", json!({"url": url}))?;
        std::thread::sleep(Duration::from_millis(300));
        if self.wait_for("document.readyState === 'complete'", Duration::from_secs(30))? { Ok(()) } else { Err("The page took too long to load".into()) }
    }

    pub fn wait_for(&self, cond: &str, limit: Duration) -> Result<bool, String> {
        let start = Instant::now();
        loop {
            if !self.alive() { return Err("The browser closed".into()); }
            if let Ok(Value::Bool(true)) = self.eval(&format!("!!({cond})")) { return Ok(true); }
            if start.elapsed() > limit { return Ok(false); }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    pub fn url(&self) -> String {
        self.eval("location.href").ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
    }

    /// Starts streaming the page at `width`×`height` CSS pixels, rendered at the viewer's pixel
    /// density (`dpr`, 2 on most laptops), so text is drawn at the screen's own resolution instead
    /// of being stretched. Moving frames are lighter JPEGs; [`LiveTab::capture`] sends the sharp
    /// still once the page settles.
    pub fn start_screencast(&self, width: u32, height: u32, dpr: f64) -> Result<(), String> {
        let dpr = dpr.clamp(1.0, 3.0);
        self.call("Emulation.setDeviceMetricsOverride", json!({"width": width, "height": height, "deviceScaleFactor": dpr, "mobile": false}))?;
        let (mw, mh) = ((width as f64 * dpr).round(), (height as f64 * dpr).round());
        self.call("Page.startScreencast", json!({"format": "jpeg", "quality": 75, "maxWidth": mw, "maxHeight": mh, "everyNthFrame": 1}))?;
        Ok(())
    }

    /// The newest frame and its number (frames come only when the page changes).
    pub fn latest_frame(&self) -> (u64, Option<Frame>) {
        (self.shared.frame_no.load(Ordering::Relaxed), self.shared.latest.lock().ok().and_then(|f| f.clone()))
    }

    /// A full screenshot, for the first frame of a page that is not changing.
    pub fn capture(&self) -> Option<Frame> {
        let r = self.call("Page.captureScreenshot", json!({"format": "jpeg", "quality": 92})).ok()?;
        let jpeg = base64_decode(r["data"].as_str()?)?;
        let m = self.eval("[innerWidth, innerHeight]").ok()?;
        Some(Frame { jpeg, width: m[0].as_f64()?, height: m[1].as_f64()? })
    }

    // ---- a person's input (page CSS pixels) ----

    pub fn mouse(&self, kind: &str, x: f64, y: f64, button: &str, clicks: u32) {
        let kind = match kind { "down" => "mousePressed", "up" => "mouseReleased", _ => "mouseMoved" };
        let buttons = if kind == "mousePressed" { 1 } else { 0 };
        self.fire("Input.dispatchMouseEvent", json!({"type": kind, "x": x, "y": y, "button": if kind == "mouseMoved" { "none" } else { button }, "buttons": buttons, "clickCount": clicks}));
    }

    pub fn wheel(&self, x: f64, y: f64, dx: f64, dy: f64) {
        self.fire("Input.dispatchMouseEvent", json!({"type": "mouseWheel", "x": x, "y": y, "deltaX": dx, "deltaY": dy}));
    }

    /// Text as typed (letters, pasted text).
    pub fn text(&self, text: &str) {
        self.fire("Input.insertText", json!({"text": text}));
    }

    /// A key that is not text: Enter, Backspace, Tab, arrows, Escape, Delete, Home, End.
    pub fn key(&self, key: &str, down: bool, modifiers: u32) {
        let (code, vk) = match key {
            "Enter" => ("Enter", 13), "Backspace" => ("Backspace", 8), "Tab" => ("Tab", 9), "Escape" => ("Escape", 27),
            "Delete" => ("Delete", 46), "ArrowLeft" => ("ArrowLeft", 37), "ArrowUp" => ("ArrowUp", 38),
            "ArrowRight" => ("ArrowRight", 39), "ArrowDown" => ("ArrowDown", 40), "Home" => ("Home", 36), "End" => ("End", 35),
            "PageUp" => ("PageUp", 33), "PageDown" => ("PageDown", 34),
            k if k.len() == 1 => ("", k.to_ascii_uppercase().bytes().next().unwrap_or(0) as i64),
            _ => return,
        };
        let mut p = json!({"type": if down { "rawKeyDown" } else { "keyUp" }, "key": key, "windowsVirtualKeyCode": vk, "modifiers": modifiers});
        if !code.is_empty() { p["code"] = json!(code); }
        if down && key == "Enter" { p["text"] = json!("\r"); p["type"] = json!("keyDown"); }
        self.fire("Input.dispatchKeyEvent", p);
    }
}

/// The socket thread: sends queued commands, reads answers and events.
fn pump(mut ws: WebSocket<MaybeTlsStream<TcpStream>>, rx: Receiver<String>, out: Sender<String>, shared: Arc<Shared>) {
    let mut ack_id = 2_000_000_000u64;
    loop {
        while let Ok(msg) = rx.try_recv() {
            if ws.send(Message::text(msg)).is_err() { shared.alive.store(false, Ordering::Relaxed); return; }
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                let v: Value = serde_json::from_str(t.as_str()).unwrap_or(Value::Null);
                if let Some(id) = v["id"].as_u64() {
                    if let Some(tx) = shared.pending.lock().ok().and_then(|mut p| p.remove(&id)) { let _ = tx.send(v); }
                } else if v["method"] == "Page.screencastFrame" {
                    let p = &v["params"];
                    if let Some(jpeg) = p["data"].as_str().and_then(base64_decode) {
                        let m = &p["metadata"];
                        if let Ok(mut l) = shared.latest.lock() {
                            *l = Some(Frame { jpeg, width: m["deviceWidth"].as_f64().unwrap_or(0.0), height: m["deviceHeight"].as_f64().unwrap_or(0.0) });
                        }
                        shared.frame_no.fetch_add(1, Ordering::Relaxed);
                    }
                    ack_id += 1;
                    let _ = out.send(json!({"id": ack_id, "method": "Page.screencastFrameAck", "params": {"sessionId": p["sessionId"]}}).to_string());
                }
            }
            Ok(Message::Close(_)) => { shared.alive.store(false, Ordering::Relaxed); return; }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(_) => { shared.alive.store(false, Ordering::Relaxed); return; }
        }
    }
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(s).ok()
}
