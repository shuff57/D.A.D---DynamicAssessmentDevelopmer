//! Tests only: what a render costs, measured so that the rest of the suite running alongside cannot fail a time limit.
//!
//! Wall time is no good: a busy machine stretches it. The thread's CPU time is better but not enough either: with every
//! core busy (and two threads to a core) the same render took 3.4 times the CPU time it takes alone, because the cores
//! run at a lower clock, share their execution units and fight over the cache. So where the system can count them (Linux,
//! through `perf_event_open`), a render is measured by the instructions its thread retired in user space, which a busy
//! neighbour does not change (the same render: 3,369,114,878 instructions alone and under load, within 0.01%), and the
//! count is turned into seconds at [`INSTR_PER_SEC`], what an idle core manages on this code. Everywhere else it is the
//! thread's CPU time (unix: `CLOCK_THREAD_CPUTIME_ID`) and, where there is no thread clock (Windows), wall time, which
//! is what the limits were before.
//!
//! The renders under test are single-threaded, so the calling thread's count is the render's.

use std::time::Duration;

/// Instructions of the figure code that an idle core retires in a second: 1.8e10, the middle of what the figure renders
/// measured alone on the machine the limits were set on (a Ryzen AI 9 HX 370: 1.4e10 to 2.1e10, that is 3 to 4.4
/// instructions a cycle at 4.5 to 4.8 GHz). The limits in the tests are in these seconds, so a render that took 0.3 s
/// alone is 0.3 s here whatever else is running; a machine with another speed only changes what one of them is.
pub(crate) const INSTR_PER_SEC: f64 = 1.8e10;

/// A point on this thread's meter: [`now`] takes one, [`since`] says what has been used after it.
#[derive(Clone, Copy)]
pub(crate) struct Mark {
    cpu: Duration,
    instr: Option<u64>,
}

/// This thread's meter reading now.
pub(crate) fn now() -> Mark {
    Mark { cpu: cpu_time(), instr: instructions() }
}

/// What this thread has used since `start` (a value of [`now`], taken on this same thread), in seconds of an idle core.
pub(crate) fn since(start: Mark) -> Duration {
    let end = now();
    match (start.instr, end.instr) {
        (Some(a), Some(b)) => Duration::from_secs_f64(b.saturating_sub(a) as f64 / INSTR_PER_SEC),
        _ => end.cpu.saturating_sub(start.cpu),
    }
}

/// What running a closure costs this thread, with its result.
pub(crate) fn time<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let t = now();
    let v = f();
    (v, since(t))
}

/// Runs `f` on a new thread and waits for it, giving up once the thread has used `limit` of CPU time (polled on its own
/// CPU clock, not the wall clock; a thread that is only waiting for a core on a busy machine is not charged): `None` if
/// it is still running then (a loop that never ends; the thread is left behind), else its result and what it cost.
pub(crate) fn run_within<T: Send + 'static>(limit: Duration, f: impl FnOnce() -> T + Send + 'static) -> Option<(T, Duration)> {
    let (tx, rx) = std::sync::mpsc::channel();
    let wall = std::time::Instant::now();
    let handle = std::thread::spawn(move || {
        let _ = tx.send(time(f));
    });
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(done) => return Some(done),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return None, // (it panicked)
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if cpu_time_of(&handle).unwrap_or_else(|| wall.elapsed()) >= limit { return rx.try_recv().ok(); }
            }
        }
    }
}

/// The calling thread's CPU time so far (wall time since the first call, where the system has no thread clock).
fn cpu_time() -> Duration {
    #[cfg(unix)]
    {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: `ts` is a valid out-pointer for the call.
        if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) } == 0 {
            return Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32);
        }
    }
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed()
}

/// Another thread's CPU time so far, where the system can say (Linux: its pthread CPU clock).
fn cpu_time_of<T>(handle: &std::thread::JoinHandle<T>) -> Option<Duration> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use std::os::unix::thread::JoinHandleExt;
        let mut clock: libc::clockid_t = 0;
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: the pthread is alive (the handle has not been joined) and both out-pointers are valid.
        let ok = unsafe { libc::pthread_getcpuclockid(handle.as_pthread_t(), &mut clock) == 0 && libc::clock_gettime(clock, &mut ts) == 0 };
        if ok { return Some(Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)); }
    }
    let _ = handle;
    None
}

/// Instructions this thread has retired in user space so far, where the system counts them (Linux, if `perf_event_open`
/// is allowed: `perf_event_paranoid` of 2 or less); `None` otherwise.
fn instructions() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        use std::os::raw::c_void;
        /// `struct perf_event_attr` up to its `config1` (PERF_ATTR_SIZE_VER0, which every kernel accepts).
        #[repr(C)]
        struct Attr { kind: u32, size: u32, config: u64, sample_period: u64, sample_type: u64, read_format: u64, flags: u64, wakeup: u32, bp_type: u32, config1: u64 }
        /// The counter's file descriptor for this thread, opened on first use and closed when the thread ends.
        struct Counter(Option<i32>);
        impl Drop for Counter {
            fn drop(&mut self) { if let Some(fd) = self.0 { unsafe { libc::close(fd); } } }
        }
        thread_local! { static COUNTER: Counter = Counter(open()); }
        fn open() -> Option<i32> {
            const HARDWARE: u32 = 0;
            const INSTRUCTIONS: u64 = 1;
            const EXCLUDE_KERNEL: u64 = 1 << 5;
            const EXCLUDE_HV: u64 = 1 << 6;
            const FD_CLOEXEC: libc::c_ulong = 8;
            let attr = Attr { kind: HARDWARE, size: std::mem::size_of::<Attr>() as u32, config: INSTRUCTIONS, sample_period: 0, sample_type: 0, read_format: 0, flags: EXCLUDE_KERNEL | EXCLUDE_HV, wakeup: 0, bp_type: 0, config1: 0 };
            // SAFETY: `attr` is a valid, fully initialised attribute block of the size it states; pid 0, cpu -1 counts this thread on any cpu.
            let fd = unsafe { libc::syscall(libc::SYS_perf_event_open, &attr as *const Attr, 0 as libc::pid_t, -1 as libc::c_int, -1 as libc::c_int, FD_CLOEXEC) };
            (fd >= 0).then_some(fd as i32)
        }
        let read = |c: &Counter| {
            let fd = c.0?;
            let mut count = 0u64;
            // SAFETY: reads the 8-byte counter value into `count`.
            let n = unsafe { libc::read(fd, &mut count as *mut u64 as *mut c_void, 8) };
            (n == 8).then_some(count)
        };
        return COUNTER.with(read);
    }
    #[allow(unreachable_code)]
    None
}

/// Shows that the instruction count does not move with the load while the CPU time does (run with `--nocapture` to see
/// both); a plain run only checks the count is steady, where the system counts at all.
#[cfg(test)]
mod tests {
    use super::*;

    /// Some work with the shape of a render: allocation, floating point, string building, a lookup table.
    fn work() -> usize {
        let mut table = std::collections::HashMap::new();
        let mut text = String::new();
        let mut x = 0.5f64;
        for i in 0..400_000usize {
            x = (x * 1.000001 + (i as f64).sqrt()).sin().abs() + 0.5;
            if i % 97 == 0 { table.insert(i, format!("{x:.6}")); text.push_str(&table[&i]); }
        }
        text.len() + table.len()
    }

    #[test]
    fn a_render_costs_the_same_in_instructions_alone_and_loaded() {
        let measure = || { let (c, i) = (cpu_time(), instructions()); work(); (cpu_time() - c, instructions().zip(i).map(|(b, a)| b - a)) };
        let alone: Vec<_> = (0..5).map(|_| measure()).collect();
        // all the cores busy, two threads each
        let busy = std::sync::atomic::AtomicBool::new(true);
        let loaded = std::thread::scope(|s| {
            for _ in 0..std::thread::available_parallelism().map_or(8, |n| n.get()) * 2 {
                s.spawn(|| { while busy.load(std::sync::atomic::Ordering::Relaxed) { std::hint::black_box(work()); } });
            }
            let l: Vec<_> = (0..5).map(|_| measure()).collect();
            busy.store(false, std::sync::atomic::Ordering::Relaxed);
            l
        });
        for (name, runs) in [("alone", &alone), ("loaded", &loaded)] {
            for (cpu, instr) in runs.iter() { eprintln!("{name}: {:.1} ms of CPU, {instr:?} instructions, {:?} per CPU second", cpu.as_secs_f64() * 1000., instr.map(|i| i as f64 / cpu.as_secs_f64())); }
        }
        let counts: Vec<u64> = alone.iter().chain(&loaded).filter_map(|r| r.1).collect();
        if let (Some(lo), Some(hi)) = (counts.iter().min(), counts.iter().max()) {
            assert!(*hi as f64 <= *lo as f64 * 1.05, "instruction counts moved with the load: {lo} to {hi}");
        }
    }
}
