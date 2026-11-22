//! A small, synchronous Chrome controller over the DevTools protocol, for adding questions to
//! MyOpenMath in a real browser the teacher can watch and use.
//!
//! Chrome runs with its own profile folder (Chrome refuses remote control of a person's default
//! profile), so a MyOpenMath sign-in made there is kept for next time. It listens for DevTools on a
//! random port on 127.0.0.1 only, which it writes to `DevToolsActivePort` in the profile; we talk
//! to one tab over that WebSocket. Every call has a timeout, so a stuck page cannot hang the app.
//!
//! The same client serves the desktop app (a visible window) and a server-side browser (headless,
//! streamed to the web app).

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// Chrome starting and writing its DevTools port.
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(25);
/// One DevTools command; a script may wait on the page, so this is generous.
const CALL_TIMEOUT: Duration = Duration::from_secs(60);

/// A Chrome this app started.
pub struct Chrome {
    /// None when we attached to a Chrome a previous run left open on the same profile.
    child: Option<Child>,
    port: u16,
}

/// One tab, driven over its DevTools WebSocket.
pub struct Tab {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
    next: u64,
}

/// Chrome or Edge on this machine: `DAD_CHROME` first, then the usual places.
pub fn find_executable() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("DAD_CHROME") {
        return Some(PathBuf::from(p));
    }
    let fixed: &[&str] = if cfg!(target_os = "macos") {
        &["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
          "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
          "/Applications/Chromium.app/Contents/MacOS/Chromium"]
    } else if cfg!(windows) {
        &[r"C:\Program Files\Google\Chrome\Application\chrome.exe",
          r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
          r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
          r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"]
    } else {
        &[]
    };
    if let Some(p) = fixed.iter().map(PathBuf::from).find(|p| p.exists()) {
        return Some(p);
    }
    if cfg!(windows) {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let p = Path::new(&local).join(r"Google\Chrome\Application\chrome.exe");
            if p.exists() { return Some(p); }
        }
        return None;
    }
    let names = ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "microsoft-edge"];
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

impl Chrome {
    /// Starts Chrome on `profile` (created if missing). `headless`: no window (the server browser).
    pub fn launch(profile: &Path, headless: bool) -> Result<Chrome, String> {
        Self::launch_with_options(profile, headless, false)
    }

    pub(crate) fn launch_student_preview(profile: &Path) -> Result<Chrome, String> {
        Self::launch_with_options(profile, true, true)
    }

    fn launch_with_options(profile: &Path, headless: bool, student_preview: bool) -> Result<Chrome, String> {
        let exe = find_executable().ok_or("Chrome or Edge isn't installed on this computer. Install Google Chrome, then try again.")?;
        std::fs::create_dir_all(profile).map_err(|e| format!("Could not make the browser folder: {e}"))?;
        let port_file = profile.join("DevToolsActivePort");
        // A Chrome from an earlier run may still have this profile open (the teacher kept the
        // window): use it, since a second Chrome cannot open the same profile.
        if let Some(port) = std::fs::read_to_string(&port_file).ok().and_then(|s| s.lines().next().and_then(|l| l.trim().parse::<u16>().ok())) {
            if http_get(port, "/json/version").is_ok() {
                return Ok(Chrome { child: None, port });
            }
        }
        let _ = std::fs::remove_file(&port_file);
        let mut cmd = Command::new(&exe);
        cmd.arg(format!("--user-data-dir={}", profile.display()))
            .arg("--remote-debugging-port=0")
            .arg("--remote-debugging-address=127.0.0.1")
            .args(["--no-first-run", "--no-default-browser-check", "--disable-features=Translate,OptimizationHints",
                "--window-size=1280,900", "about:blank"])
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if headless {
            cmd.arg("--headless=new");
        }
        if student_preview && cfg!(target_os = "linux") {
            // The local student player never signs in. Don't ask for the desktop wallet.
            cmd.arg("--password-store=basic");
        }
        let mut child = cmd.spawn().map_err(|e| format!("Could not start {}: {e}", exe.display()))?;
        let start = Instant::now();
        let port = loop {
            if let Some(port) = std::fs::read_to_string(&port_file).ok()
                .and_then(|s| s.lines().next().and_then(|l| l.trim().parse::<u16>().ok()))
            {
                break port;
            }
            if let Ok(Some(status)) = child.try_wait() {
                return Err(format!("Chrome closed right away ({status}). Is another copy using the same profile?"));
            }
            if start.elapsed() > LAUNCH_TIMEOUT {
                let _ = child.kill();
                return Err("Chrome didn't start in time.".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        Ok(Chrome { child: Some(child), port })
    }

    /// Whether Chrome is still running and answering.
    pub fn alive(&mut self) -> bool {
        if let Some(c) = self.child.as_mut() {
            if matches!(c.try_wait(), Ok(Some(_))) { return false; }
        }
        http_get(self.port, "/json/version").is_ok()
    }

    /// The DevTools address of the first ordinary tab, opening one if the teacher closed them all.
    pub fn tab_url(&self) -> Result<String, String> {
        let list: Value = serde_json::from_str(&http_get(self.port, "/json/list")?).map_err(|e| e.to_string())?;
        let page = list.as_array().and_then(|a| a.iter().find(|t| t["type"] == "page").cloned());
        let page = match page {
            Some(p) => p,
            None => serde_json::from_str(&http_put(self.port, "/json/new?about:blank")?).map_err(|e| e.to_string())?,
        };
        page["webSocketDebuggerUrl"].as_str().map(str::to_string).ok_or_else(|| "Chrome gave no tab address".to_string())
    }

    /// The first ordinary tab, opening one if the teacher closed them all.
    pub fn tab(&self) -> Result<Tab, String> {
        let url = self.tab_url()?;
        let (ws, _) = tungstenite::connect(url.as_str()).map_err(|e| format!("Could not reach the Chrome tab: {e}"))?;
        if let MaybeTlsStream::Plain(s) = ws.get_ref() {
            let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
        }
        let mut tab = Tab { ws, next: 0 };
        tab.call("Page.enable", json!({}))?;
        tab.call("Runtime.enable", json!({}))?;
        Ok(tab)
    }

    /// Asks Chrome to quit, then makes sure it has.
    pub fn close(mut self) {
        if let Ok(mut tab) = self.tab() {
            let _ = tab.call("Browser.close", json!({}));
        }
        let Some(mut child) = self.child.take() else { return };
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if matches!(child.try_wait(), Ok(Some(_))) { return; }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
    }
}

impl Tab {
    /// One DevTools command and its result; events in between are skipped.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next += 1;
        let id = self.next;
        self.ws.send(Message::text(json!({"id": id, "method": method, "params": params}).to_string()))
            .map_err(|e| format!("Chrome connection lost: {e}"))?;
        let start = Instant::now();
        loop {
            if start.elapsed() > CALL_TIMEOUT {
                return Err(format!("Chrome didn't answer {method} in time"));
            }
            match self.ws.read() {
                Ok(Message::Text(t)) => {
                    let v: Value = serde_json::from_str(t.as_str()).unwrap_or(Value::Null);
                    if v["id"].as_u64() == Some(id) {
                        if let Some(e) = v.get("error") {
                            return Err(format!("{method}: {}", e["message"].as_str().unwrap_or("failed")));
                        }
                        return Ok(v["result"].clone());
                    }
                }
                Ok(Message::Close(_)) => return Err("The Chrome tab was closed".into()),
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(e) => return Err(format!("Chrome connection lost: {e}")),
            }
        }
    }

    /// Runs `js` in the page and returns its value (promises are awaited).
    pub fn eval(&mut self, js: &str) -> Result<Value, String> {
        let r = self.call("Runtime.evaluate", json!({"expression": js, "returnByValue": true, "awaitPromise": true}))?;
        if let Some(ex) = r.get("exceptionDetails") {
            let msg = ex["exception"]["description"].as_str().or(ex["text"].as_str()).unwrap_or("script error");
            return Err(format!("The page script failed: {}", msg.lines().next().unwrap_or(msg)));
        }
        Ok(r["result"]["value"].clone())
    }

    /// Goes to `url` and waits until the page has loaded.
    pub fn navigate(&mut self, url: &str) -> Result<(), String> {
        self.call("Page.navigate", json!({"url": url}))?;
        std::thread::sleep(Duration::from_millis(300));
        self.wait_for("document.readyState === 'complete'", Duration::from_secs(30))
            .and_then(|ok| if ok { Ok(()) } else { Err("The page took too long to load".into()) })
    }

    /// Polls `cond` (a JS expression) until it is true or `limit` passes.
    pub fn wait_for(&mut self, cond: &str, limit: Duration) -> Result<bool, String> {
        let start = Instant::now();
        loop {
            // A navigation in progress destroys the context; that is "not yet", not a failure.
            if let Ok(Value::Bool(true)) = self.eval(&format!("!!({cond})")) {
                return Ok(true);
            }
            if start.elapsed() > limit {
                return Ok(false);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// The tab's address.
    pub fn url(&mut self) -> String {
        self.eval("location.href").ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
    }

    /// Brings the window to the front, so the teacher sees what needs them.
    pub fn show(&mut self) {
        let _ = self.call("Page.bringToFront", json!({}));
    }
}

fn http_get(port: u16, path: &str) -> Result<String, String> {
    ureq::get(&format!("http://127.0.0.1:{port}{path}")).call()
        .map_err(|e| e.to_string())?
        .body_mut().read_to_string().map_err(|e| e.to_string())
}

fn http_put(port: u16, path: &str) -> Result<String, String> {
    ureq::put(&format!("http://127.0.0.1:{port}{path}")).send_empty()
        .map_err(|e| e.to_string())?
        .body_mut().read_to_string().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs Chrome: `cargo test --lib mom::chrome -- --ignored`.
    #[test]
    #[ignore]
    fn drives_a_real_chrome() {
        let dir = std::env::temp_dir().join(format!("dad-chrome-test-{}", std::process::id()));
        let mut chrome = Chrome::launch(&dir, true).expect("Chrome starts");
        assert!(chrome.alive());
        let mut tab = chrome.tab().unwrap();
        tab.navigate("data:text/html,<title>hi</title><input id=x><script>document.title='ready'</script>").unwrap();
        assert_eq!(tab.eval("document.title").unwrap(), json!("ready"));
        assert_eq!(tab.eval("Promise.resolve(40 + 2)").unwrap(), json!(42));
        assert!(tab.wait_for("document.getElementById('x')", Duration::from_secs(2)).unwrap());
        assert!(tab.eval("nosuchthing()").is_err());
        assert!(tab.url().starts_with("data:text/html"));
        chrome.close();
        let _ = std::fs::remove_dir_all(dir);
    }
}
