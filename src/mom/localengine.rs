//! The question checker on the teacher's own computer, without Docker: IMathAS (the same files the
//! web app runs in the browser, `engine/browser/dist`) on a static PHP build from static-php-cli,
//! served on a free port on 127.0.0.1 for as long as the app runs.
//!
//! PHP is downloaded once per platform from dl.static-php.dev and checked against a pinned SHA-256
//! (the site publishes none, so these were taken when the version was pinned). The checker files come
//! from `DAD_ENGINE_FILES`, an `engine/` folder next to the app, or this repo's build output, and are
//! installed once per version into the app's data folder. Measured 2026-11-21: the whole bank (1,239
//! questions) gives the same verdicts as the Docker engine, in 37 s.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

/// The engine address that means "the checker this app runs itself".
pub const LOCAL: &str = "local";

/// (platform, download path under static-php-cli/, sha256).
const PHP_BUILDS: &[(&str, &str, &str)] = &[
    ("linux-x86_64", "common/php-8.2.32-cli-linux-x86_64.tar.gz", "2e609bb88bae88661dbd447e409531f7c36931adff480d8a0a74e812b889d1be"),
    ("linux-aarch64", "common/php-8.2.32-cli-linux-aarch64.tar.gz", "2c646a48523015daa0aff703876cab6c66d3fdd8d9012a348ceff78569558913"),
    ("macos-x86_64", "common/php-8.2.32-cli-macos-x86_64.tar.gz", "44013a26832908738a674848faa313282985fcd9f4efc20841d97bb9e2bc318c"),
    ("macos-aarch64", "common/php-8.2.32-cli-macos-aarch64.tar.gz", "074e50b2b039a5f2c0830dd30494d714a10128f0d8eaf0e5e0c0dbe70c33d7f7"),
    ("windows-x86_64", "windows/spc-max/php-8.2.30-cli-win.zip", "004f0fec7c7e92135f88a37d04236433b3627ef2339218cbfe503f15a83b9be1"),
];

/// The running checker and its address.
static RUNNING: Mutex<Option<(Child, String)>> = Mutex::new(None);

fn platform() -> String {
    let os = if cfg!(target_os = "macos") { "macos" } else if cfg!(windows) { "windows" } else { "linux" };
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
    format!("{os}-{arch}")
}

fn base() -> PathBuf { crate::store::store_dir().join("mom") }

/// The checker's address, starting it (and fetching PHP the first time) if needed. Blocking;
/// call from a background thread. `log` gets plain progress lines.
pub fn ensure(log: &mut dyn FnMut(&str)) -> Result<String, String> {
    let mut running = RUNNING.lock().map_err(|_| "lock")?;
    if let Some((child, url)) = running.as_mut() {
        if child.try_wait().ok().flatten().is_none() && healthy(url) {
            return Ok(url.clone());
        }
    }
    let php = php_binary(log)?;
    let root = engine_files(log)?;
    // A checker left by an app that crashed or was killed: stop it before starting ours.
    let pid_file = base().join("checker.pid");
    if let Some(pid) = std::fs::read_to_string(&pid_file).ok().and_then(|s| s.trim().parse::<u32>().ok()) {
        kill_tree(pid);
        let _ = std::fs::remove_file(&pid_file);
    }
    let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map_err(|e| e.to_string())?.port();
    let url = format!("http://127.0.0.1:{port}");
    log("Starting the question checker…");
    let mut cmd = Command::new(&php);
    cmd.current_dir(&root)
        .args(["-c", "php.ini", "-S", &format!("127.0.0.1:{port}"), "serve.php"])
        .env("PHP_CLI_SERVER_WORKERS", "4")
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // Its own process group: PHP's built-in server forks worker processes, and stopping the group
    // stops them all (stopping only the first one leaves the workers serving).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(|e| format!("Could not start the question checker: {e}"))?;
    let _ = std::fs::write(&pid_file, child.id().to_string());
    let start = Instant::now();
    while !healthy(&url) {
        if let Ok(Some(s)) = child.try_wait() {
            return Err(format!("The question checker stopped right away ({s})."));
        }
        if start.elapsed() > Duration::from_secs(20) {
            kill_tree(child.id());
            let _ = child.wait();
            return Err("The question checker didn't start in time.".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    *running = Some((child, url.clone()));
    Ok(url)
}

/// Stops the checker and its workers (the app calls this when it closes).
pub fn stop() {
    if let Ok(mut r) = RUNNING.lock() {
        if let Some((mut child, _)) = r.take() {
            kill_tree(child.id());
            let _ = child.wait();
            let _ = std::fs::remove_file(base().join("checker.pid"));
        }
    }
}

/// Stops a checker and the workers it forked.
fn kill_tree(pid: u32) {
    #[cfg(unix)]
    unsafe {
        // The checker leads its own process group (see `ensure`), so -pid is all of it.
        libc::kill(-(pid as i32), libc::SIGTERM);
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()])
            .stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
}

fn healthy(url: &str) -> bool {
    ureq::get(&format!("{url}/health")).call().ok()
        .and_then(|mut r| r.body_mut().read_to_string().ok()).is_some_and(|b| b.trim() == "ok")
}

/// PHP for this platform, downloaded and verified once.
fn php_binary(log: &mut dyn FnMut(&str)) -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("DAD_PHP") { return Ok(PathBuf::from(p)); }
    let plat = platform();
    let (_, path, sha) = PHP_BUILDS.iter().find(|b| b.0 == plat)
        .ok_or_else(|| format!("The question checker has no PHP build for {plat} yet."))?;
    let dir = base().join("php").join(&sha[..12]);
    let exe = dir.join(if cfg!(windows) { "php.exe" } else { "php" });
    if exe.is_file() { return Ok(exe); }
    log("Downloading the question checker (about 12 MB, once)…");
    let url = format!("https://dl.static-php.dev/static-php-cli/{path}");
    let mut bytes = Vec::new();
    ureq::get(&url).call().map_err(|e| format!("Could not download the question checker: {e}"))?
        .body_mut().with_config().limit(64 * 1024 * 1024).reader().read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    let got = hex(&Sha256::digest(&bytes));
    if got != *sha {
        return Err("The downloaded question checker didn't match its expected fingerprint, so it wasn't used.".into());
    }
    let binary = if path.ends_with(".zip") { from_zip(&bytes, "php.exe")? } else { from_tar_gz(&bytes, "php")? };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = dir.join("php.download");
    std::fs::write(&tmp, &binary).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, &exe).map_err(|e| e.to_string())?;
    Ok(exe)
}

fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

fn from_zip(bytes: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut f = z.by_name(name).map_err(|_| format!("{name} is missing from the download"))?;
    let mut out = Vec::new();
    f.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// One file out of a .tar.gz (ustar: 512-byte headers, size in octal at 124).
fn from_tar_gz(bytes: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let mut tar = Vec::new();
    flate2::read::GzDecoder::new(bytes).read_to_end(&mut tar).map_err(|e| e.to_string())?;
    let mut at = 0;
    while at + 512 <= tar.len() {
        let h = &tar[at..at + 512];
        if h.iter().all(|&b| b == 0) { break; }
        let field = |a: usize, b: usize| String::from_utf8_lossy(&h[a..b]).trim_end_matches('\0').trim().to_string();
        let fname = field(0, 100);
        let size = usize::from_str_radix(&field(124, 136), 8).map_err(|_| "a damaged download")?;
        let data = at + 512;
        if fname.trim_start_matches("./") == name {
            return tar.get(data..data + size).map(<[u8]>::to_vec).ok_or_else(|| "a damaged download".into());
        }
        at = data + size.div_ceil(512) * 512;
    }
    Err(format!("{name} is missing from the download"))
}

/// The checker files, installed once per version: IMathAS's PHP (imathas-engine.zip) and the
/// student page's scripts and styles (imathas/).
fn engine_files(log: &mut dyn FnMut(&str)) -> Result<PathBuf, String> {
    let src = source().ok_or("The question checker's files weren't found. Reinstall D.A.D, or set DAD_ENGINE_FILES.")?;
    let zip = std::fs::read(src.join("imathas-engine.zip")).map_err(|e| format!("Could not read the checker files: {e}"))?;
    let dir = base().join("engine").join(&hex(&Sha256::digest(&zip))[..12]);
    if dir.join("serve.php").is_file() { return Ok(dir); }
    log("Setting up the question checker (once)…");
    let tmp = dir.with_extension("partial");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    zip::ZipArchive::new(std::io::Cursor::new(&zip)).and_then(|mut z| z.extract(&tmp)).map_err(|e| format!("Could not unpack the checker: {e}"))?;
    copy_tree(&src.join("imathas"), &tmp).map_err(|e| format!("Could not copy the checker's page files: {e}"))?;
    // The Docker engine's limits (engine/php.ini), without its log path (Docker's stderr).
    let ini: String = include_str!("../../engine/php.ini").lines().filter(|l| !l.starts_with("error_log") && !l.starts_with("log_errors")).map(|l| format!("{l}\n")).collect();
    std::fs::write(tmp.join("php.ini"), ini + "log_errors = Off\n").map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&tmp, &dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Where the checker files ship: DAD_ENGINE_FILES, `engine/` beside the app, or this repo's build.
fn source() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("DAD_ENGINE_FILES").map(PathBuf::from),
        std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("engine"))),
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("engine/browser/dist")),
    ];
    candidates.into_iter().flatten().find(|d| d.join("imathas-engine.zip").is_file())
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_tree(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_platform_has_a_pinned_build() {
        for p in ["linux-x86_64", "linux-aarch64", "macos-x86_64", "macos-aarch64", "windows-x86_64"] {
            assert!(PHP_BUILDS.iter().any(|b| b.0 == p && b.2.len() == 64), "{p}");
        }
    }

    #[test]
    fn reads_one_file_from_a_tar_gz() {
        let mut tar = vec![0u8; 512];
        tar[..3].copy_from_slice(b"php");
        tar[124..135].copy_from_slice(b"00000000005");
        tar.extend_from_slice(b"hello");
        tar.resize(1024 + 1024, 0);
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, &tar).unwrap();
        assert_eq!(from_tar_gz(&gz.finish().unwrap(), "php").unwrap(), b"hello");
    }

    /// Downloads PHP (once) and starts the checker: `cargo test --lib mom::localengine -- --ignored`.
    #[test]
    #[ignore]
    fn starts_and_self_tests() {
        let url = ensure(&mut |l| eprintln!("{l}")).unwrap();
        let src = "// === SET QUESTION TYPE TO: number ===\n// === COMMON CONTROL ===\n$a = rand(2,9)\n$answer = $a + 1\n// === QUESTION TEXT ===\nWhat is `$a+1`? $answerbox\n";
        let text = ureq::post(&format!("{url}/api/selftest")).header("Content-Type", "application/json")
            .send(serde_json::json!({"src": src}).to_string()).unwrap().body_mut().read_to_string().unwrap();
        let r: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(r["pass"], true, "{r}");
        stop();
    }
}
