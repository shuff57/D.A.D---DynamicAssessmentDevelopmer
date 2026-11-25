//! LLM conversion of a test photo/scan/screenshot into Blackboard import text (see
//! `formats::parse_blackboard` for the exact grammar the model must produce).
//!
//! One conversion job at a time: `send_image` spawns a `std::thread` (egui never blocks)
//! and keeps the result channel in [`Ai`]; the Imports screen calls `poll` each frame and
//! the thread calls `ctx.request_repaint()` when it finishes, so the parsed questions show
//! up on the next frame. API keys come from, in order: a key saved in the app (`ai-keys.json`,
//! mode 0600, never in settings.json or backups), the provider's environment variable, and
//! (Ollama Cloud) the ollama-cloud tool's credentials file. They are read at request time
//! and never logged, echoed, put in errors or Debug output.

pub use crate::Settings;

use serde_json::{Value, json};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

const OLLAMA_DEFAULT_BASE: &str = "http://127.0.0.1:11434";
const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";
/// Small local vision model that a laptop GPU can carry through a slow image round trip.
const OLLAMA_DEFAULT_MODEL: &str = "gemma4:31b";
/// OpenRouter default mirrors the local Ollama model on the free tier; set `ai_model` to override.
const OPENROUTER_DEFAULT_MODEL: &str = "qwen/qwen3-vl-8b-instruct:free";
/// Custom OpenAI-compatible endpoints default to the same model as local Ollama unless
/// `ai_model` says otherwise (the base URL decides what that model id actually means).
const CUSTOM_DEFAULT_MODEL: &str = "gemma4:31b";
/// The verSHion AI relay: a Cloudflare Worker that calls Workers AI on its own account. The base
/// is the worker's `/ai` path, the key is the relay's key (`AI_RELAY_KEY`), and the worker picks
/// the model (a client's model id is ignored there, so this one is only for display).
const WORKERS_DEFAULT_MODEL: &str = "@cf/google/gemma-4-26b-a4b-it";
const WORKERS_KEY_ENV: &str = "VERSHION_RELAY_KEY";
/// Images are slow, on small local GPUs especially.
#[cfg(not(target_arch = "wasm32"))]
const TIMEOUT: Duration = Duration::from_secs(300);

const OPENROUTER_KEY_ENV: &str = "OPENROUTER_API_KEY";
const CUSTOM_KEY_ENV: &str = "VERSHION_AI_KEY";
/// Ollama's hosted service: same OpenAI-compatible surface (`/v1`), keyed.
#[cfg(not(target_arch = "wasm32"))]
const OLLAMA_CLOUD_BASE: &str = "https://ollama.com/v1";
/// Browsers cannot call ollama.com directly (it sends no CORS headers). The web build talks to
/// the bundled relay instead, which forwards `/u/ollama.com/...` to `https://ollama.com/...`.
#[cfg(target_arch = "wasm32")]
const OLLAMA_CLOUD_RELAY_BASE: &str = "http://localhost:8189/u/ollama.com/v1";
const OLLAMA_CLOUD_DEFAULT_MODEL: &str = "gemma4:31b";
/// Ollama Cloud's key: `OLLAMA_API_KEY`, else the ollama-cloud tool's own credentials file.
const CLOUD_KEY_ENV: &str = "OLLAMA_API_KEY";
const CLOUD_KEY_FILE: &str = ".config/ollama-cloud/credentials.json";

/// The conversion prompt: one page in, one verSHion Ditto package (`docs/ditto-format.md`) out.
const TRANSCRIBE_PROMPT: &str = include_str!("../prompts/ditto-convert.md");

/// The instruction plus the user's typed note about the source, if any.
/// Public so the headless `VERSHION_CONVERT` path shares the UI's prompt.
pub fn prompt_for(hint: &str) -> String {
    transcription_prompt(hint)
}

fn transcription_prompt(hint: &str) -> String {
    let hint = hint.trim();
    if hint.is_empty() {
        return TRANSCRIBE_PROMPT.to_string();
    }
    format!("{TRANSCRIBE_PROMPT}\n\nUser note about this source:\n{hint}")
}

/// Ollama Cloud's base for this target: direct on the desktop, through the local relay in the
/// browser.
fn cloud_base() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        OLLAMA_CLOUD_RELAY_BASE.to_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        OLLAMA_CLOUD_BASE.to_string()
    }
}

/// The Ollama Cloud key note: the browser goes through the relay; the desktop can also read the
/// environment and the ollama-cloud credentials file.
fn cloud_key_help() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        "Ollama Cloud needs an API key. Requests go through the local relay: run \
         `node tools/relay.js` once and keep it running. The first request asks for local-network \
         access in your browser; allow it. The key stays in this browser."
            .to_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        format!(
            "Ollama Cloud needs an API key. A key saved here is used first, then the {CLOUD_KEY_ENV} environment variable, then ~/{CLOUD_KEY_FILE}."
        )
    }
}

#[derive(Clone, Debug)]
pub struct AiCfg {
    /// Normalized provider: "ollama" (the default), "openrouter", or "custom".
    pub provider: String,
    pub base: String,
    pub model: String,
    /// How much the model reasons before answering: "off", "on", "low", "medium" or "high". None leaves
    /// the model's own default (DeepSeek and Gemma think unless told not to).
    pub think: Option<String>,
}

impl AiCfg {
    pub fn from(s: &Settings) -> AiCfg {
        let provider = match s.ai_provider.trim().to_ascii_lowercase().as_str() {
            "openrouter" => "openrouter",
            "custom" => "custom",
            "workers" | "workers-ai" | "relay" => "workers",
            "account" => "account",
            "ollama-cloud" | "cloud" | "ollama cloud" => "cloud",
            // An AI command-line tool on this computer (desktop): the teacher's own Claude or Codex.
            "claude-cli" | "claude" => "claude-cli",
            "codex-cli" | "codex" => "codex-cli",
            // Empty or unknown settings fall back to local Ollama.
            _ => "ollama",
        };
        let model: String = if s.ai_model.trim().is_empty() {
            match provider {
                "openrouter" => OPENROUTER_DEFAULT_MODEL.to_string(),
                "custom" => CUSTOM_DEFAULT_MODEL.to_string(),
                "workers" | "account" => WORKERS_DEFAULT_MODEL.to_string(),
                "cloud" => OLLAMA_CLOUD_DEFAULT_MODEL.to_string(),
                // Sonnet: strong at question writing, and gentle on a subscription's limits.
                "claude-cli" => "sonnet".to_string(),
                // The CLI's own default model.
                "codex-cli" => String::new(),
                _ => OLLAMA_DEFAULT_MODEL.to_string(),
            }
        } else {
            s.ai_model.trim().to_string()
        };
        let base: String = match provider {
            "openrouter" => OPENROUTER_BASE.to_string(),
            "cloud" => cloud_base(),
            // Both the custom endpoint and the relay are whatever URL the user set.
            "custom" | "workers" => s.ai_base.trim().trim_end_matches('/').to_string(),
            // The hosted AI is the account server's own relay.
            "account" => account_base(),
            _ => {
                let own = s.ollama_base.trim().trim_end_matches('/');
                if own.starts_with("http://") || own.starts_with("https://") { own.to_string() } else { OLLAMA_DEFAULT_BASE.to_string() }
            }
        };
        AiCfg { provider: provider.to_string(), base, model, think: None }
    }

    /// The built-in relay, whether it is the one a person typed in or the account server's own.
    pub fn is_relay(&self) -> bool {
        self.provider == "workers" || self.provider == "account"
    }

    /// One human sentence about where the API key for this provider comes from.
    /// Order: a key saved in the app wins, then the environment variable, then (Ollama
    /// Cloud only) the ollama-cloud credentials file.
    pub fn key_source_help(&self) -> String {
        match self.provider.as_str() {
            "openrouter" => format!(
                "OpenRouter needs an API key. A key saved here is used first, otherwise the {OPENROUTER_KEY_ENV} environment variable."
            ),
            "cloud" => cloud_key_help(),
            "custom" => format!(
                "A custom endpoint may need an API key. A key saved here is used first, otherwise the {CUSTOM_KEY_ENV} environment variable."
            ),
            "account" => "D.A.D AI runs on the school server you signed in to. Each conversion costs credits from your account: 40 are free every week, and you can buy more under Settings, then D.A.D account.".to_string(),
            "workers" => format!(
                "The D.A.D relay calls Workers AI on its Cloudflare account, so there is no provider key: the binding is the credential. A relay key is needed only when the relay is reachable publicly, and is then the same key the relay was deployed with (saved here first, otherwise the {WORKERS_KEY_ENV} environment variable). A relay running on this machine needs none."
            ),
            _ => "Ollama runs on the local machine and needs no API key.".to_string(),
        }
    }

    /// True for providers that take an API key (everything but local Ollama).
    pub fn uses_key(&self) -> bool {
        self.key_env().is_some()
    }

    /// Where the key would come from right now, for the Settings status line.
    pub fn key_status(&self) -> KeySource {
        resolve_key(self, &load_keys(), &|v| std::env::var(v).ok(), home_dir().as_deref()).map(|(_, s)| s).unwrap_or(KeySource::None)
    }

    /// True when a request would carry a key (or the provider needs none). The relay's key is
    /// optional: a relay that is not publicly reachable (local `wrangler dev --remote`) needs none.
    pub fn has_key_or_needs_none(&self) -> bool {
        !self.uses_key() || self.provider == "custom" || self.provider == "workers" || self.key_status() != KeySource::None
    }

    fn key_env(&self) -> Option<&'static str> {
        match self.provider.as_str() {
            "openrouter" => Some(OPENROUTER_KEY_ENV),
            "custom" => Some(CUSTOM_KEY_ENV),
            "cloud" => Some(CLOUD_KEY_ENV),
            "workers" => Some(WORKERS_KEY_ENV),
            _ => None,
        }
    }
}

/// Where a resolved key came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    Saved,
    Env(&'static str),
    CredentialsFile,
    None,
}

impl KeySource {
    pub fn describe(self) -> String {
        match self {
            KeySource::Saved => "Key saved on this computer".to_string(),
            KeySource::Env(v) => format!("Using {v} from the environment"),
            KeySource::CredentialsFile => format!("Using ~/{CLOUD_KEY_FILE}"),
            KeySource::None => "No key".to_string(),
        }
    }
}

/// Provider -> key, as stored in `ai-keys.json`. `Debug` is deliberately redacted.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Keys(std::collections::BTreeMap<String, String>);

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Keys({} saved, redacted)", self.0.len())
    }
}

impl Keys {
    pub fn get(&self, provider: &str) -> Option<&str> {
        self.0.get(provider).map(String::as_str).filter(|k| !k.is_empty())
    }
}

const KEYS_FILE: &str = "ai-keys.json";

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    /// Browser keys live in memory for the page's lifetime: a page cannot write a protected file,
    /// and a key must never land in IndexedDB/localStorage.
    static WEB_KEYS: std::cell::RefCell<Keys> = std::cell::RefCell::new(Keys::default());
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_keys() -> Keys {
    load_keys_in(&crate::store::store_dir())
}

#[cfg(target_arch = "wasm32")]
pub fn load_keys() -> Keys {
    WEB_KEYS.with(|k| k.borrow().clone())
}
pub fn load_keys_in(dir: &std::path::Path) -> Keys {
    std::fs::read(dir.join(KEYS_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Saves (`Some`) or removes (`None`) one provider's key.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_key(provider: &str, key: Option<&str>) -> Result<(), String> {
    set_key_in(&crate::store::store_dir(), provider, key)
}

#[cfg(target_arch = "wasm32")]
pub fn set_key(provider: &str, key: Option<&str>) -> Result<(), String> {
    WEB_KEYS.with(|keys| {
        let mut keys = keys.borrow_mut();
        match key.map(clean_key).filter(|k| !k.is_empty()) {
            Some(k) => {
                keys.0.insert(provider.to_string(), k);
            }
            None => {
                keys.0.remove(provider);
            }
        }
    });
    Ok(())
}

/// What a teacher pastes is often more than the key: a "Key:" or `API_KEY=` label, quotes or
/// backticks from an email, a line break, a full stop. Keep only the key itself.
pub fn clean_key(raw: &str) -> String {
    let unquoted: String = raw.chars()
        .filter(|c| !matches!(c, '"' | '\'' | '`' | '\u{201C}' | '\u{201D}' | '\u{2018}' | '\u{2019}' | '<' | '>'))
        .collect();
    let strip_label = |t: &str| -> String {
        if let Some(i) = t.find([':', '=']) {
            let (label, rest) = (t[..i].to_ascii_lowercase(), &t[i + 1..]);
            if !rest.is_empty() && ["key", "token", "secret", "bearer"].iter().any(|w| label.contains(w)) {
                return rest.to_string();
            }
        }
        t.to_string()
    };
    // The key is the longest word left; labels ("Key:", "Bearer") and stray words are shorter.
    unquoted.split_whitespace()
        .map(|t| strip_label(t.trim_end_matches(['.', ',', ';'])))
        .fold(String::new(), |best, t| if t.len() > best.len() { t } else { best })
}

/// One sentence when a pasted key can't belong to this AI (most often: a key from another site).
pub fn key_shape_problem(provider: &str, key: &str) -> Option<String> {
    let key = clean_key(key);
    if key.is_empty() { return None; }
    let (name, prefix) = match provider {
        "openrouter" => ("OpenRouter", "sk-or-"),
        "anthropic" => ("Anthropic", "sk-ant-"),
        "openai" => ("OpenAI", "sk-"),
        _ => return None,
    };
    (!key.starts_with(prefix))
        .then(|| format!("This doesn't look like an {name} key: they start with {prefix}. Check that you copied the whole key."))
}

/// Atomic write (temp + rename) of the whole map, mode 0600 on unix from creation.
pub fn set_key_in(dir: &std::path::Path, provider: &str, key: Option<&str>) -> Result<(), String> {
    use std::io::Write as _;
    let mut keys = load_keys_in(dir);
    match key.map(clean_key).filter(|k| !k.is_empty()) {
        Some(k) => {
            keys.0.insert(provider.to_string(), k);
        }
        None => {
            keys.0.remove(provider);
        }
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't save the key: {}", e.kind()))?;
    let tmp = dir.join(format!("{KEYS_FILE}.tmp"));
    let path = dir.join(KEYS_FILE);
    let body = serde_json::to_vec(&keys).map_err(|_| "Couldn't save the key.".to_string())?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let res = (|| -> std::io::Result<()> {
        let mut f = opts.open(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(&body)?;
        f.sync_all()?;
        std::fs::rename(&tmp, &path)
    })();
    res.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Couldn't save the key: {}", e.kind())
    })
}

/// Pure resolution: saved key, then env var, then (cloud) the credentials file under `home`.
fn resolve_key(
    cfg: &AiCfg,
    saved: &Keys,
    env: &dyn Fn(&str) -> Option<String>,
    home: Option<&std::path::Path>,
) -> Option<(String, KeySource)> {
    let var = cfg.key_env()?;
    if let Some(k) = saved.get(&cfg.provider) {
        return Some((k.to_string(), KeySource::Saved));
    }
    if let Some(k) = env(var).filter(|k| !k.is_empty()) {
        return Some((k, KeySource::Env(var)));
    }
    if cfg.provider == "cloud"
        && let Some(bytes) = home.and_then(|h| std::fs::read(h.join(CLOUD_KEY_FILE)).ok())
        && let Ok(v) = serde_json::from_slice::<Value>(&bytes)
        && let Some(k) = v.get("apiKey").and_then(Value::as_str)
        && !k.is_empty()
    {
        return Some((k.to_string(), KeySource::CredentialsFile));
    }
    None
}

/// The signed-in account the hosted AI is charged to: the server's relay address and a way to get a fresh access token.
/// The app keeps this current each frame; an account that is not signed in leaves it empty.
type TokenSource = std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>;
static ACCOUNT_AI: std::sync::Mutex<Option<(String, TokenSource)>> = std::sync::Mutex::new(None);

pub fn set_account_ai(account: Option<(String, TokenSource)>) {
    if let Ok(mut a) = ACCOUNT_AI.lock() {
        *a = account;
    }
}

/// True when a signed-in account is there to pay for the hosted AI.
pub fn account_ai_ready() -> bool {
    ACCOUNT_AI.lock().is_ok_and(|a| a.is_some())
}

fn account_base() -> String {
    ACCOUNT_AI.lock().ok().and_then(|a| a.as_ref().map(|(b, _)| format!("{}/ai", b.trim_end_matches('/')))).unwrap_or_default()
}

fn account_token() -> Option<String> {
    let src = ACCOUNT_AI.lock().ok().and_then(|a| a.as_ref().map(|(_, t)| t.clone()))?;
    src()
}

/// The API key for a request, resolved at request time. Never logged or echoed.
fn request_key(cfg: &AiCfg) -> Option<String> {
    // The hosted AI is charged to the signed-in account, so it carries that account's access token.
    if cfg.provider == "account" {
        return account_token();
    }
    resolve_key(cfg, &load_keys(), &|v| std::env::var(v).ok(), home_dir().as_deref()).map(|(k, _)| k)
}

/// The listing/probe URL for a provider: Ollama's tags, the relay's health route, else `/models`.
fn list_url(cfg: &AiCfg, base: &str) -> String {
    match cfg.provider.as_str() {
        "ollama" => format!("{base}/api/tags"),
        // The relay serves one model and has no /models route; its health route proves it is up.
        "workers" | "account" => format!("{base}/health"),
        _ => format!("{base}/models"),
    }
}

/// What the key test asks for. OpenRouter's model list is public, so it answers any key (or none);
/// its `/key` route answers only a real key.
fn probe_url(cfg: &AiCfg, base: &str) -> String {
    if cfg.provider == "openrouter" { format!("{base}/key") } else { list_url(cfg, base) }
}

/// Cheap authenticated probe for the Settings "Test connection" button: GET {base}/models.
/// Blocking; call from a thread. The reply never contains the key.
#[cfg(not(target_arch = "wasm32"))]
pub fn test_connection(cfg: &AiCfg) -> Result<(), String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = probe_url(cfg, base);
    let mut req = agent().get(&url);
    if let Some(k) = request_key(cfg) {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let resp = req.call().map_err(|_| format!("Can't reach {base}."))?;
    match resp.status().as_u16() {
        200..=299 => Ok(()),
        401 | 403 => Err("Key rejected (401).".to_string()),
        404 => Err("Reached the server, but it has no /models endpoint (404). The key may still work.".to_string()),
        n => Err(format!("The server answered {n}.")),
    }
}

/// Browser probe: the same GET, awaited through `fetch`.
#[cfg(target_arch = "wasm32")]
pub async fn test_connection_async(cfg: &AiCfg) -> Result<(), String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = probe_url(cfg, base);
    let status = fetch_status_async(&url, request_key(cfg).as_deref())
        .await
        .map_err(|e| cloud_error_hint(cfg, e))?;
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err("Key rejected (401).".to_string()),
        404 => Err("Reached the server, but it has no /models endpoint (404). The key may still work.".to_string()),
        n => Err(format!("The server answered {n}.")),
    }
}

/// Adds the relay instruction when a browser cannot reach Ollama Cloud.
#[cfg(target_arch = "wasm32")]
fn cloud_error_hint(cfg: &AiCfg, err: String) -> String {
    if cfg.provider == "cloud" {
        format!(
            "{err} If the relay is not running, start it once with `node tools/relay.js`; if the \
             browser asked for local-network access, choose Allow."
        )
    } else {
        err
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_status_async(url: &str, key: Option<&str>) -> Result<u16, String> {
    use wasm_bindgen::JsCast as _;
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let headers = web_sys::Headers::new().map_err(js)?;
    if let Some(k) = key {
        headers.set("Authorization", &format!("Bearer {k}")).map_err(js)?;
    }
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_mode(web_sys::RequestMode::Cors);
    init.set_headers(&headers);
    let signal = web_sys::AbortSignal::timeout_with_u32(30_000);
    init.set_signal(Some(&signal));
    let window = web_sys::window().ok_or("no window")?;
    let promise = window.fetch_with_str_and_init(url, &init);
    let resp = wasm_bindgen_futures::JsFuture::from(promise).await.map_err(|_| {
        format!("Can't reach {url}. The server may be off, or it does not allow requests from this page (CORS).")
    })?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "not a response".to_string())?;
    Ok(resp.status())
}

/// Model ids from a `/models` (OpenAI shape) or `/api/tags` (Ollama) reply.
pub fn parse_models(provider: &str, v: &Value) -> Vec<String> {
    let (rows, key) = if provider == "ollama" {
        (v["models"].as_array(), "name")
    } else {
        (v["data"].as_array(), "id")
    };
    let mut ids: Vec<String> = rows
        .into_iter()
        .flatten()
        .filter_map(|m| m.get(key).and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// The models the server lists. Blocking; call from a thread.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_models(cfg: &AiCfg) -> Result<Vec<String>, String> {
    // The relay serves exactly one model: nothing to list.
    if cfg.is_relay() {
        return Ok(vec![cfg.model.clone()]);
    }
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = list_url(cfg, base);
    let mut req = agent().get(&url);
    if let Some(k) = request_key(cfg) {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut resp = req.call().map_err(|_| format!("Can't reach {base}."))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from {base}: {e}"))?;
    if !(200..300).contains(&status) {
        return Err(format!("The server answered {status}."));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{base} did not answer with JSON ({e})"))?;
    Ok(parse_models(&cfg.provider, &v))
}

/// Browser model list: the same GET, awaited through `fetch`.
#[cfg(target_arch = "wasm32")]
pub async fn list_models_async(cfg: &AiCfg) -> Result<Vec<String>, String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = list_url(cfg, base);
    let v = get_json_async(&url, request_key(cfg).as_deref())
        .await
        .map_err(|e| cloud_error_hint(cfg, e))?;
    Ok(parse_models(&cfg.provider, &v))
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn get_json_async(url: &str, key: Option<&str>) -> Result<Value, String> {
    use wasm_bindgen::JsCast as _;
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let headers = web_sys::Headers::new().map_err(js)?;
    if let Some(k) = key {
        headers.set("Authorization", &format!("Bearer {k}")).map_err(js)?;
    }
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_mode(web_sys::RequestMode::Cors);
    init.set_headers(&headers);
    let signal = web_sys::AbortSignal::timeout_with_u32(30_000);
    init.set_signal(Some(&signal));
    let window = web_sys::window().ok_or("no window")?;
    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str_and_init(url, &init))
        .await
        .map_err(|_| {
            format!("Can't reach {url}. The server may be off, or it does not allow requests from this page (CORS).")
        })?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "not a response".to_string())?;
    let status = resp.status();
    let text = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(js)?)
        .await
        .map_err(js)?
        .as_string()
        .unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(format!("{url} answered {status}."));
    }
    serde_json::from_str(&text).map_err(|e| format!("{url} did not answer with JSON ({e})"))
}

/// One pending conversion job; `Default` starts with none.
#[derive(Default)]
pub struct Ai {
    job: Option<Receiver<Result<Converted, String>>>,
}

impl Ai {
    /// Starts converting one image in the background. The channel delivers the model's
    /// reply with [`extract_import_text`] already applied, so `poll` returns import-ready
    /// text; the thread asks egui to repaint the moment it is done.
    pub fn send_image(&mut self, bytes: Vec<u8>, mime: &str, hint: String, page_no: usize, cfg: &AiCfg, ctx: &egui::Context) {
        let (tx, rx) = channel();
        self.job = Some(rx);
        let cfg = cfg.clone();
        let mime = mime.to_string();
        let ctx = ctx.clone();
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let _ = tx.send(convert_page(&bytes, &mime, &hint, &cfg, page_no));
            ctx.request_repaint();
        });
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let _ = tx.send(convert_page_async(&bytes, &mime, &hint, &cfg, page_no).await);
            ctx.request_repaint();
        });
    }

    /// Picks up a finished job, if any. Called every frame from the Imports screen.
    pub fn poll(&mut self) -> Option<Result<Converted, String>> {
        let Some(rx) = self.job.as_mut() else { return None };
        match rx.try_recv() {
            Ok(r) => {
                self.job = None;
                Some(r)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.job = None;
                Some(Err("the AI job ended without sending a result".to_string()))
            }
        }
    }
}

/// One round trip: Ollama's native `/api/chat` or an OpenAI-compatible `/chat/completions`.
/// Also the entry point for the headless `VERSHION_CONVERT` path.
pub fn transcribe(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str) -> Result<String, String> {
    transcribe_at(bytes, mime, cfg, prompt, 0.0)
}

fn transcribe_at(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str, temp: f64) -> Result<String, String> {
    match cfg.provider.as_str() {
        "ollama" => ollama_chat(cfg, prompt, &[bytes], temp),
        _ => openai_chat(cfg, prompt, mime, &[bytes], temp),
    }
}

/// A question to the model about one or more PNG images, answered with JSON.
pub fn ask_images(images: &[&[u8]], cfg: &AiCfg, prompt: &str, temp: f64) -> Result<String, String> {
    match cfg.provider.as_str() {
        "ollama" => ollama_chat(cfg, prompt, images, temp),
        _ => openai_chat(cfg, prompt, "image/png", images, temp),
    }
}

// ---------- Find models ----------
//
// "Find models" keeps what this account can use to read a picture. Ollama Cloud and custom
// endpoints: each listed model gets a tiny image and a one-word reply limit, and only the ones that
// answer stay (a free account is refused with 402 on paid models, text-only models refuse the
// image). Local Ollama is not pinged, since that would load every installed model; its own
// `capabilities` say who can see. OpenRouter lists its input modalities, so it is not pinged either.

/// One model a server lists. `vision` is `Some` when the listing itself says.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub vision: Option<bool>,
}

/// What Find models learned.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Checked {
    /// Models to offer, in the order listed.
    pub ok: Vec<String>,
    /// Models left out and why.
    pub left_out: Vec<(String, String)>,
    /// How many the server listed.
    pub total: usize,
}

/// Models past this many are not pinged one by one (a long public list would be slow and rude).
const PROBE_LIMIT: usize = 40;

/// Ids from a listing, with OpenRouter's `architecture.input_modalities` read as the vision flag.
pub fn parse_model_info(provider: &str, v: &Value) -> Vec<ModelInfo> {
    let modal = |id: &str| -> Option<bool> {
        let row = v["data"].as_array()?.iter().find(|m| m.get("id").and_then(Value::as_str) == Some(id))?;
        let mods = row.pointer("/architecture/input_modalities")?.as_array()?;
        Some(mods.iter().any(|m| m.as_str() == Some("image")))
    };
    parse_models(provider, v)
        .into_iter()
        .map(|id| {
            let vision = if provider == "openrouter" { modal(&id) } else { None };
            ModelInfo { id, vision }
        })
        .collect()
}

/// A small picture with something in it, for the ping.
fn probe_png() -> &'static [u8] {
    static P: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let mut img = image::RgbImage::from_pixel(64, 64, image::Rgb([255, 255, 255]));
        for y in 16..48 {
            for x in 16..48 {
                img.put_pixel(x, y, image::Rgb([40, 40, 40]));
            }
        }
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .expect("encode the probe picture");
        out
    })
}

/// The ping: the model's chat endpoint, one picture, a reply of a few tokens at most.
fn probe_request(cfg: &AiCfg, model: &str) -> (String, Value) {
    let base = cfg.base.trim_end_matches('/');
    let png = base64_encode(probe_png());
    let prompt = "Reply with the single word OK.";
    if cfg.provider == "ollama" {
        let body = json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt, "images": [png]}],
            "stream": false,
            "options": {"num_predict": 8},
        });
        (format!("{base}/api/chat"), body)
    } else {
        let body = json!({
            "model": model,
            "stream": false,
            "max_tokens": 8,
            "messages": [{"role": "user", "content": [
                {"type": "text", "text": prompt},
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{png}")}},
            ]}],
        });
        (format!("{base}/chat/completions"), body)
    }
}

/// A few words for why a ping failed, from `post_json`'s "<url> answered <status>: <message>".
pub fn why_not(err: &str) -> String {
    let (status, msg) = match err.split_once(" answered ") {
        Some((_, rest)) => match rest.split_once(": ") {
            Some((s, m)) => (s.trim().parse::<u16>().unwrap_or(0), m),
            None => (rest.trim().parse::<u16>().unwrap_or(0), ""),
        },
        None => (0, err),
    };
    let low = msg.to_ascii_lowercase();
    match status {
        401 => "key refused".to_string(),
        402 => "not on your plan".to_string(),
        403 if low.contains("rolled out") || low.contains("not yet available") => "not available yet".to_string(),
        403 => "not available to you".to_string(),
        400 if low.contains("image") => "text only".to_string(),
        400 => "not a chat model".to_string(),
        404 => "not found".to_string(),
        429 => "busy, try again".to_string(),
        0 => "no answer".to_string(),
        s => format!("error {s}"),
    }
}

/// Folds ping results into what to offer. `results` is in listing order.
fn assemble_checked(total: usize, results: Vec<(String, Result<(), String>)>) -> Checked {
    let mut c = Checked { total, ..Default::default() };
    for (id, r) in results {
        match r {
            Ok(()) => c.ok.push(id),
            Err(e) => c.left_out.push((id, why_not(&e))),
        }
    }
    c
}

/// The line under the picker: how many answered and what was left out.
pub fn checked_note(c: &Checked) -> String {
    let mut note = format!(
        "{} of {} models can read images with your account{}",
        c.ok.len(),
        c.total,
        if c.ok.len() == c.total { ". All are listed." } else { ". Only those are listed." }
    );
    if !c.left_out.is_empty() {
        let shown: Vec<String> = c.left_out.iter().take(4).map(|(id, why)| format!("{id} ({why})")).collect();
        note.push_str(&format!(" Left out: {}", shown.join(", ")));
        if c.left_out.len() > 4 {
            note.push_str(&format!(" and {} more", c.left_out.len() - 4));
        }
        note.push('.');
    }
    note
}

/// The answer to "Find models" when nothing could be kept (usually a wrong key or no connection):
/// the picker keeps its old list rather than going empty.
fn nothing_answered(total: usize, left_out: &[(String, String)]) -> String {
    let refused = left_out.iter().any(|(_, w)| w == "key refused");
    let tail = if refused { ", and the server refused the key. Check the API key." } else { ". Check the key and the connection." };
    format!("None of the {total} models answered{tail}")
}

#[cfg(not(target_arch = "wasm32"))]
fn list_models_json(cfg: &AiCfg) -> Result<Value, String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = list_url(cfg, base);
    let mut req = agent().get(&url);
    if let Some(k) = request_key(cfg) {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut resp = req.call().map_err(|_| format!("Can't reach {base}."))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from {base}: {e}"))?;
    if !(200..300).contains(&status) {
        return Err(format!("The server answered {status}."));
    }
    serde_json::from_str(&text).map_err(|e| format!("{base} did not answer with JSON ({e})"))
}

/// Local Ollama: does `/api/show` list `vision` among the model's capabilities? `None` when the
/// server is too old to say.
fn show_vision(shown: &Value) -> Option<bool> {
    let caps = shown.get("capabilities")?.as_array()?;
    Some(caps.iter().any(|c| c.as_str() == Some("vision")))
}

/// Find models: the listed models this account can use to read a picture. `progress(done, total)` is
/// called as the pings go. Blocking, so call it off the UI thread.
#[cfg(not(target_arch = "wasm32"))]
pub fn check_models(cfg: &AiCfg, progress: &dyn Fn(usize, usize)) -> Result<Checked, String> {
    // The relay serves one model chosen by the worker; there is nothing to list or ping.
    if cfg.is_relay() {
        return Ok(Checked { ok: vec![cfg.model.clone()], total: 1, ..Default::default() });
    }
    let listing = list_models_json(cfg)?;
    let infos = parse_model_info(&cfg.provider, &listing);
    let total = infos.len();
    if total == 0 {
        return Err("The server listed no models.".to_string());
    }
    match cfg.provider.as_str() {
        "openrouter" => Ok(checked_from_flags(total, infos.iter().map(|m| (m.id.clone(), m.vision)).collect())),
        "ollama" => {
            let base = cfg.base.trim_end_matches('/');
            let flags = infos
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    progress(i, total);
                    let shown = post_json(&format!("{base}/api/show"), json!({"model": m.id}), None).ok();
                    (m.id.clone(), shown.as_ref().and_then(show_vision))
                })
                .collect();
            Ok(checked_from_flags(total, flags))
        }
        _ if total > PROBE_LIMIT => Ok(checked_from_flags(total, infos.iter().map(|m| (m.id.clone(), None)).collect())),
        _ => {
            let key = request_key(cfg);
            let mut results = Vec::new();
            for (i, m) in infos.iter().enumerate() {
                progress(i, total);
                let (url, body) = probe_request(cfg, &m.id);
                results.push((m.id.clone(), post_json(&url, body, key.as_deref()).map(|_| ())));
            }
            let c = assemble_checked(total, results);
            if c.ok.is_empty() { Err(nothing_answered(total, &c.left_out)) } else { Ok(c) }
        }
    }
}

/// Listing flags into a [`Checked`]: a model the listing says cannot see is left out; one it does not
/// say anything about stays.
fn checked_from_flags(total: usize, flags: Vec<(String, Option<bool>)>) -> Checked {
    let mut c = Checked { total, ..Default::default() };
    for (id, v) in flags {
        match v {
            Some(false) => c.left_out.push((id, "text only".to_string())),
            _ => c.ok.push(id),
        }
    }
    c
}

#[cfg(target_arch = "wasm32")]
pub async fn check_models_async(cfg: &AiCfg, progress: &dyn Fn(usize, usize)) -> Result<Checked, String> {
    // The relay serves one model chosen by the worker; there is nothing to list or ping.
    if cfg.is_relay() {
        return Ok(Checked { ok: vec![cfg.model.clone()], total: 1, ..Default::default() });
    }
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = list_url(cfg, base);
    let listing = get_json_async(&url, request_key(cfg).as_deref()).await.map_err(|e| cloud_error_hint(cfg, e))?;
    let infos = parse_model_info(&cfg.provider, &listing);
    let total = infos.len();
    if total == 0 {
        return Err("The server listed no models.".to_string());
    }
    match cfg.provider.as_str() {
        "openrouter" => Ok(checked_from_flags(total, infos.iter().map(|m| (m.id.clone(), m.vision)).collect())),
        "ollama" => {
            let mut flags = Vec::new();
            for (i, m) in infos.iter().enumerate() {
                progress(i, total);
                let shown = post_json_async(&format!("{base}/api/show"), &json!({"model": m.id}), None).await.ok();
                flags.push((m.id.clone(), shown.as_ref().and_then(show_vision)));
            }
            Ok(checked_from_flags(total, flags))
        }
        _ if total > PROBE_LIMIT => Ok(checked_from_flags(total, infos.iter().map(|m| (m.id.clone(), None)).collect())),
        _ => {
            let key = request_key(cfg);
            let mut results = Vec::new();
            for (i, m) in infos.iter().enumerate() {
                progress(i, total);
                let (url, body) = probe_request(cfg, &m.id);
                results.push((m.id.clone(), post_json_async(&url, &body, key.as_deref()).await.map(|_| ())));
            }
            let c = assemble_checked(total, results);
            if c.ok.is_empty() { Err(nothing_answered(total, &c.left_out)) } else { Ok(c) }
        }
    }
}

/// One message of a multi-turn chat (the figure agent's loop). `role` is "system", "user",
/// "assistant" or "tool"; `images` are PNG bytes shown to the model alongside `text`.
#[derive(Clone, Debug)]
pub struct Turn {
    pub role: &'static str,
    pub text: String,
    pub images: Vec<Vec<u8>>,
    /// An assistant turn's function calls (native tool calling).
    pub calls: Vec<ToolCall>,
    /// A "tool" turn: the call whose result `text` is.
    pub answers: Option<ToolCall>,
}

/// One function call a model asked for: `args` is the arguments object.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

/// A reply that may call tools instead of (or besides) answering in text.
#[derive(Clone, Debug, Default)]
pub struct ChatReply {
    pub text: String,
    pub calls: Vec<ToolCall>,
}

impl Turn {
    pub fn new(role: &'static str, text: impl Into<String>) -> Turn {
        Turn { role, text: text.into(), images: Vec::new(), calls: Vec::new(), answers: None }
    }

    /// The assistant's turn that called tools.
    pub fn calling(text: impl Into<String>, calls: Vec<ToolCall>) -> Turn {
        Turn { calls, ..Turn::new("assistant", text) }
    }

    /// The result of one tool call.
    pub fn tool_result(call: &ToolCall, text: impl Into<String>) -> Turn {
        Turn { answers: Some(call.clone()), ..Turn::new("tool", text) }
    }
}

/// One chat request, built once for the blocking and the browser transports.
struct ChatReq {
    url: String,
    body: Value,
    key: Option<String>,
    /// Where the reply text sits in the response JSON.
    pointer: &'static str,
    /// Where its tool calls sit.
    calls_pointer: &'static str,
}

/// `json` asks the provider for a JSON object (Ollama `format`, OpenAI `response_format`); without it
/// the reply is free text, which suits answers that are mostly code.
fn chat_request(cfg: &AiCfg, turns: &[Turn], temp: f64, json: bool, tools: Option<&Value>) -> Result<ChatReq, String> {
    match cfg.provider.as_str() {
        "ollama" => {
            let url = format!("{}/api/chat", cfg.base.trim_end_matches('/'));
            let messages: Vec<Value> = turns
                .iter()
                .map(|t| match (&t.answers, t.calls.is_empty()) {
                    (Some(call), _) => json!({"role": "tool", "content": t.text, "tool_name": call.name}),
                    (None, false) => json!({"role": t.role, "content": t.text,
                        "tool_calls": t.calls.iter().map(|c| json!({"function": {"name": c.name, "arguments": c.args}})).collect::<Vec<_>>()}),
                    _ => json!({"role": t.role, "content": t.text, "images": t.images.iter().map(|b| base64_encode(b)).collect::<Vec<_>>()}),
                })
                .collect();
            let mut body = json!({"model": cfg.model, "messages": messages, "stream": false, "options": {"temperature": temp}});
            if json { body["format"] = json!("json"); }
            match cfg.think.as_deref() {
                Some("off") => body["think"] = json!(false),
                // Models with only on/off thinking (Gemma 4) take a bool.
                Some("on") => body["think"] = json!(true),
                Some(level) => body["think"] = json!(level),
                None => {}
            }
            if let Some(t) = tools { body["tools"] = t.clone(); }
            Ok(ChatReq { url, body, key: None, pointer: "/message/content", calls_pointer: "/message/tool_calls" })
        }
        _ => {
            let base = cfg.base.trim_end_matches('/');
            if base.is_empty() {
                return Err("Set a base URL for the custom AI endpoint in Settings.".to_string());
            }
            let url = format!("{base}/chat/completions");
            let messages: Vec<Value> = turns
                .iter()
                .map(|t| {
                    if let Some(call) = &t.answers {
                        return json!({"role": "tool", "tool_call_id": call.id, "content": t.text});
                    }
                    if !t.calls.is_empty() {
                        return json!({"role": "assistant", "content": if t.text.is_empty() { Value::Null } else { json!(t.text) },
                            "tool_calls": t.calls.iter().map(|c| json!({"id": c.id, "type": "function",
                                "function": {"name": c.name, "arguments": c.args.to_string()}})).collect::<Vec<_>>()});
                    }
                    let mut content = vec![json!({"type": "text", "text": t.text})];
                    for b in &t.images {
                        content.push(json!({"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", base64_encode(b))}}));
                    }
                    json!({"role": t.role, "content": content})
                })
                .collect();
            let mut body = json!({"model": cfg.model, "stream": false, "temperature": temp, "messages": messages});
            if json { body["response_format"] = json!({"type": "json_object"}); }
            if cfg.is_relay() {
                // The relay answers in JSON unless asked for text; a question file is plain text.
                if !json { body["response_format"] = json!({"type": "text"}); }
                // A signed-in account's calls ride on the credit session its service opened.
                if let Some(s) = current_session() { body["session"] = json!(s); }
            }
            if let Some(level) = cfg.think.as_deref() {
                body["reasoning_effort"] = json!(match level { "off" => "none", "on" => "medium", l => l });
            }
            if let Some(t) = tools { body["tools"] = t.clone(); }
            Ok(ChatReq { url, body, key: request_key(cfg), pointer: "/choices/0/message/content", calls_pointer: "/choices/0/message/tool_calls" })
        }
    }
}

fn chat_reply_text(req: &ChatReq, reply: &Value) -> Result<String, String> {
    reply
        .pointer(req.pointer)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "The reply had no message content.".to_string())
}

/// The text and tool calls of a reply to a request made with tools. Ollama sends arguments as an
/// object, OpenAI-style APIs as a JSON string; both come out as an object.
fn chat_reply_full(req: &ChatReq, reply: &Value) -> Result<ChatReply, String> {
    let text = reply.pointer(req.pointer).and_then(Value::as_str).unwrap_or("").to_string();
    let calls: Vec<ToolCall> = reply
        .pointer(req.calls_pointer)
        .and_then(Value::as_array)
        .map(|a| a.iter().enumerate().filter_map(|(i, c)| {
            let f = c.get("function")?;
            let args = match f.get("arguments") {
                Some(Value::String(s)) => serde_json::from_str(s).unwrap_or(json!({})),
                Some(v) => v.clone(),
                None => json!({}),
            };
            Some(ToolCall {
                id: c.get("id").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("call_{i}")),
                name: f.get("name")?.as_str()?.to_string(),
                args,
            })
        }).collect())
        .unwrap_or_default();
    if text.is_empty() && calls.is_empty() {
        return Err("The reply had no message content.".to_string());
    }
    Ok(ChatReply { text, calls })
}

/// A conversation with function tools (`tools` is the "tools" array, the same shape for Ollama
/// and OpenAI-style APIs): free text and any calls back.
#[cfg(not(target_arch = "wasm32"))]
pub fn chat_turns_tools(cfg: &AiCfg, turns: &[Turn], temp: f64, tools: &Value) -> Result<ChatReply, String> {
    // A CLI has no function tools here; it gets the text LOOKUP protocol instead.
    if cli::is_cli(cfg) { return cli::chat(cfg, turns).map(|text| ChatReply { text, calls: Vec::new() }); }
    let req = chat_request(cfg, turns, temp, false, Some(tools))?;
    let reply = post_json(&req.url, req.body.clone(), req.key.as_deref()).map_err(|e| account_message(cfg, &e))?;
    log_served(cfg, &reply);
    chat_reply_full(&req, &reply)
}

/// The browser's [`chat_turns_tools`].
#[cfg(target_arch = "wasm32")]
pub async fn chat_turns_tools_async(cfg: &AiCfg, turns: &[Turn], temp: f64, tools: &Value) -> Result<ChatReply, String> {
    let req = chat_request(cfg, turns, temp, false, Some(tools))?;
    let reply = post_json_async(&req.url, &req.body, req.key.as_deref()).await.map_err(|e| account_message(cfg, &e))?;
    chat_reply_full(&req, &reply)
}

/// A router model (openrouter/free, openrouter/auto) names the model that actually answered;
/// DAD_LOG_SERVED prints it (never set in the browser).
fn log_served(cfg: &AiCfg, reply: &Value) {
    if std::env::var_os("DAD_LOG_SERVED").is_some()
        && let Some(served) = reply.get("model").and_then(Value::as_str)
        && served != cfg.model
    {
        eprintln!("served-by: {served}");
    }
}

/// A whole conversation in, the model's JSON reply out. Same providers and keys as [`transcribe`].
pub fn chat_turns(cfg: &AiCfg, turns: &[Turn], temp: f64) -> Result<String, String> {
    #[cfg(not(target_arch = "wasm32"))]
    if cli::is_cli(cfg) { return cli::chat(cfg, turns); }
    let req = chat_request(cfg, turns, temp, true, None)?;
    let reply = post_json(&req.url, req.body.clone(), req.key.as_deref()).map_err(|e| account_message(cfg, &e))?;
    chat_reply_text(&req, &reply)
}

/// [`chat_turns`] with a free-text reply (no JSON mode).
pub fn chat_turns_text(cfg: &AiCfg, turns: &[Turn], temp: f64) -> Result<String, String> {
    #[cfg(not(target_arch = "wasm32"))]
    if cli::is_cli(cfg) { return cli::chat(cfg, turns); }
    let req = chat_request(cfg, turns, temp, false, None)?;
    let reply = post_json(&req.url, req.body.clone(), req.key.as_deref()).map_err(|e| account_message(cfg, &e))?;
    log_served(cfg, &reply);
    chat_reply_text(&req, &reply)
}

/// The browser's [`chat_turns`] / [`chat_turns_text`] (`json` picks which).
#[cfg(target_arch = "wasm32")]
pub async fn chat_turns_async(cfg: &AiCfg, turns: &[Turn], temp: f64, json: bool) -> Result<String, String> {
    let req = chat_request(cfg, turns, temp, json, None)?;
    let reply = post_json_async(&req.url, &req.body, req.key.as_deref()).await.map_err(|e| account_message(cfg, &e))?;
    chat_reply_text(&req, &reply)
}

/// A JSON POST through the browser's fetch, for other modules (the D.A.D engine).
#[cfg(target_arch = "wasm32")]
pub async fn post_json_browser(url: &str, body: &Value) -> Result<Value, String> {
    post_json_async(url, body, None).await
}

/// What one converted page yields.
#[derive(Debug)]
pub struct Converted {
    pub doc: crate::formats::ImportDoc,
    /// Skipped questions, dropped figures, figures kept as plain images.
    pub warnings: Vec<String>,
    /// Pictures cropped from the page, and how many of those became editable Figures-tab figures.
    pub cropped: usize,
    pub redrawn: usize,
}

/// The whole Import path for one page image: model reply -> Ditto parse (figures cropped) ->
/// figure fitting (crops become editable Figures-tab figures when a generator matches).
/// One chat turn per page (the relay's import jobs are not used: see `convert_pages`).
pub fn convert_page(bytes: &[u8], mime: &str, hint: &str, cfg: &AiCfg, page_no: usize) -> Result<Converted, String> {
    #[cfg(not(target_arch = "wasm32"))]
    begin_service(cfg, "import", 1)?;
    convert_page_chat(bytes, mime, hint, cfg, page_no)
}

/// [`convert_page`] without the job route: the model answers one chat turn (retried on bad JSON).
pub fn convert_page_chat(bytes: &[u8], mime: &str, hint: &str, cfg: &AiCfg, page_no: usize) -> Result<Converted, String> {
    let raw = transcribe_checked(bytes, mime, cfg, &transcription_prompt(hint))?;
    let first = finish_page(&raw, bytes, cfg, page_no)?;
    Ok(redo_for_box(first, bytes, mime, hint, cfg, page_no))
}

/// After this long without an answer a relay call is sent again in parallel and the first answer wins.
/// A call normally answers in 1 to 12 s on the relay, but now and then one hangs for minutes.
#[cfg(not(target_arch = "wasm32"))]
const HEDGE_AFTER: Duration = Duration::from_secs(25);

/// `VERSHION_AI_HEDGE_SECS` overrides [`HEDGE_AFTER`] (used to test the race).
#[cfg(not(target_arch = "wasm32"))]
fn hedge_after() -> Duration {
    crate::env_var("VERSHION_AI_HEDGE_SECS").ok().and_then(|v| v.trim().parse::<u64>().ok()).map(Duration::from_secs).unwrap_or(HEDGE_AFTER)
}

/// Pictures dropped because their box was wrong (blank paper, off the page).
fn boxes_dropped(c: &Converted) -> usize {
    c.warnings.iter().filter(|w| w.contains("figure dropped")).count()
}

/// The model's picture box is the least stable part of a page read (on a real worksheet it was right
/// on one run and over the empty bottom of the page on the next three). When a figure was dropped for
/// its box, the page is asked once more, at a little temperature so the answer differs, with a note
/// about what went wrong; the answer that keeps more pictures wins. Never worse than the first read.
fn redo_for_box(first: Converted, bytes: &[u8], mime: &str, hint: &str, cfg: &AiCfg, page_no: usize) -> Converted {
    if boxes_dropped(&first) == 0 {
        return first;
    }
    let prompt = format!(
        "{}\n\nYour earlier answer for this page gave a picture box over a blank part of the page. Look at the page again and put the box tightly around the printed picture itself (the drawing and its labels), not around empty paper.",
        transcription_prompt(hint)
    );
    let second = transcribe_at(bytes, mime, cfg, &prompt, 0.3)
        .ok()
        .filter(|raw| crate::ditto::json_error(raw).is_none())
        .and_then(|raw| finish_page(&raw, bytes, cfg, page_no).ok());
    match second {
        Some(c) if c.cropped > first.cropped || (c.cropped == first.cropped && boxes_dropped(&c) < boxes_dropped(&first)) => c,
        _ => first,
    }
}

/// Pages read at the same time. Each page is its own chat request (about 20 s on the relay), so a
/// document takes roughly the time of its slowest page plus one more per `PAGE_PARALLEL` pages.
/// `VERSHION_AI_PARALLEL` overrides it (used to measure the best value).
#[cfg(not(target_arch = "wasm32"))]
fn page_parallel() -> usize {
    crate::env_var("VERSHION_AI_PARALLEL").ok().and_then(|v| v.trim().parse::<usize>().ok()).filter(|n| *n > 0).unwrap_or(PAGE_PARALLEL)
}
#[cfg(not(target_arch = "wasm32"))]
const PAGE_PARALLEL: usize = 12;

/// Every page of a document, a few at a time. Each page is one chat request, not an import job: the
/// relay's background job was cut off after about a page (the platform cancels work that outlives
/// the request that started it), and one request per page needs no polling. `progress(done, total)`
/// reports pages as they finish; results come back in page order. A page's figure cropping and
/// fitting run on the same worker thread right after its read.
#[cfg(not(target_arch = "wasm32"))]
pub fn convert_pages(pages: &[Vec<u8>], hint: &str, cfg: &AiCfg, progress: &(dyn Fn(usize, usize) + Sync)) -> Vec<Result<Converted, String>> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let total = pages.len();
    if let Err(e) = begin_service(cfg, "import", total) {
        return (0..total).map(|_| Err(e.clone())).collect();
    }
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<Result<Converted, String>>>> = (0..total).map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for _ in 0..page_parallel().min(total) {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(png) = pages.get(i) else { break };
                    let r = convert_page_chat(png, "image/png", hint, cfg, i + 1);
                    *slots[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
                    progress(done.fetch_add(1, Ordering::SeqCst) + 1, total);
                }
            });
        }
    });
    slots.into_iter().map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner()).unwrap_or_else(|| Err("page was not read".into()))).collect()
}

/// Parses one page's model reply into questions, crops its pictures and fits them to generators.
fn finish_page(raw: &str, bytes: &[u8], cfg: &AiCfg, page_no: usize) -> Result<Converted, String> {
    let text = extract_import_text(raw);
    if text.is_empty() {
        return Err("The model returned no text.".to_string());
    }
    if !crate::ditto::is_ditto(&text) {
        let doc = crate::formats::detect_and_parse(text.as_bytes(), "converted")?;
        return Ok(Converted { doc, warnings: Vec::new(), cropped: 0, redrawn: 0 });
    }
    let mut r = crate::ditto::import_text(&text, Some(bytes), page_no, "converted")?;
    let mut redrawn = 0;
    for (i, q) in r.doc.questions.iter_mut().enumerate() {
        if q.image.is_none() || q.figure.is_some() {
            continue;
        }
        let crop = crate::store::read_media(&q.image.as_ref().expect("checked").file).unwrap_or_default();
        // Up to 3 attempts: a picture the first generator misses gets another generator.
        match crate::figfit::fit_with_retries(&crop, &q.stem, cfg, 3) {
            Ok(Some(f)) => match crate::store::put_image(&f.png) {
                Ok(img) => {
                    q.image = Some(img);
                    q.figure = Some(f.spec);
                    redrawn += 1;
                }
                Err(e) => r.warnings.push(format!("Question {}: couldn't save the redrawn figure ({e}).", i + 1)),
            },
            Ok(None) => r.warnings.push(format!("Question {}: no Figures-tab generator matched its picture, kept the cropped image.", i + 1)),
            Err(e) => r.warnings.push(format!("Question {}: figure fitting failed ({e}), kept the cropped image.", i + 1)),
        }
    }
    Ok(Converted { doc: r.doc, warnings: r.warnings, cropped: r.figures, redrawn })
}

/// `transcribe`, then up to two retries when the reply isn't valid JSON: the error is quoted
/// back to the model and the temperature nudged up so it doesn't repeat the same slip.
pub fn transcribe_checked(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str) -> Result<String, String> {
    let mut reply = transcribe_at(bytes, mime, cfg, prompt, 0.0)?;
    for attempt in 1..=2 {
        // An empty reply is retried like a bad one: a burst of pages can come back empty.
        let problem = match crate::ditto::json_error(&reply) {
            Some(e) => format!("Your previous reply was not valid JSON ({e})."),
            None if reply.trim().is_empty() => "Your previous reply was empty.".to_string(),
            None => break,
        };
        let again = format!("{prompt}\n\n{problem} Output the complete package again as strictly valid JSON: commas between members, no trailing commas, no comments.");
        match transcribe_at(bytes, mime, cfg, &again, 0.2 * attempt as f64) {
            Ok(r) => reply = r,
            Err(_) => break,
        }
    }
    Ok(reply)
}

#[cfg(not(target_arch = "wasm32"))]
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).http_status_as_error(false).build().into()
}

/// Ollama's native chat API: images ride in the message's `images` array as raw base64.
fn ollama_chat(cfg: &AiCfg, prompt: &str, images: &[&[u8]], temp: f64) -> Result<String, String> {
    let url = format!("{}/api/chat", cfg.base.trim_end_matches('/'));
    let body = json!({
        "model": cfg.model,
        "messages": [{"role": "user", "content": prompt, "images": images.iter().map(|b| base64_encode(b)).collect::<Vec<_>>()}],
        "stream": false,
        "format": "json",
        "options": {"temperature": temp},
    });
    let reply = post_json(&url, body, None)?;
    reply
        .pointer("/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "The Ollama reply had no message content.".to_string())
}

/// OpenRouter and any custom OpenAI-compatible endpoint, with the image as a data URI.
fn openai_chat(cfg: &AiCfg, prompt: &str, mime: &str, images: &[&[u8]], temp: f64) -> Result<String, String> {
    #[cfg(not(target_arch = "wasm32"))]
    if cfg.is_relay() {
        return hedged(cfg, prompt, mime, images, temp);
    }
    openai_chat_once(cfg, prompt, mime, images, temp)
}

/// The relay call, raced against a second identical call once it has been slow for [`hedge_after`].
/// The first usable answer wins; the slower call is left to finish on its own thread.
#[cfg(not(target_arch = "wasm32"))]
fn hedged(cfg: &AiCfg, prompt: &str, mime: &str, images: &[&[u8]], temp: f64) -> Result<String, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
    let launch = |tx: std::sync::mpsc::Sender<Result<String, String>>| {
        let (c, p, m) = (cfg.clone(), prompt.to_string(), mime.to_string());
        let imgs: Vec<Vec<u8>> = images.iter().map(|b| b.to_vec()).collect();
        std::thread::spawn(move || {
            let refs: Vec<&[u8]> = imgs.iter().map(Vec::as_slice).collect();
            let _ = tx.send(openai_chat_once(&c, &p, &m, &refs, temp));
        });
    };
    launch(tx.clone());
    let (mut running, mut hedge_sent, mut last) = (1, false, Err("no reply".to_string()));
    loop {
        let got = if hedge_sent { rx.recv().ok() } else { rx.recv_timeout(hedge_after()).ok() };
        match got {
            Some(Ok(reply)) if !reply.trim().is_empty() => return Ok(reply),
            Some(other) => {
                running -= 1;
                last = other;
                if running == 0 {
                    return last;
                }
            }
            None if !hedge_sent => {
                hedge_sent = true;
                running += 1;
                launch(tx.clone());
            }
            None => return last,
        }
    }
}

/// Bases that answered 400 to `response_format`, for the life of the process.
fn refused_json_mode() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static SET: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = std::sync::OnceLock::new();
    SET.get_or_init(Default::default)
}

fn json_mode_refused(base: &str) -> bool {
    refused_json_mode().lock().map(|s| s.contains(base)).unwrap_or(false)
}

fn note_json_mode_refused(base: &str) {
    if let Ok(mut s) = refused_json_mode().lock() {
        s.insert(base.to_string());
    }
}

/// The credit session the relay opened for the service now running (None: the relay does not meter this caller).
static SERVICE_SESSION: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// What a started service left in the account's weekly allowance.
#[derive(Debug, Clone, PartialEq)]
pub struct Charged {
    pub credits: u32,
    pub left: Option<u32>,
}

/// Asks the relay to price and start a service (`import`, `figure`, `handwriting`, ...) before its model
/// calls. A relay that does not meter this caller answers without a session and the calls go through as
/// before; a metered one charges the credits, and every later relay call carries the session. Only the
/// built-in relay is metered, so any other provider returns at once.
#[cfg(not(target_arch = "wasm32"))]
pub fn begin_service(cfg: &AiCfg, service: &str, pages: usize) -> Result<Charged, String> {
    if !cfg.is_relay() {
        return Ok(Charged { credits: 0, left: None });
    }
    account_signed_in_check(cfg)?;
    let url = format!("{}/service", cfg.base.trim_end_matches('/'));
    let key = request_key(cfg);
    let reply = post_json(&url, json!({"service": service, "pages": pages}), key.as_deref()).map_err(|e| account_message(cfg, &credit_message(&e)))?;
    let session = reply.get("session").and_then(Value::as_str).map(str::to_string);
    if let Ok(mut s) = SERVICE_SESSION.lock() {
        *s = session;
    }
    let n = |k: &str| reply.get(k).and_then(Value::as_u64).map(|v| v as u32);
    Ok(Charged { credits: n("credits").unwrap_or(0), left: n("left") })
}

/// The browser version of [`begin_service`]: the same call, awaited.
#[cfg(target_arch = "wasm32")]
pub async fn begin_service_async(cfg: &AiCfg, service: &str, pages: usize) -> Result<Charged, String> {
    if !cfg.is_relay() {
        return Ok(Charged { credits: 0, left: None });
    }
    account_signed_in_check(cfg)?;
    let url = format!("{}/service", cfg.base.trim_end_matches('/'));
    let key = request_key(cfg);
    let reply = post_json_async(&url, &json!({"service": service, "pages": pages}), key.as_deref()).await.map_err(|e| account_message(cfg, &credit_message(&e)))?;
    let session = reply.get("session").and_then(Value::as_str).map(str::to_string);
    if let Ok(mut s) = SERVICE_SESSION.lock() {
        *s = session;
    }
    let n = |k: &str| reply.get(k).and_then(Value::as_u64).map(|v| v as u32);
    Ok(Charged { credits: n("credits").unwrap_or(0), left: n("left") })
}

/// The hosted AI has no address until the account is signed in: say so, instead of posting to a relative `/service`.
fn account_signed_in_check(cfg: &AiCfg) -> Result<(), String> {
    if cfg.provider == "account" && (cfg.base.trim().is_empty() || !account_ai_ready()) {
        return Err("Sign in first: Settings, then D.A.D account (or wait a moment if you just opened the page), then try again.".to_string());
    }
    Ok(())
}

/// [`begin_service`] on either build: the desktop's blocking call is ready at once, the browser awaits fetch.
pub async fn start_service(cfg: &AiCfg, service: &str, pages: usize) -> Result<Charged, String> {
    #[cfg(not(target_arch = "wasm32"))]
    { begin_service(cfg, service, pages) }
    #[cfg(target_arch = "wasm32")]
    { begin_service_async(cfg, service, pages).await }
}

/// What a refusal from the D.A.D server means for the teacher, in their words. Other AIs' errors pass
/// through unchanged (the Questions screen has its own sentences for those). Every sentence here starts
/// with "D.A.D" or "Your D.A.D", which the screen shows as it is.
pub fn account_message(cfg: &AiCfg, err: &str) -> String {
    if cfg.provider != "account" { return err.to_string(); }
    if err.starts_with("Sign in first") || err.starts_with("D.A.D") || err.starts_with("Your D.A.D") { return err.to_string(); }
    let status = err.find(" answered ").and_then(|i| err[i + 10..].get(..3)).and_then(|s| s.parse::<u16>().ok());
    let lower = err.to_ascii_lowercase();
    match status {
        Some(401) => "Your D.A.D sign-in has ended. Sign in again: Settings, then D.A.D account.".into(),
        Some(403) => "Your D.A.D account can't use D.A.D AI yet. Open Settings, then D.A.D account, to see why.".into(),
        Some(402) if lower.contains("session") => "D.A.D AI stopped partway through. Press Write again; it starts a fresh try.".into(),
        Some(429) if lower.contains("today") => "D.A.D AI has reached today's limit for everyone. Try again tomorrow, or pick another AI in Settings, then AI.".into(),
        Some(429) => "D.A.D AI is busy right now. Wait a minute and try again.".into(),
        Some(503) => "The D.A.D server isn't ready to write questions right now. Try again later, or pick another AI in Settings, then AI.".into(),
        Some(500..=599) => "D.A.D AI didn't answer this time. Wait a minute and try again.".into(),
        // A refused service carries the server's own count, without an " answered " prefix (credit_message).
        _ if lower.contains("credits") && lower.contains("you have") => "D.A.D AI is out of credits for this week. Free credits come back every Monday, or you can buy more in Settings, then D.A.D account.".into(),
        _ if lower.starts_with("can't reach") => "Couldn't reach the D.A.D server. Check your internet connection and try again.".into(),
        _ => err.to_string(),
    }
}

/// The relay's sentence for a refused service ("This needs 2 credits and you have 0 left this week.").
fn credit_message(err: &str) -> String {
    if let Some(i) = err.find("answered 402: ") {
        return err[i + "answered 402: ".len()..].trim().to_string();
    }
    err.to_string()
}

fn current_session() -> Option<String> {
    SERVICE_SESSION.lock().ok().and_then(|s| s.clone())
}

fn openai_chat_once(cfg: &AiCfg, prompt: &str, mime: &str, images: &[&[u8]], temp: f64) -> Result<String, String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL for the custom AI endpoint in Settings.".to_string());
    }
    let url = format!("{base}/chat/completions");
    let mut content = vec![json!({"type": "text", "text": prompt})];
    for b in images {
        content.push(json!({"type": "image_url", "image_url": {"url": format!("data:{mime};base64,{}", base64_encode(b))}}));
    }
    let mut body = json!({
        "model": cfg.model,
        "stream": false,
        "temperature": temp,
        "response_format": {"type": "json_object"},
        "messages": [{"role": "user", "content": content}],
    });
    if cfg.provider == "openrouter" {
        // Answer at once instead of reasoning first, and report what the call cost.
        body["reasoning"] = json!({"enabled": false});
        body["usage"] = json!({"include": true});
    }
    if cfg.is_relay() {
        if let Some(s) = current_session() {
            body["session"] = json!(s);
        }
    }
    // Some local servers refuse JSON mode (LM Studio answers 400 "'response_format.type' must be
    // 'json_schema' or 'text'"). Once one has, it is asked without it; the prompt already says JSON only.
    let custom = cfg.provider == "custom";
    if custom && json_mode_refused(base) {
        body.as_object_mut().map(|o| o.remove("response_format"));
    }
    let key = request_key(cfg);
    let reply = match post_json(&url, body.clone(), key.as_deref()) {
        Err(e) if custom && body.get("response_format").is_some() && e.contains("answered 400") && e.contains("response_format") => {
            note_json_mode_refused(base);
            body.as_object_mut().map(|o| o.remove("response_format"));
            post_json(&url, body, key.as_deref())?
        }
        other => other?,
    };
    reply
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "The reply had no message content.".to_string())
}

/// How long a job may run, polled every JOB_POLL, before the app stops waiting.
#[allow(dead_code)] // the app reads pages over chat; kept for when the relay's jobs run on Workflows
#[cfg(not(target_arch = "wasm32"))]
const JOB_WAIT: Duration = Duration::from_secs(25 * 60);
#[allow(dead_code)] // the app reads pages over chat; kept for when the relay's jobs run on Workflows
#[cfg(not(target_arch = "wasm32"))]
const JOB_POLL: Duration = Duration::from_millis(1500);

/// Sends pages to the relay's import job (`POST {base}/import`, then `GET {base}/import/<id>`) and
/// waits for it. One entry per page: the model's reply text, or why that page has none. `Err`
/// means the job could not be started or followed at all (an older relay, a refused request, a job
/// that failed outright), and the caller falls back to the chat path.
#[allow(dead_code)] // the app reads pages over chat; kept for when the relay's jobs run on Workflows
#[cfg(not(target_arch = "wasm32"))]
fn job_texts(pages: &[(&[u8], &str)], hint: &str, cfg: &AiCfg, progress: &dyn Fn(usize, usize)) -> Result<Vec<Result<String, String>>, String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set the relay URL in Settings.".to_string());
    }
    let key = request_key(cfg);
    let body = json!({
        "hint": hint.trim().chars().take(500).collect::<String>(),
        "pages": pages.iter().map(|(b, m)| json!({"mime": m, "b64": base64_encode(b)})).collect::<Vec<_>>(),
    });
    let id = post_json(&format!("{base}/import"), body, key.as_deref())?
        .get("job_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "The relay did not start a job.".to_string())?;
    let url = format!("{base}/import/{id}");
    let started = std::time::Instant::now();
    loop {
        let job = get_json(&url, key.as_deref())?;
        progress(job.get("pages_done").and_then(Value::as_u64).unwrap_or(0) as usize, pages.len());
        match job.get("status").and_then(Value::as_str) {
            Some("done") => return Ok(job_pages(&job, pages.len())),
            Some("failed") => return Err(job.get("error").and_then(Value::as_str).unwrap_or("The import job failed.").to_string()),
            _ if started.elapsed() > JOB_WAIT => return Err("The import job took too long.".to_string()),
            _ => std::thread::sleep(JOB_POLL),
        }
    }
}

/// The per-page texts of a finished job, in page order; a missing page counts as unread.
#[allow(dead_code)] // the app reads pages over chat; kept for when the relay's jobs run on Workflows
#[cfg(not(target_arch = "wasm32"))]
fn job_pages(job: &Value, total: usize) -> Vec<Result<String, String>> {
    let mut out: Vec<Result<String, String>> = (0..total).map(|_| Err("no reply".to_string())).collect();
    for p in job.pointer("/result/pages").and_then(Value::as_array).into_iter().flatten() {
        let Some(i) = p.get("page").and_then(Value::as_u64).and_then(|n| (n as usize).checked_sub(1)).filter(|i| *i < total) else { continue };
        out[i] = match p.get("text").and_then(Value::as_str) {
            Some(t) => Ok(t.to_string()),
            None => Err(p.get("error").and_then(Value::as_str).unwrap_or("no reply").to_string()),
        };
    }
    out
}

/// One JSON GET, answered like [`post_json`].
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn get_json(url: &str, key: Option<&str>) -> Result<Value, String> {
    let mut req = agent().get(url);
    if let Some(k) = key {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut resp = req.call().map_err(|e| format!("Can't reach {url}: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from {url}: {e}"))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{url} did not answer with JSON ({e})."))?;
    if !(200..300).contains(&status) {
        return Err(format!("{url} answered {status}: {}", v.pointer("/error/message").and_then(Value::as_str).unwrap_or("request failed")));
    }
    Ok(v)
}

/// One JSON POST, with the same error shapes for both providers; answers with the body.
#[cfg(not(target_arch = "wasm32"))]
fn post_json(url: &str, body: Value, key: Option<&str>) -> Result<Value, String> {
    let mut req = agent().post(url).header("Content-Type", "application/json");
    if let Some(k) = key {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut resp = req.send(body.to_string()).map_err(|e| format!("Can't reach {url}: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from {url}: {e}"))?;
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{url} did not answer with JSON ({e})."))?;
    if !(200..300).contains(&status) {
        let msg = v.pointer("/error/message").and_then(Value::as_str).or_else(|| v.get("error").and_then(Value::as_str)).unwrap_or("request failed");
        return Err(format!("{url} answered {status}: {msg}"));
    }
    Ok(v)
}

/// The blocking path is not used in the browser; async callers use [`post_json_async`].
#[cfg(target_arch = "wasm32")]
fn post_json(url: &str, _body: Value, _key: Option<&str>) -> Result<Value, String> {
    Err(format!("This build uses the async browser transport (cannot reach {url})"))
}

/// Browser transport: one JSON POST with CORS and a 300 s timeout.
#[cfg(target_arch = "wasm32")]
async fn post_json_async(url: &str, body: &Value, key: Option<&str>) -> Result<Value, String> {
    use wasm_bindgen::JsCast as _;
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let headers = web_sys::Headers::new().map_err(js)?;
    headers.set("Content-Type", "application/json").map_err(js)?;
    if let Some(k) = key {
        headers.set("Authorization", &format!("Bearer {k}")).map_err(js)?;
    }
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_mode(web_sys::RequestMode::Cors);
    init.set_body(&wasm_bindgen::JsValue::from_str(&body.to_string()));
    init.set_headers(&headers);
    let signal = web_sys::AbortSignal::timeout_with_u32(300_000);
    init.set_signal(Some(&signal));
    let window = web_sys::window().ok_or("no window")?;
    let promise = window.fetch_with_str_and_init(url, &init);
    let resp = wasm_bindgen_futures::JsFuture::from(promise).await.map_err(|_| {
        // A page cannot tell a CORS refusal from a server that is not there.
        format!("Can't reach {url}. The server may be off, or it does not allow requests from this page (CORS).")
    })?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "not a response".to_string())?;
    let status = resp.status();
    let text = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(js)?).await.map_err(js)?.as_string().unwrap_or_default();
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{url} did not answer with JSON ({e})."))?;
    if !(200..300).contains(&status) {
        let msg = v.pointer("/error/message").and_then(Value::as_str).or_else(|| v.get("error").and_then(Value::as_str)).unwrap_or("request failed");
        return Err(format!("{url} answered {status}: {msg}"));
    }
    Ok(v)
}

#[cfg(target_arch = "wasm32")]
async fn ollama_chat_async(cfg: &AiCfg, prompt: &str, images: &[&[u8]], temp: f64) -> Result<String, String> {
    let url = format!("{}/api/chat", cfg.base.trim_end_matches('/'));
    let body = json!({
        "model": cfg.model,
        "messages": [{"role": "user", "content": prompt, "images": images.iter().map(|b| base64_encode(b)).collect::<Vec<_>>()}],
        "stream": false,
        "format": "json",
        "options": {"temperature": temp},
    });
    let reply = post_json_async(&url, &body, None).await?;
    reply
        .pointer("/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "The Ollama reply had no message content.".to_string())
}

#[cfg(target_arch = "wasm32")]
async fn openai_chat_async(cfg: &AiCfg, prompt: &str, mime: &str, images: &[&[u8]], temp: f64) -> Result<String, String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL for the custom AI endpoint in Settings.".to_string());
    }
    let url = format!("{base}/chat/completions");
    let mut content = vec![json!({"type": "text", "text": prompt})];
    for b in images {
        content.push(json!({"type": "image_url", "image_url": {"url": format!("data:{mime};base64,{}", base64_encode(b))}}));
    }
    let mut body = json!({
        "model": cfg.model,
        "stream": false,
        "temperature": temp,
        "response_format": {"type": "json_object"},
        "messages": [{"role": "user", "content": content}],
    });
    if cfg.provider == "openrouter" {
        body["reasoning"] = json!({"enabled": false});
        body["usage"] = json!({"include": true});
    }
    if cfg.is_relay() {
        if let Some(s) = current_session() {
            body["session"] = json!(s);
        }
    }
    // A local server that refuses JSON mode is asked without it (see `openai_chat_once`).
    let custom = cfg.provider == "custom";
    if custom && json_mode_refused(base) {
        body.as_object_mut().map(|o| o.remove("response_format"));
    }
    let key = request_key(cfg);
    let first = post_json_async(&url, &body, key.as_deref()).await;
    let reply = match first {
        Err(e) if custom && body.get("response_format").is_some() && e.contains("answered 400") && e.contains("response_format") => {
            note_json_mode_refused(base);
            body.as_object_mut().map(|o| o.remove("response_format"));
            post_json_async(&url, &body, key.as_deref()).await
        }
        other => other,
    }
    .map_err(|e| cloud_error_hint(cfg, e))?;
    reply
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "The reply had no message content.".to_string())
}

#[cfg(target_arch = "wasm32")]
async fn transcribe_at_async(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str, temp: f64) -> Result<String, String> {
    match cfg.provider.as_str() {
        "ollama" => ollama_chat_async(cfg, prompt, &[bytes], temp).await,
        _ => openai_chat_async(cfg, prompt, mime, &[bytes], temp).await,
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn transcribe_checked_async(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str) -> Result<String, String> {
    let mut reply = transcribe_at_async(bytes, mime, cfg, prompt, 0.0).await?;
    if crate::ditto::json_error(&reply).is_none() {
        return Ok(reply);
    }
    for attempt in 1..=2 {
        let again = format!(
            "{prompt}\n\nYour previous reply could not be used: {}. Answer again with ONLY the JSON object.",
            crate::ditto::json_error(&reply).unwrap_or_default()
        );
        match transcribe_at_async(bytes, mime, cfg, &again, 0.2 * attempt as f64).await {
            Ok(r) => {
                reply = r;
                if crate::ditto::json_error(&reply).is_none() {
                    break;
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(reply)
}

#[cfg(target_arch = "wasm32")]
pub async fn ask_images_async(images: &[&[u8]], cfg: &AiCfg, prompt: &str, temp: f64) -> Result<String, String> {
    match cfg.provider.as_str() {
        "ollama" => ollama_chat_async(cfg, prompt, images, temp).await,
        _ => openai_chat_async(cfg, prompt, "image/png", images, temp).await,
    }
}

/// The browser version of [`convert_page`]: same parse and crop path, awaited transport.
#[cfg(target_arch = "wasm32")]
pub async fn convert_page_async(bytes: &[u8], mime: &str, hint: &str, cfg: &AiCfg, page_no: usize) -> Result<Converted, String> {
    begin_service_async(cfg, "import", 1).await?;
    let raw = transcribe_checked_async(bytes, mime, cfg, &transcription_prompt(hint)).await?;
    let text = extract_import_text(&raw);
    if text.is_empty() {
        return Err("The model returned no text.".to_string());
    }
    if !crate::ditto::is_ditto(&text) {
        let doc = crate::formats::detect_and_parse(text.as_bytes(), "converted")?;
        return Ok(Converted { doc, warnings: Vec::new(), cropped: 0, redrawn: 0 });
    }
    let mut r = crate::ditto::import_text(&text, Some(bytes), page_no, "converted")?;
    let mut redrawn = 0;
    for (i, q) in r.doc.questions.iter_mut().enumerate() {
        if q.image.is_none() || q.figure.is_some() {
            continue;
        }
        let crop = crate::store::read_media(&q.image.as_ref().expect("checked").file).unwrap_or_default();
        match crate::figfit::fit_async_with_retries(&crop, &q.stem, cfg, 3).await {
            Ok(Some(f)) => match crate::store::put_image(&f.png) {
                Ok(img) => {
                    q.image = Some(img);
                    q.figure = Some(f.spec);
                    redrawn += 1;
                }
                Err(e) => r.warnings.push(format!("Question {}: couldn't save the redrawn figure ({e}).", i + 1)),
            },
            Ok(None) => r.warnings.push(format!("Question {}: no Figures-tab generator matched its picture, kept the cropped image.", i + 1)),
            Err(e) => r.warnings.push(format!("Question {}: figure fitting failed ({e}), kept the cropped image.", i + 1)),
        }
    }
    Ok(Converted { doc: r.doc, warnings: r.warnings, cropped: r.figures, redrawn })
}

/// Strips an accidental markdown fence (with its language tag) and outer whitespace; the
/// `formats` parsers tolerate everything else a model might add around the import text.
pub fn extract_import_text(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(rest) = s.strip_prefix("```")
        && let Some((_, body)) = rest.split_once('\n')
    {
        let body = body.trim_end();
        let body = body.strip_suffix("```").map_or(body, str::trim_end);
        s = body;
    }
    s.trim().to_string()
}

/// Standard base64 (RFC 4648, with padding). Small local encoder; base64 is not a crate here.
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let a = u32::from(chunk[0]);
        let b = chunk.get(1).copied().map_or(0, u32::from);
        let c = chunk.get(2).copied().map_or(0, u32::from);
        let n = (a << 16) | (b << 8) | c;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(provider: &str, model: &str, base: &str) -> Settings {
        Settings { ai_provider: provider.into(), ai_model: model.into(), ai_base: base.into(), dark: bool::default(), ..Default::default() }
    }

    #[test]
    fn prompt_names_every_type_and_the_figure_rules() {
        let p = transcription_prompt("");
        for ty in ["multiple-choice", "true-false", "matching", "fill-in-the-blank", "short-answer", "multipart"] {
            assert!(p.contains(ty), "prompt must name {ty}");
        }
        for must in ["___", "\"box\"", "queshion/ditto", "No markdown fences", "[ymin, xmin, ymax, xmax]", "0 to 1000 grid"] {
            assert!(p.contains(must), "prompt must mention {must}");
        }
    }

    #[test]
    fn prompt_appends_the_user_hint() {
        let p = transcription_prompt("page 2 is multiple choice");
        assert!(p.contains("User note about this source:\npage 2 is multiple choice"), "{p}");
        // Empty hint changes nothing.
        assert_eq!(transcription_prompt("  "), TRANSCRIBE_PROMPT);
    }

    #[test]
    fn extract_strips_fences_and_whitespace() {
        assert_eq!(extract_import_text("  \n```text\nMC\nQ?\n*a. 1\n```\n  "), "MC\nQ?\n*a. 1");
        assert_eq!(extract_import_text("```\nMC\nQ?\n*a. 1\n```"), "MC\nQ?\n*a. 1");
        assert_eq!(extract_import_text(" MC\nQ? "), "MC\nQ?");
        // No content line after the fence: left as-is for the parsers to report.
        assert_eq!(extract_import_text("```junk"), "```junk");
    }

    /// A one-connection-per-request stub relay: answers `replies` in order, records each request line
    /// and body, and stops when they run out.
    fn stub_relay(replies: Vec<(u16, String)>) -> (String, std::thread::JoinHandle<Vec<(String, String)>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/ai", listener.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for (status, body) in replies {
                let (mut sock, _) = listener.accept().unwrap();
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let (head_end, want) = loop {
                    let n = sock.read(&mut chunk).unwrap();
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..p]).to_ascii_lowercase();
                        let len = head.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
                        break (p + 4, len);
                    }
                };
                while buf.len() < head_end + want {
                    let n = sock.read(&mut chunk).unwrap();
                    buf.extend_from_slice(&chunk[..n]);
                }
                let line = String::from_utf8_lossy(&buf).lines().next().unwrap_or("").to_string();
                seen.push((line, String::from_utf8_lossy(&buf[head_end..]).to_string()));
                let reply = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                sock.write_all(reply.as_bytes()).unwrap();
            }
            seen
        });
        (base, h)
    }

    fn relay_cfg(base: &str) -> AiCfg {
        AiCfg { provider: "workers".into(), base: base.into(), model: String::new(), think: None }
    }

    #[test]
    fn the_hosted_ai_goes_to_the_account_server_with_the_accounts_token() {
        let mut st = Settings { ai_provider: "account".into(), ..Default::default() };
        st.ai_base = "https://typed.example/ignored".into();
        set_account_ai(Some(("https://pool.example/".into(), std::sync::Arc::new(|| Some("tok-1".to_string())))));
        let cfg = AiCfg::from(&st);
        assert_eq!((cfg.provider.as_str(), cfg.base.as_str()), ("account", "https://pool.example/ai"));
        assert!(cfg.is_relay() && !cfg.uses_key() && cfg.has_key_or_needs_none());
        assert_eq!(request_key(&cfg).as_deref(), Some("tok-1"));
        assert!(account_ai_ready());
        // signed out: no address and no token, so nothing is sent for anybody else's account
        set_account_ai(None);
        let cfg = AiCfg::from(&st);
        assert!(cfg.base.is_empty() && request_key(&cfg).is_none() && !account_ai_ready());
    }

    #[test]
    fn a_job_is_started_followed_and_read_in_page_order() {
        let done = r#"{"status":"done","pages_done":3,"pages_total":3,"error":null,"result":{"pages":[{"page":2,"error":"model_failed"},{"page":1,"text":"ONE"},{"page":3,"text":"THREE"}]}}"#;
        let (base, h) = stub_relay(vec![
            (202, r#"{"job_id":"abc"}"#.into()),
            (200, r#"{"status":"running","pages_done":1,"pages_total":3,"result":{"pages":[]}}"#.into()),
            (200, done.into()),
        ]);
        let seen_progress = std::sync::Mutex::new(Vec::new());
        let pages: [(&[u8], &str); 3] = [(b"p1", "image/png"), (b"p2", "image/png"), (b"p3", "image/png")];
        let out = job_texts(&pages, "  Quiz 4  ", &relay_cfg(&base), &|d, t| seen_progress.lock().unwrap().push((d, t))).unwrap();
        // Page order, not the order the worker listed them in; a failed page says why.
        assert_eq!(out, vec![Ok("ONE".to_string()), Err("model_failed".to_string()), Ok("THREE".to_string())]);
        assert_eq!(*seen_progress.lock().unwrap(), vec![(1, 3), (3, 3)]);
        let seen = h.join().unwrap();
        assert_eq!(seen[0].0, "POST /ai/import HTTP/1.1");
        assert_eq!(seen[1].0, "GET /ai/import/abc HTTP/1.1");
        let sent: Value = serde_json::from_str(&seen[0].1).unwrap();
        assert_eq!(sent["hint"], "Quiz 4");
        assert_eq!(sent["pages"].as_array().unwrap().len(), 3);
        assert_eq!(sent["pages"][0]["mime"], "image/png");
        assert_eq!(sent["pages"][0]["b64"], base64_encode(b"p1"));
    }

    #[test]
    fn a_job_the_relay_refuses_or_fails_is_an_error_so_the_caller_can_fall_back() {
        let pages: [(&[u8], &str); 1] = [(b"p", "image/png")];
        // An older relay without the job routes.
        let (base, h) = stub_relay(vec![(404, r#"{"error":{"message":"not found"}}"#.into())]);
        assert!(job_texts(&pages, "", &relay_cfg(&base), &|_, _| {}).unwrap_err().contains("404"));
        h.join().unwrap();
        // A quota refusal names itself.
        let (base, h) = stub_relay(vec![(429, r#"{"error":{"code":"quota_exhausted","message":"The AI allowance for today is used up."}}"#.into())]);
        assert!(job_texts(&pages, "", &relay_cfg(&base), &|_, _| {}).unwrap_err().contains("used up"));
        h.join().unwrap();
        // A job that failed outright.
        let (base, h) = stub_relay(vec![(202, r#"{"job_id":"j"}"#.into()), (200, r#"{"status":"failed","error":"No page could be read.","result":{"pages":[]}}"#.into())]);
        assert_eq!(job_texts(&pages, "", &relay_cfg(&base), &|_, _| {}).unwrap_err(), "No page could be read.");
        h.join().unwrap();
        // No URL set.
        assert!(job_texts(&pages, "", &relay_cfg(""), &|_, _| {}).unwrap_err().contains("relay URL"));
    }

    #[test]
    fn a_page_the_job_did_not_return_counts_as_unread() {
        let job: Value = serde_json::from_str(r#"{"result":{"pages":[{"page":1,"text":"A"},{"page":9,"text":"stray"},{"page":0,"text":"zero"}]}}"#).unwrap();
        assert_eq!(job_pages(&job, 2), vec![Ok("A".to_string()), Err("no reply".to_string())]);
    }

    #[test]
    fn cfg_maps_the_relay_provider() {
        let c = AiCfg::from(&settings("workers", "", "https://relay.example/ai/"));
        assert_eq!(c.provider, "workers");
        assert_eq!(c.base, "https://relay.example/ai");
        assert_eq!(c.model, WORKERS_DEFAULT_MODEL);
        assert!(c.uses_key(), "the relay needs its relay key");
        assert!(c.key_source_help().contains("relay"));
        // The aliases map to the same provider; an empty base stays empty so chat_turns says to set it.
        assert_eq!(AiCfg::from(&settings("relay", "", "")).provider, "workers");
        assert_eq!(AiCfg::from(&settings("workers-ai", "", "")).base, "");
    }

    #[test]
    fn a_self_hosted_ollama_url_is_used_and_a_bad_one_falls_back() {
        let mut st = settings("ollama", "", "https://not-for-ollama.example/v1");
        assert_eq!(AiCfg::from(&st).base, OLLAMA_DEFAULT_BASE, "the Custom URL must not leak into Ollama");
        st.ollama_base = " http://gpu-box.lan:11434/ ".into();
        assert_eq!(AiCfg::from(&st).base, "http://gpu-box.lan:11434");
        st.ollama_base = "gpu-box.lan".into();
        assert_eq!(AiCfg::from(&st).base, OLLAMA_DEFAULT_BASE, "a URL without a scheme is ignored");
    }

    #[test]
    fn a_server_that_refuses_json_mode_is_asked_again_without_it_and_remembered() {
        let refusal = r#"{"error":"'response_format.type' must be 'json_schema' or 'text'"}"#;
        let ok = r#"{"choices":[{"message":{"content":"{\"ok\":true}"}}]}"#;
        let (base, h) = stub_relay(vec![(400, refusal.into()), (200, ok.into()), (200, ok.into())]);
        let cfg = AiCfg { provider: "custom".into(), base: base.clone(), model: "m".into(), think: None };
        assert_eq!(openai_chat_once(&cfg, "p", "image/png", &[], 0.0).unwrap(), "{\"ok\":true}");
        assert_eq!(openai_chat_once(&cfg, "p", "image/png", &[], 0.0).unwrap(), "{\"ok\":true}");
        let seen = h.join().unwrap();
        assert_eq!(seen.len(), 3);
        assert!(seen[0].1.contains("response_format"));
        assert!(!seen[1].1.contains("response_format"), "the retry drops JSON mode");
        assert!(!seen[2].1.contains("response_format"), "and so does the next call");
    }

    #[test]
    fn json_mode_is_only_dropped_for_a_400_that_names_it() {
        let (base, h) = stub_relay(vec![(400, r#"{"error":"model not found"}"#.into())]);
        let cfg = AiCfg { provider: "custom".into(), base, model: "m".into(), think: None };
        assert!(openai_chat_once(&cfg, "p", "image/png", &[], 0.0).is_err());
        assert_eq!(h.join().unwrap().len(), 1, "no retry for an unrelated 400");
    }

    #[test]
    fn openrouter_is_asked_not_to_reason_and_to_report_cost() {
        let ok = r#"{"choices":[{"message":{"content":"{}"}}]}"#;
        let (base, h) = stub_relay(vec![(200, ok.into())]);
        let cfg = AiCfg { provider: "openrouter".into(), base, model: "m".into(), think: None };
        openai_chat_once(&cfg, "p", "image/png", &[], 0.0).unwrap();
        let body = &h.join().unwrap()[0].1;
        assert!(body.contains(r#""reasoning":{"enabled":false}"#) && body.contains(r#""usage":{"include":true}"#));
    }

    #[test]
    fn the_hosted_ai_with_nobody_signed_in_says_so_and_sends_nothing() {
        // The address comes from the signed-in account; without it the old code posted to a relative "/service".
        let cfg = AiCfg { provider: "account".into(), base: String::new(), model: "m".into(), think: None };
        let err = begin_service(&cfg, "import", 1).unwrap_err();
        // It names a place the teacher can find: Settings, then D.A.D account.
        assert!(err.starts_with("Sign in first: Settings, then D.A.D account"), "{err}");
    }

    #[test]
    fn a_service_session_is_started_and_rides_on_later_calls() {
        let ok = r#"{"choices":[{"message":{"content":"{}"}}]}"#;
        let started = r#"{"session":"abc123","credits":2,"left":38}"#;
        let (base, h) = stub_relay(vec![(200, started.into()), (200, ok.into())]);
        let cfg = AiCfg { provider: "workers".into(), base, model: String::new(), think: None };
        let c = begin_service(&cfg, "import", 4).unwrap();
        assert_eq!(c, Charged { credits: 2, left: Some(38) });
        openai_chat_once(&cfg, "p", "image/png", &[], 0.0).unwrap();
        let seen = h.join().unwrap();
        assert!(seen[0].0.contains("/ai/service"));
        assert!(seen[0].1.contains(r#""service":"import""#) && seen[0].1.contains(r#""pages":4"#));
        assert!(seen[1].1.contains(r#""session":"abc123""#));
        let (base, h) = stub_relay(vec![(200, r#"{"session":null,"credits":0,"unlimited":true}"#.into()), (200, ok.into())]);
        let cfg = AiCfg { provider: "workers".into(), base, model: String::new(), think: None };
        begin_service(&cfg, "import", 1).unwrap();
        openai_chat_once(&cfg, "p", "image/png", &[], 0.0).unwrap();
        assert!(!h.join().unwrap()[1].1.contains("session"));
    }

    #[test]
    fn dad_ai_refusals_read_as_teacher_sentences() {
        let dad = AiCfg { provider: "account".into(), base: "http://x/ai".into(), model: String::new(), think: None };
        let m = |e: &str| account_message(&dad, e);
        assert!(m("This needs 1 credits and you have 0 left this week and 0 purchased.").starts_with("D.A.D AI is out of credits for this week."));
        assert!(m("http://x/ai/chat/completions answered 401: Sign in first, or use the relay key.").starts_with("Your D.A.D sign-in has ended."));
        assert!(m("http://x/ai/chat/completions answered 402: This AI session has ended or used its calls.").contains("Press Write again"));
        assert!(m("http://x/ai/service answered 429: The AI allowance for today is used up.").contains("today's limit"));
        assert!(m("http://x/ai/chat/completions answered 503: This deployment has no OpenRouter key.").starts_with("The D.A.D server isn't ready"));
        assert!(m("Can't reach http://x/ai/service: Connection refused").starts_with("Couldn't reach the D.A.D server"));
        // Every one is shown as written, not replaced by the general "out of credit with your AI provider" line.
        for e in ["This needs 1 credits and you have 0 left this week and 0 purchased.", "x answered 401: no", "x answered 503: no", "Can't reach x: y"] {
            assert_eq!(crate::mom::form::plain_error(&m(e)), m(e));
        }
        // Another AI's errors are not touched here.
        let other = AiCfg { provider: "openrouter".into(), ..dad.clone() };
        assert_eq!(account_message(&other, "x answered 401: bad key"), "x answered 401: bad key");
    }

    #[test]
    fn relay_chat_asks_for_text_when_it_wants_text_and_carries_the_session() {
        let dad = AiCfg { provider: "account".into(), base: "http://x/ai".into(), model: String::new(), think: None };
        let turns = [Turn::new("user", "hi".to_string())];
        let text = chat_request(&dad, &turns, 0.2, false, None).unwrap();
        assert_eq!(text.body["response_format"], json!({"type": "text"}));
        let json_req = chat_request(&dad, &turns, 0.2, true, None).unwrap();
        assert_eq!(json_req.body["response_format"], json!({"type": "json_object"}));
        let other = AiCfg { provider: "openrouter".into(), ..dad };
        assert!(chat_request(&other, &turns, 0.2, false, None).unwrap().body.get("response_format").is_none());
    }

    #[test]
    fn a_refused_service_says_what_the_relay_said() {
        let body = r#"{"error":{"code":"insufficient_credits","message":"This needs 2 credits and you have 0 left this week."}}"#;
        let (base, h) = stub_relay(vec![(402, body.into())]);
        let cfg = AiCfg { provider: "workers".into(), base, model: String::new(), think: None };
        assert_eq!(begin_service(&cfg, "import", 1).unwrap_err(), "This needs 2 credits and you have 0 left this week.");
        h.join().unwrap();
        assert_eq!(begin_service(&AiCfg { provider: "ollama".into(), base: String::new(), model: String::new(), think: None }, "import", 1).unwrap().credits, 0);
    }

    #[test]
    fn cfg_maps_the_three_providers() {
        let c = AiCfg::from(&settings("ollama", "", ""));
        assert_eq!(c.provider, "ollama");
        assert_eq!(c.base, OLLAMA_DEFAULT_BASE);
        assert_eq!(c.model, OLLAMA_DEFAULT_MODEL);

        let c = AiCfg::from(&settings("openrouter", "vendor/my-model", ""));
        assert_eq!(c.provider, "openrouter");
        assert_eq!(c.base, OPENROUTER_BASE);
        assert_eq!(c.model, "vendor/my-model");

        let c = AiCfg::from(&settings("custom", "", "https://api.example.com/v1/"));
        assert_eq!(c.provider, "custom");
        assert_eq!(c.base, "https://api.example.com/v1");
        assert_eq!(c.model, CUSTOM_DEFAULT_MODEL);

        // Empty or unknown provider settings fall back to local Ollama.
        let c = AiCfg::from(&Settings::default());
        assert_eq!(c.provider, "ollama");
        assert_eq!(c.model, OLLAMA_DEFAULT_MODEL);
    }

    #[test]
    fn key_help_names_the_env_var() {
        assert!(AiCfg::from(&settings("openrouter", "", "")).key_source_help().contains(OPENROUTER_KEY_ENV));
        assert!(AiCfg::from(&settings("custom", "", "")).key_source_help().contains(CUSTOM_KEY_ENV));
        assert!(AiCfg::from(&settings("ollama", "", "")).key_source_help().contains("no API key"));
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn poll_reports_a_job_that_ended_without_a_result() {
        let (tx, rx) = channel::<Result<Converted, String>>();
        drop(tx);
        let mut ai = Ai { job: Some(rx) };
        assert!(matches!(ai.poll(), Some(Err(_))), "a dropped sender is an error, not silence");
        assert!(ai.poll().is_none(), "the ended job must not stick");
    }

    #[test]
    fn poll_silently_waits_for_a_running_job() {
        let (tx, rx) = channel::<Result<Converted, String>>();
        let mut ai = Ai { job: Some(rx) };
        assert!(ai.poll().is_none());
        assert!(ai.job.is_some());
        let doc = crate::formats::detect_and_parse(b"MC\nQ?\n*a. 1\nb. 2\n", "x").unwrap();
        tx.send(Ok(Converted { doc, warnings: Vec::new(), cropped: 0, redrawn: 0 })).unwrap();
        let _ = tx; // keep the channel alive until the reply is read
        assert_eq!(ai.poll().unwrap().unwrap().doc.questions.len(), 1);
        assert!(ai.poll().is_none(), "the job is consumed once read");
    }

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vershion-keytest-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_pasted_key_keeps_only_the_key() {
        for raw in ["  Key: sk-or-v1-abc123  ", "OPENROUTER_API_KEY=sk-or-v1-abc123", "\"sk-or-v1-abc123\"\n",
                    "Your key is `sk-or-v1-abc123`.", "Bearer sk-or-v1-abc123", "sk-or-v1-abc123"] {
            assert_eq!(clean_key(raw), "sk-or-v1-abc123", "{raw:?}");
        }
        // Base64-style keys keep their padding; a colon inside a key that has no label stays.
        assert_eq!(clean_key("abcDEF123=="), "abcDEF123==");
        assert_eq!(key_shape_problem("openrouter", "Key: sk-or-v1-abc"), None);
        assert!(key_shape_problem("openrouter", "sk-ant-abc").unwrap().contains("sk-or-"));
        assert_eq!(key_shape_problem("cloud", "anything"), None);
    }

    #[test]
    fn key_store_roundtrip_remove_and_perms() {
        let d = tmpdir("rt");
        assert!(load_keys_in(&d).get("cloud").is_none());
        set_key_in(&d, "cloud", Some("  sk-dummy-1 ")).unwrap();
        set_key_in(&d, "openrouter", Some("sk-dummy-2")).unwrap();
        let k = load_keys_in(&d);
        assert_eq!(k.get("cloud"), Some("sk-dummy-1"));
        assert_eq!(k.get("openrouter"), Some("sk-dummy-2"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let m = std::fs::metadata(d.join(KEYS_FILE)).unwrap().permissions().mode();
            assert_eq!(m & 0o777, 0o600);
        }
        assert!(!d.join("ai-keys.json.tmp").exists());
        set_key_in(&d, "cloud", None).unwrap();
        assert!(load_keys_in(&d).get("cloud").is_none());
        assert_eq!(load_keys_in(&d).get("openrouter"), Some("sk-dummy-2"));
        assert!(!format!("{:?}", load_keys_in(&d)).contains("sk-dummy"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn key_resolution_order_saved_env_file() {
        let home = tmpdir("home");
        std::fs::create_dir_all(home.join(".config/ollama-cloud")).unwrap();
        std::fs::write(home.join(CLOUD_KEY_FILE), r#"{"apiKey":"from-file"}"#).unwrap();
        let cfg = AiCfg::from(&settings("cloud", "", ""));
        let env_set = |v: &str| (v == CLOUD_KEY_ENV).then(|| "from-env".to_string());
        let env_none = |_: &str| None;
        let mut saved = Keys::default();
        // file only
        let r = resolve_key(&cfg, &saved, &env_none, Some(&home)).unwrap();
        assert_eq!((r.0.as_str(), r.1), ("from-file", KeySource::CredentialsFile));
        // env beats file
        let r = resolve_key(&cfg, &saved, &env_set, Some(&home)).unwrap();
        assert_eq!((r.0.as_str(), r.1), ("from-env", KeySource::Env(CLOUD_KEY_ENV)));
        // saved beats env
        saved.0.insert("cloud".into(), "from-app".into());
        let r = resolve_key(&cfg, &saved, &env_set, Some(&home)).unwrap();
        assert_eq!((r.0.as_str(), r.1), ("from-app", KeySource::Saved));
        // openrouter ignores the credentials file; local ollama never has a key
        let or = AiCfg::from(&settings("openrouter", "", ""));
        assert!(resolve_key(&or, &Keys::default(), &env_none, Some(&home)).is_none());
        let local = AiCfg::from(&settings("ollama", "", ""));
        assert!(resolve_key(&local, &saved, &env_set, Some(&home)).is_none());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn parses_model_lists() {
        let openai = json!({"data":[{"id":"b"},{"id":"a"},{"id":"a"}]});
        assert_eq!(parse_models("cloud", &openai), vec!["a".to_string(), "b".to_string()]);
        let ollama = json!({"models":[{"name":"z"},{"name":"y"}]});
        assert_eq!(parse_models("ollama", &ollama), vec!["y".to_string(), "z".to_string()]);
        assert!(parse_models("openrouter", &json!({})).is_empty());
    }

    #[test]
    fn why_not_names_the_reason_in_a_few_words() {
        let e = |s: u16, m: &str| format!("https://ollama.com/v1/chat/completions answered {s}: {m}");
        assert_eq!(why_not(&e(402, "This model is not in the Free plan. Pro is $20/month")), "not on your plan");
        assert_eq!(why_not(&e(403, "This model is currently being rolled out and is not yet available to you.")), "not available yet");
        assert_eq!(why_not(&e(403, "forbidden")), "not available to you");
        assert_eq!(why_not(&e(400, "this model does not support image input (ref: x)")), "text only");
        assert_eq!(why_not(&e(400, "clef:27b is a decision model and only supports /v1/systemone")), "not a chat model");
        assert_eq!(why_not(&e(401, "bad key")), "key refused");
        assert_eq!(why_not(&e(429, "slow down")), "busy, try again");
        assert_eq!(why_not("Can't reach https://x/v1/chat/completions: timed out"), "no answer");
        assert_eq!(why_not(&e(500, "oops")), "error 500");
    }

    #[test]
    fn find_models_keeps_only_the_models_that_answered() {
        let refused = |s: u16, m: &str| Err(format!("u answered {s}: {m}"));
        let c = assemble_checked(
            5,
            vec![
                ("gemma4:31b".into(), Ok(())),
                ("kimi-k3".into(), refused(402, "not in the Free plan")),
                ("beam".into(), refused(400, "does not support image input")),
                ("minimax-m3.1-flash".into(), Ok(())),
                ("clef:27b".into(), refused(400, "decision model")),
            ],
        );
        assert_eq!(c.ok, ["gemma4:31b", "minimax-m3.1-flash"]);
        assert_eq!(c.left_out[0], ("kimi-k3".to_string(), "not on your plan".to_string()));
        assert_eq!(c.left_out.len(), 3);
        let note = checked_note(&c);
        assert!(note.starts_with("2 of 5 models can read images with your account. Only those are listed."), "{note}");
        assert!(note.contains("kimi-k3 (not on your plan)") && note.contains("beam (text only)"), "{note}");
    }

    #[test]
    fn the_note_says_when_everything_answered_and_caps_the_left_out_list() {
        let all = Checked { ok: vec!["a".into(), "b".into()], left_out: vec![], total: 2 };
        assert_eq!(checked_note(&all), "2 of 2 models can read images with your account. All are listed.");
        let many = Checked {
            ok: vec!["a".into()],
            left_out: (0..7).map(|i| (format!("m{i}"), "not on your plan".to_string())).collect(),
            total: 8,
        };
        let note = checked_note(&many);
        assert!(note.contains("m3 (not on your plan)") && !note.contains("m4 (") && note.ends_with("and 3 more."), "{note}");
    }

    #[test]
    fn nothing_answering_is_an_error_that_names_a_refused_key() {
        let refused = vec![("a".to_string(), "key refused".to_string())];
        assert!(nothing_answered(3, &refused).contains("refused the key"));
        assert!(nothing_answered(3, &[("a".to_string(), "no answer".to_string())]).contains("connection"));
    }

    #[test]
    fn openrouter_modalities_and_local_capabilities_decide_vision_without_pinging() {
        let v = json!({"data": [
            {"id": "a/vision", "architecture": {"input_modalities": ["text", "image"]}},
            {"id": "b/text", "architecture": {"input_modalities": ["text"]}},
            {"id": "c/unknown"},
        ]});
        let infos = parse_model_info("openrouter", &v);
        assert_eq!(infos.iter().map(|m| m.vision).collect::<Vec<_>>(), [Some(true), Some(false), None]);
        // Only OpenRouter's listing carries modalities; other servers' flags stay unknown.
        assert!(parse_model_info("cloud", &v).iter().all(|m| m.vision.is_none()));
        let c = checked_from_flags(3, infos.into_iter().map(|m| (m.id, m.vision)).collect());
        assert_eq!(c.ok, ["a/vision", "c/unknown"]);
        assert_eq!(c.left_out, [("b/text".to_string(), "text only".to_string())]);
        assert_eq!(show_vision(&json!({"capabilities": ["completion", "vision"]})), Some(true));
        assert_eq!(show_vision(&json!({"capabilities": ["completion"]})), Some(false));
        assert_eq!(show_vision(&json!({"details": {}})), None);
    }


    #[test]
    fn think_setting_reaches_the_request() {
        let turns = [Turn::new("user", "hi")];
        let mut cfg = AiCfg { provider: "ollama".into(), base: "http://x".into(), model: "m".into(), think: None };
        assert!(chat_request(&cfg, &turns, 0.2, false, None).unwrap().body.get("think").is_none());
        cfg.think = Some("off".into());
        assert_eq!(chat_request(&cfg, &turns, 0.2, false, None).unwrap().body["think"], json!(false));
        cfg.think = Some("low".into());
        assert_eq!(chat_request(&cfg, &turns, 0.2, false, None).unwrap().body["think"], json!("low"));
        cfg.provider = "custom".into();
        assert_eq!(chat_request(&cfg, &turns, 0.2, false, None).unwrap().body["reasoning_effort"], json!("low"));
        cfg.think = Some("off".into());
        assert_eq!(chat_request(&cfg, &turns, 0.2, false, None).unwrap().body["reasoning_effort"], json!("none"));
    }

    #[test]
    fn tool_turns_and_calls_in_both_wire_shapes() {
        let call = ToolCall { id: "c1".into(), name: "lookup".into(), args: json!({"kind": "macro", "query": "rand"}) };
        let turns = [Turn::new("user", "q"), Turn::calling("", vec![call.clone()]), Turn::tool_result(&call, "doc")];
        let tools = json!([{"type": "function", "function": {"name": "lookup"}}]);
        let mut cfg = AiCfg { provider: "ollama".into(), base: "http://x".into(), model: "m".into(), think: None };
        let req = chat_request(&cfg, &turns, 0.2, false, Some(&tools)).unwrap();
        assert_eq!(req.body["tools"], tools);
        assert_eq!(req.body["messages"][1]["tool_calls"][0]["function"]["arguments"]["query"], "rand");
        assert_eq!((req.body["messages"][2]["role"].as_str(), req.body["messages"][2]["tool_name"].as_str()), (Some("tool"), Some("lookup")));
        let r = chat_reply_full(&req, &json!({"message": {"content": "", "tool_calls": [{"function": {"name": "lookup", "arguments": {"kind": "check", "query": "x"}}}]}})).unwrap();
        assert_eq!((r.calls[0].id.as_str(), r.calls[0].args["kind"].as_str()), ("call_0", Some("check")));
        cfg.provider = "custom".into();
        let req = chat_request(&cfg, &turns, 0.2, false, Some(&tools)).unwrap();
        assert_eq!(req.body["messages"][1]["tool_calls"][0]["function"]["arguments"], json!(call.args.to_string()));
        assert!(req.body["messages"][1]["content"].is_null());
        assert_eq!(req.body["messages"][2]["tool_call_id"], "c1");
        let r = chat_reply_full(&req, &json!({"choices": [{"message": {"content": null, "tool_calls": [{"id": "z", "function": {"name": "lookup", "arguments": "{\"kind\":\"topic\",\"query\":\"syntax\"}"}}]}}]})).unwrap();
        assert_eq!((r.calls[0].id.as_str(), r.calls[0].args["query"].as_str()), ("z", Some("syntax")));
        assert!(chat_reply_full(&req, &json!({"choices": [{"message": {"content": null}}]})).is_err());
    }
    #[test]
    fn the_ping_is_one_tiny_picture_and_a_short_reply() {
        let cloud = AiCfg { provider: "cloud".into(), base: "https://ollama.com/v1/".into(), model: "x".into(), think: None };
        let (url, body) = probe_request(&cloud, "gemma4:31b");
        assert_eq!(url, "https://ollama.com/v1/chat/completions");
        assert_eq!(body["model"], "gemma4:31b");
        assert_eq!(body["max_tokens"], 8);
        assert!(body.pointer("/messages/0/content/1/image_url/url").and_then(Value::as_str).is_some_and(|u| u.starts_with("data:image/png;base64,")));
        let local = AiCfg { provider: "ollama".into(), base: "http://127.0.0.1:11434".into(), model: "x".into(), think: None };
        let (url, body) = probe_request(&local, "m");
        assert_eq!(url, "http://127.0.0.1:11434/api/chat");
        assert_eq!(body["options"]["num_predict"], 8);
        assert_eq!(body.pointer("/messages/0/images").and_then(Value::as_array).map(Vec::len), Some(1));
        assert!(probe_png().starts_with(b"\x89PNG"));
    }
}

/// AI command-line tools on the teacher's own computer, used as the AI: Claude Code (`claude -p`)
/// and Codex (`codex exec`). A teacher with a Claude or ChatGPT subscription needs no API key, and
/// nothing runs on our servers. Each call is non-interactive, with no saved session, in a private
/// scratch folder containing only request attachments and the CLI's reply file.
#[cfg(not(target_arch = "wasm32"))]
pub mod cli {
    use super::{AiCfg, Turn};
    use std::io::Write as _;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// The longest one answer may take (a long question file from a big model).
    const TIMEOUT: Duration = Duration::from_secs(300);

    pub fn is_cli(cfg: &AiCfg) -> bool {
        matches!(cfg.provider.as_str(), "claude-cli" | "codex-cli")
    }

    /// `name` on PATH (with Windows' .cmd/.exe), if installed.
    pub fn find(name: &str) -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat", ""] } else { &[""] };
        std::env::split_paths(&path)
            .flat_map(|d| exts.iter().map(move |e| d.join(format!("{name}{e}"))))
            .find(|p| p.is_file())
            .or_else(|| {
                // Claude Code's own installer puts it in ~/.local/bin, which a desktop app's PATH may lack.
                let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
                let p = PathBuf::from(home).join(".local").join("bin").join(if cfg!(windows) { format!("{name}.exe") } else { name.to_string() });
                p.is_file().then_some(p)
            })
    }

    /// Which AI tools this computer has, as provider names ("claude-cli", "codex-cli").
    pub fn installed() -> Vec<&'static str> {
        [("claude", "claude-cli"), ("codex", "codex-cli")].into_iter().filter(|(bin, _)| find(bin).is_some()).map(|(_, p)| p).collect()
    }

    /// The conversation as one prompt: a CLI run takes a single message.
    pub fn transcript(turns: &[Turn]) -> (String, String) {
        let system = turns.iter().filter(|t| t.role == "system").map(|t| t.text.as_str()).collect::<Vec<_>>().join("\n\n");
        let rest: Vec<&Turn> = turns.iter().filter(|t| t.role != "system").collect();
        if rest.len() == 1 {
            return (system, rest[0].text.clone());
        }
        let mut p = String::from("This is a conversation so far. Reply to the last message, as the assistant.\n");
        for t in rest {
            let who = match t.role { "assistant" => "YOUR EARLIER REPLY", "tool" => "LOOKUP RESULT", _ => "MESSAGE" };
            p.push_str(&format!("\n===== {who} =====\n{}\n", t.text));
        }
        (system, p)
    }

    pub fn chat(cfg: &AiCfg, turns: &[Turn]) -> Result<String, String> {
        let images = turns.iter().flat_map(|t| &t.images);
        if cfg.provider == "claude-cli" && images.clone().next().is_some() {
            return Err("For PDFs or photos, choose Codex or a vision-capable AI in Your AI.".into());
        }
        let (system, prompt) = transcript(turns);
        let dir = tempfile::Builder::new().prefix("dad-ai-").tempdir()
            .map_err(|e| format!("Could not create a temporary AI workspace: {e}"))?;
        match cfg.provider.as_str() {
            "claude-cli" => {
                let bin = find("claude").ok_or("Claude Code isn't installed on this computer.")?;
                let mut cmd = Command::new(bin);
                cmd.args(["-p", "--output-format", "json", "--tools", "", "--no-session-persistence", "--strict-mcp-config", "--disable-slash-commands"]);
                if !system.is_empty() { cmd.args(["--system-prompt", &system]); }
                if !cfg.model.trim().is_empty() { cmd.args(["--model", cfg.model.trim()]); }
                let out = run(cmd, dir.path(), &prompt)?;
                let v: serde_json::Value = serde_json::from_str(out.trim())
                    .map_err(|_| format!("Claude Code gave an unexpected answer: {}", out.chars().take(200).collect::<String>()))?;
                if v["is_error"] == true {
                    return Err(format!("Claude Code: {}", v["result"].as_str().unwrap_or("it failed")));
                }
                v["result"].as_str().map(str::to_string).ok_or_else(|| "Claude Code gave no answer.".into())
            }
            _ => {
                let bin = find("codex").ok_or("Codex isn't installed on this computer.")?;
                let last = dir.path().join("reply.txt");
                let mut cmd = Command::new(bin);
                cmd.args(["exec", "--skip-git-repo-check", "--ephemeral", "-s", "read-only", "--color", "never", "-o"]).arg(&last);
                if !cfg.model.trim().is_empty() { cmd.args(["-m", cfg.model.trim()]); }
                for (index, image) in images.enumerate() {
                    let name = format!("worksheet-{}.png", index + 1);
                    std::fs::write(dir.path().join(&name), image)
                        .map_err(|e| format!("Could not attach the worksheet image: {e}"))?;
                    cmd.args(["--image", &name]);
                }
                cmd.args(["--", "-"]);
                let full = if system.is_empty() { prompt } else { format!("{system}\n\n{prompt}") };
                run(cmd, dir.path(), &full)?;
                std::fs::read_to_string(&last).map_err(|_| "Codex gave no answer.".to_string())
            }
        }
    }

    /// Runs the tool with `input` on stdin; its stdout, or a plain error.
    fn run(mut cmd: Command, dir: &std::path::Path, input: &str) -> Result<String, String> {
        cmd.current_dir(dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("Could not start the AI tool: {e}"))?;
        fn drain(mut stream: impl std::io::Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                let _ = stream.read_to_end(&mut bytes);
                bytes
            })
        }
        let stdout = drain(child.stdout.take().expect("piped stdout"));
        let stderr = drain(child.stderr.take().expect("piped stderr"));
        if let Some(mut stdin) = child.stdin.take() {
            let input = input.to_string();
            std::thread::spawn(move || { let _ = stdin.write_all(input.as_bytes()); });
        }
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? { break status; }
            if start.elapsed() > TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                return Err("The AI on this computer took too long to answer.".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        let stdout = String::from_utf8_lossy(&stdout.join().unwrap_or_default()).into_owned();
        let stderr = stderr.join().unwrap_or_default();
        if !status.success() && stdout.trim().is_empty() {
            let err = String::from_utf8_lossy(&stderr);
            let line = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("it stopped with an error");
            // Checked first: Codex's usage-limit message comes with other stderr lines that mention
            // "auth", and it read as "isn't signed in".
            if err.to_ascii_lowercase().contains("usage limit") {
                return Err(format!("The AI tool has reached its usage limit. ({line})"));
            }
            if err.contains("login") || err.contains("auth") || err.contains("API key") {
                return Err(format!("The AI tool isn't signed in. Open a terminal and run it once to sign in. ({line})"));
            }
            return Err(format!("The AI tool failed: {line}"));
        }
        Ok(stdout)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn one_message_goes_as_is_and_a_conversation_is_laid_out() {
            let (s, p) = transcript(&[Turn::new("system", "S"), Turn::new("user", "Q")]);
            assert_eq!((s.as_str(), p.as_str()), ("S", "Q"));
            let (_, p) = transcript(&[Turn::new("system", "S"), Turn::new("user", "Q"), Turn::new("assistant", "A"), Turn::new("user", "fix it")]);
            assert!(p.contains("===== MESSAGE =====\nQ") && p.contains("===== YOUR EARLIER REPLY =====\nA") && p.trim_end().ends_with("fix it"), "{p}");
        }

        /// Needs Claude Code signed in: `cargo test --lib ai::cli -- --ignored`.
        #[test]
        #[ignore]
        fn claude_code_answers() {
            let cfg = AiCfg { provider: "claude-cli".into(), base: String::new(), model: "haiku".into(), think: None };
            let r = chat(&cfg, &[Turn::new("system", "Reply with only a number."), Turn::new("user", "17*23?")]).unwrap();
            assert!(r.contains("391"), "{r}");
        }
    }
}
