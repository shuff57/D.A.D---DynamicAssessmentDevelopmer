//! The MyOpenMath helper as a server, for the web app: a headless Chrome per teacher, streamed to a
//! panel in the page, which the teacher can watch and take over at any time (modelled on T3 Code's
//! server browser).
//!
//! One WebSocket per viewer at `/browser?token=…`. Binary messages to the viewer are frames: 8 bytes
//! (the page's CSS width and height as f32, little-endian) and a JPEG; the viewer sends `ack` for
//! the next one, so only one frame is ever in flight. Text messages are JSON:
//!
//! - viewer → server: `hello {width,height}`, `ack`, `mouse {kind,x,y,button,clicks}`,
//!   `wheel {x,y,dx,dy}`, `key {key,down,modifiers}`, `text {text}`, `take`, `release`,
//!   `courses`, `assessments {cid}`, `add {cid,aid,question,save}`.
//! - server → viewer: `control {controller}` ("you", "helper" or "nobody"), `status {line}`,
//!   `courses {list}`, `assessments {cid,list}`, `added {ok,message}`, `error {message}`.
//!
//! Control: the helper's steps only act while it holds control. "Take control" stops it at its next
//! step; "Release control" lets it go on. When MyOpenMath needs signing in, control goes to the
//! teacher, and the helper carries on by itself once the page shows they are signed in. Each
//! teacher's Chrome profile (and so their MyOpenMath sign-in) lives under the store, named by a hash
//! of their access token, and is never shared.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::Message;

use super::chrome::Chrome;
use super::livecdp::LiveTab;
use super::poster::{self, Added, Page};
use super::MomQuestion;

/// Who may act on the page.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Controller { Nobody, You, Helper }

impl Controller {
    fn name(self) -> &'static str {
        match self { Controller::Nobody => "nobody", Controller::You => "you", Controller::Helper => "helper" }
    }
}

/// One teacher's browser.
struct Session {
    chrome: Mutex<Option<Chrome>>,
    tab: Mutex<Option<LiveTab>>,
    control: Mutex<Controller>,
    /// Where status lines and results go: the current viewer's outbox.
    viewer: Mutex<Option<Sender<String>>>,
    /// A helper job is running.
    busy: Mutex<bool>,
    profile: std::path::PathBuf,
}

impl Session {
    fn say(&self, v: Value) {
        if let Some(tx) = self.viewer.lock().ok().and_then(|v| v.clone()) { let _ = tx.send(v.to_string()); }
    }
    fn status(&self, line: &str) { self.say(json!({"type": "status", "line": line})); }
    fn set_control(&self, c: Controller) {
        if let Ok(mut g) = self.control.lock() { *g = c; }
        self.say(json!({"type": "control", "controller": c.name()}));
    }
    fn controller(&self) -> Controller { self.control.lock().map(|c| *c).unwrap_or(Controller::Nobody) }

    /// The tab, starting Chrome if needed.
    fn tab(&self) -> Result<LiveTab, String> {
        let mut t = self.tab.lock().map_err(|_| "lock")?;
        if let Some(tab) = t.as_ref().filter(|t| t.alive()) { return Ok(tab.clone()); }
        let mut c = self.chrome.lock().map_err(|_| "lock")?;
        if !c.as_mut().is_some_and(|c| c.alive()) {
            *c = Some(Chrome::launch(&self.profile, true)?);
        }
        let tab = LiveTab::connect(&c.as_ref().unwrap().tab_url()?)?;
        // Present as the ordinary Chrome it is, not "HeadlessChrome": some sign-in pages refuse that.
        if let Ok(Value::String(ua)) = tab.eval("navigator.userAgent") {
            let _ = tab.call("Emulation.setUserAgentOverride", json!({"userAgent": ua.replace("HeadlessChrome", "Chrome")}));
        }
        *t = Some(tab.clone());
        Ok(tab)
    }
}

/// The helper's steps on the page; acting waits while the teacher has control.
struct Gated { tab: LiveTab, session: Arc<Session> }

impl Gated {
    fn wait_turn(&self) -> Result<(), String> {
        let mut told = false;
        loop {
            match self.session.controller() {
                Controller::Helper => return Ok(()),
                Controller::Nobody => { self.session.set_control(Controller::Helper); return Ok(()); }
                Controller::You => {
                    if !told { self.session.status("Paused while you have control. Press Release control and the helper goes on."); told = true; }
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
            if !self.tab.alive() { return Err("The browser closed".into()); }
        }
    }
}

impl Page for Gated {
    fn navigate(&mut self, url: &str) -> Result<(), String> { self.wait_turn()?; self.tab.navigate(url) }
    fn eval(&mut self, js: &str) -> Result<Value, String> { self.wait_turn()?; self.tab.eval(js) }
    // Watching (waiting for a save, a sign-in) never needs control.
    fn wait_for(&mut self, cond: &str, limit: Duration) -> Result<bool, String> { self.tab.wait_for(cond, limit) }
    fn url(&mut self) -> String { self.tab.url() }
}

fn sessions() -> &'static Mutex<HashMap<String, Arc<Session>>> {
    static S: OnceLock<Mutex<HashMap<String, Arc<Session>>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

fn session_for(token: &str) -> Arc<Session> {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ("dad-browser", token).hash(&mut h);
    let key = format!("{:016x}", h.finish());
    let mut all = sessions().lock().unwrap();
    all.entry(key.clone()).or_insert_with(|| Arc::new(Session {
        chrome: Mutex::new(None), tab: Mutex::new(None), control: Mutex::new(Controller::Nobody),
        viewer: Mutex::new(None), busy: Mutex::new(false),
        profile: crate::store::store_dir().join("mom").join("server-browser").join(key),
    })).clone()
}

/// Serves viewers on `addr` (e.g. 127.0.0.1:8793). `tokens`: the access tokens allowed in.
pub fn serve(addr: &str, tokens: Vec<String>) -> Result<(), String> {
    let listener = TcpListener::bind(addr).map_err(|e| format!("Could not listen on {addr}: {e}"))?;
    eprintln!("MyOpenMath helper listening on ws://{addr}/browser");
    let tokens = Arc::new(tokens);
    for stream in listener.incoming().flatten() {
        let tokens = tokens.clone();
        std::thread::spawn(move || {
            if let Err(e) = viewer(stream, &tokens) { eprintln!("viewer: {e}"); }
        });
    }
    Ok(())
}

fn equal(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn viewer(stream: TcpStream, tokens: &[String]) -> Result<(), String> {
    let mut token = String::new();
    let mut ws = tungstenite::accept_hdr(stream, |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        let uri = req.uri();
        let t = uri.query().unwrap_or("").split('&').find_map(|kv| kv.strip_prefix("token=")).unwrap_or("").to_string();
        if uri.path() != "/browser" || !tokens.iter().any(|k| equal(k, &t)) {
            let mut r = ErrorResponse::new(Some("not allowed".into()));
            *r.status_mut() = tungstenite::http::StatusCode::FORBIDDEN;
            return Err(r);
        }
        token = t;
        Ok(resp)
    }).map_err(|e| e.to_string())?;
    let _ = ws.get_ref().set_read_timeout(Some(Duration::from_millis(15)));
    let session = session_for(&token);
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    *session.viewer.lock().unwrap() = Some(tx);
    let mut ready = true;
    let mut sent_frame = u64::MAX;
    let mut tab: Option<LiveTab> = None;
    // When the last moving frame went out, and whether the sharp still has followed it.
    let mut last_motion = std::time::Instant::now();
    let mut still_sent = true;
    loop {
        // What jobs and the session want the viewer to know.
        while let Ok(m) = rx.try_recv() {
            ws.send(Message::text(m)).map_err(|e| e.to_string())?;
        }
        // The newest frame, when the viewer is ready for one.
        if let (true, Some(t)) = (ready, tab.as_ref()) {
            let (no, frame) = t.latest_frame();
            if no != sent_frame {
                if let Some(f) = frame {
                    ws.send(Message::binary(frame_bytes(&f))).map_err(|e| e.to_string())?;
                    ready = false;
                    sent_frame = no;
                    last_motion = std::time::Instant::now();
                    still_sent = false;
                }
            } else if !still_sent && last_motion.elapsed() > Duration::from_millis(250) {
                // The page has settled: send it at full quality (as T3 Code does).
                if let Some(f) = t.capture() {
                    ws.send(Message::binary(frame_bytes(&f))).map_err(|e| e.to_string())?;
                    ready = false;
                }
                still_sent = true;
            }
        }
        let msg = match ws.read() {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
            Err(e) => return Err(e.to_string()),
        };
        let v: Value = serde_json::from_str(&msg).unwrap_or(Value::Null);
        let you = session.controller() == Controller::You;
        let t = tab.as_ref();
        match v["type"].as_str().unwrap_or("") {
            "hello" => {
                let (w, h) = (v["width"].as_u64().unwrap_or(1100).clamp(480, 1600) as u32, v["height"].as_u64().unwrap_or(760).clamp(360, 1200) as u32);
                let dpr = v["dpr"].as_f64().unwrap_or(1.0);
                session.status("Starting the browser…");
                match session.tab() {
                    Ok(lt) => {
                        let _ = lt.start_screencast(w, h, dpr);
                        if lt.url().starts_with("about:") { let _ = lt.navigate(&format!("{}/index.php", poster::MOM)); }
                        if let Some(f) = lt.capture() {
                            ws.send(Message::binary(frame_bytes(&f))).map_err(|e| e.to_string())?;
                            ready = false;
                        }
                        sent_frame = lt.latest_frame().0;
                        tab = Some(lt);
                        session.say(json!({"type": "control", "controller": session.controller().name()}));
                        session.status("Ready.");
                    }
                    Err(e) => session.say(json!({"type": "error", "message": e})),
                }
            }
            "ack" => ready = true,
            "take" => session.set_control(Controller::You),
            "release" => session.set_control(if *session.busy.lock().unwrap() { Controller::Helper } else { Controller::Nobody }),
            "mouse" | "wheel" | "key" | "text" if t.is_some() => {
                if !you && session.controller() == Controller::Helper {
                    session.status("The helper is working. Press Take control to use the page yourself.");
                    continue;
                }
                if !you { session.set_control(Controller::You); }
                let t = t.unwrap();
                let f = |k: &str| v[k].as_f64().unwrap_or(0.0);
                match v["type"].as_str().unwrap() {
                    "mouse" => t.mouse(v["kind"].as_str().unwrap_or("move"), f("x"), f("y"), v["button"].as_str().unwrap_or("left"), v["clicks"].as_u64().unwrap_or(1) as u32),
                    "wheel" => t.wheel(f("x"), f("y"), f("dx"), f("dy")),
                    "key" => t.key(v["key"].as_str().unwrap_or(""), v["down"].as_bool().unwrap_or(true), v["modifiers"].as_u64().unwrap_or(0) as u32),
                    _ => t.text(v["text"].as_str().unwrap_or("")),
                }
            }
            "courses" | "assessments" | "add" => job(session.clone(), v),
            _ => {}
        }
    }
    *session.viewer.lock().unwrap() = None;
    Ok(())
}

/// A frame as the viewer reads it: the page's CSS width and height (f32 LE), then the JPEG.
fn frame_bytes(f: &super::livecdp::Frame) -> Vec<u8> {
    let mut buf = Vec::with_capacity(f.jpeg.len() + 8);
    buf.extend_from_slice(&(f.width as f32).to_le_bytes());
    buf.extend_from_slice(&(f.height as f32).to_le_bytes());
    buf.extend_from_slice(&f.jpeg);
    buf
}

/// Runs one helper job in its own thread (one at a time per teacher).
fn job(session: Arc<Session>, v: Value) {
    {
        let mut b = session.busy.lock().unwrap();
        if *b { session.status("The helper is still busy with the last step."); return; }
        *b = true;
    }
    std::thread::spawn(move || {
        let r = run_job(&session, &v);
        if let Err(e) = r { session.say(json!({"type": "error", "message": e})); }
        *session.busy.lock().unwrap() = false;
        if session.controller() == Controller::Helper { session.set_control(Controller::Nobody); }
    });
}

fn run_job(session: &Arc<Session>, v: &Value) -> Result<(), String> {
    let mut page = Gated { tab: session.tab()?, session: session.clone() };
    signed_in(session, &mut page)?;
    match v["type"].as_str().unwrap_or("") {
        "courses" => {
            session.status("Reading your courses…");
            let list = poster::courses(&mut page)?;
            session.say(json!({"type": "courses", "list": list}));
        }
        "assessments" => {
            let cid = v["cid"].as_u64().ok_or("Which course?")?;
            session.status("Reading the assessments…");
            let list = poster::assessments(&mut page, cid)?;
            session.say(json!({"type": "assessments", "cid": cid, "list": list}));
        }
        _ => {
            let (cid, aid) = (v["cid"].as_u64().ok_or("Which course?")?, v["aid"].as_u64().ok_or("Which assessment?")?);
            let q: MomQuestion = MomQuestion::parse(v["question"].as_str().ok_or("No question was sent")?);
            let save = v["save"].as_bool().unwrap_or(false);
            let s = session.clone();
            let r = poster::add_question(&mut page, cid, aid, &q, save, &mut |l| s.status(l));
            session.say(match r {
                Ok(Added::Saved) => json!({"type": "added", "ok": true, "message": "Done. It's saved in the assessment on MyOpenMath."}),
                Ok(Added::Filled) => json!({"type": "added", "ok": true, "message": "It's filled in. Take control, check it, and press Save."}),
                Err(e) => json!({"type": "added", "ok": false, "message": e}),
            });
        }
    }
    Ok(())
}

/// Signed in to MyOpenMath, handing the page to the teacher to sign in when needed.
fn signed_in(session: &Arc<Session>, page: &mut Gated) -> Result<(), String> {
    session.status("Opening MyOpenMath…");
    if poster::signed_in(page)? { return Ok(()); }
    session.set_control(Controller::You);
    session.status("Please sign in to MyOpenMath in the page below. The helper goes on as soon as you're in.");
    if !poster::wait_signed_in(page, Duration::from_secs(900))? {
        return Err("Nobody signed in to MyOpenMath.".into());
    }
    session.set_control(Controller::Helper);
    session.status("Signed in.");
    Ok(())
}
