# D.A.D — Dynamic Assessment Developer

A teacher types a question they want answered or created. An AI writes the MyOpenMath (IMathAS)
question code, a private copy of the IMathAS engine renders and **scores** it, the AI repairs it
until it passes, and the finished question goes into the teacher's MyOpenMath. That last step is
either copy and paste, or a browser the AI drives and the teacher can take over at any moment.

Web first, because school computers are locked down. The same app also ships as a desktop build.

## Where it comes from

D.A.D is a fork of **verSHion** (local `main` at `5ea07129`, 2026-11-20). verSHion already has the
Rust + egui + WebAssembly app, question banks, a question editor, AI with the teacher's own key
(OpenRouter, Ollama Cloud, local Ollama), a blind AI answer check, the figure agent and the house design.
Remotes: `upstream` is GitHub verSHion, which lags; `vershion-local` is `~/Documents/GitHub/verSHion`,
which holds the newest `main`. Merge verSHion changes from `vershion-local`.

Pieces brought in from other repos:

| From | What | Becomes |
|---|---|---|
| steve-desktop `src/integrations/mom/` (TS) | MOM file format, health checks, revise loop, answer key | `mom-core` crate (Rust, builds for WebAssembly) |
| steve-desktop `mom-content/` | 417 questions that render clean | regression corpus for the engine and the AI loop |
| VPS `mom-render` (`serve.php`) | stateless IMathAS render | `engine/`: render + score + self-test |
| OGRE `ogre-harness::cdp` (Rust) | Chrome over CDP, pipe transport | the browser host (desktop and server) |
| OGRE `ogre-agent` | AI does a page task one approved step at a time | the MOM posting agent |
| OGRE `passwords` / `keys` | keyring storage | desktop credentials |
| T3 Code `apps/server/src/preview/ServerBrowser.ts` and `apps/web/src/browser/ServerBrowserSurface.tsx` | streamed browser with take/release control | design reference only (MIT, TypeScript); re-implemented in Rust |

Accounts and credits: **bring your own key only** for now. verSHion's credit ledger stays in the
fork but is switched off.

## The loop

```
teacher prompt
  -> AI drafts question (MOM format: control / qtext / answer, qtype)
  -> engine /selftest: for seeds s1..sN, render, take the question's own correct answers,
     score them (expect full marks), score blanks and wrong answers (expect 0), collect errors
  -> mom-core health: undefined variables, leftover markers, display-only checks
  -> if anything failed: feed the failures back to the AI and retry (bounded)
  -> MOM-style interactive preview (teacher can answer it and see it scored)
  -> post to MyOpenMath
```

## Browser control and handoff (modelled on T3 Code)

How T3 does it (read from source, `t3code` 2026-11-03):

- A real Chromium runs where the server runs (Playwright). Frames come from CDP
  `Page.startScreencast` as JPEG, acked at about 100 ms so viewers get up to about 30 fps, and motion
  frames drop to quality 50.
- The client paints frames on a `<canvas>`. Pointer events on the canvas, and a hidden `<textarea>`
  for typing and IME, are forwarded as `Input.dispatchMouseEvent`, `Input.dispatchKeyEvent` and
  `Input.insertText`.
- **One controller at a time**: `you`, `agent`, `another-viewer` or none. A strip above the page
  says who is driving and has one button, **Take control** or **Release control**. While the agent
  drives, its cursor is drawn over the page so you can see where it is about to click.
- Per-user persistent browser profiles, so a login survives.

D.A.D does the same in Rust:

- **Browser host** (Rust, tokio): starts Chromium with `--remote-debugging-pipe` (OGRE's
  `ogre-harness::cdp`), runs the screencast, applies input, and owns the control lease. It speaks one
  WebSocket protocol: frames out; input, `takeControl` and `releaseControl` in; status events out.
- **Browser panel** (egui, native and wasm): draws the latest frame as a texture, forwards egui
  pointer and key events, shows the controller strip, the agent cursor and an action log
  ("Agent: filling Common Control…").
- **Agent** (from `ogre-agent`): only acts while it holds the lease. When the teacher presses
  Take control, the agent's next step waits. When the teacher releases control, the agent re-reads
  the page and continues. It pauses and asks to hand over at sign-in, at a Duo prompt, and before
  any Save.
- **Where the browser runs:**
  - Web build: on our server. The teacher **signs in to MyOpenMath themselves inside the streamed
    browser** (works with Google, Clever and Duo). We keep their browser profile (cookies), encrypted
    at rest, not their password. A stored username and password are optional, for MOM-native accounts only.
  - Desktop build: a local Chrome over the same protocol, so the experience is the same.

**Capacity:** the VPS has 2 cores and 3.7 GB (1.2 GB free), so about one or two Chromium sessions.
Real use needs the NAS or a bigger box. Idle sessions are closed after a few minutes and the profile
is kept.

## Posting options (all four, in this order)

1. Copy buttons, one per MOM editor field.
2. Bookmarklet that fills MOM's question editor in the teacher's own tab.
3. Streamed server browser with handoff (above).
4. Desktop build: the same panel on a local Chrome.

## Build order

1. **Engine** (`engine/`): IMathAS pinned, `/render`, `/score`, `/selftest`; limits; the 417-question
   regression. Local Docker first; replace the VPS container only after sign-off.
2. **`mom-core`**: format parse and emit, health checks, ported from steve-desktop with its tests.
3. **Authoring loop** using verSHion's AI layer (BYOK).
4. **MyOpenMath screen**: chat on the left, interactive preview on the right (DOM iframe placed over
   the canvas, the way `helpweb.rs` places its overlay), copy buttons.
5. **Bookmarklet**.
6. **Browser host + panel** with take/release control, then the posting agent.
7. **Desktop** local-Chrome mode.

## Security notes

- The engine runs teacher- and AI-written question code. It runs with no network, a read-only
  filesystem, CPU, memory and time caps, and only behind our API. The current VPS endpoint is public
  with `Access-Control-Allow-Origin: *`; that stops once the API exists.
- Server browser profiles belong to one teacher each, are encrypted at rest and never shared. The
  agent stays on an allowlist of hosts (myopenmath.com).

## Status (2026-11-20)

- Step 1 done: `engine/`. 1,171 of 1,239 bank questions pass; `docs/dad/bank-triage.md` lists the rest.
- Step 2 done: `src/mom/` (format, health, engine types). Every bank file round-trips.
- Step 3 working headless: `src/mom/author.rs` and `DAD_AUTHOR="<request>"` on the desktop binary.
  First live runs with local Ollama (deepseek-v4.1-flash:cloud): `example-slope.php` passed on
  try 1 (20 s); `example-mean-median.php` passed on try 4 (84 s) after the engine sent back 6
  problems. Tries 2 and 3 were lost to invalid JSON: escaping a long PHP block inside JSON is
  error-prone for the model. Next: have the model reply in the mom file format itself (no JSON
  escaping), and do not count an unreadable reply as a full try.
- The loop is blocking for now (D.A.D's `main` has only `chat_turns`); the web build needs the
  async chat from verSHion's newer branch.
