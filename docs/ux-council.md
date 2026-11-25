# D.A.D Laws-of-UX council: what Stages A–D settled (read before changing the Questions flow)

Repo: `/home/shuff57/Documents/GitHub/D.A.D`. Stage A is in commit `3b47865d`, preceded by `fc930136`. Stage B is in commit `648f9c57`. Stage C starts from `648f9c57`.

**Rule for every later stage:** each item below was scored and kept on purpose. Do not undo, move back or re-expand any of them unless the owner says so. If a Stage C fix has to touch one of them, keep the property it protects; that property is in brackets after each item. At the end of every stage, re-capture the Stage A and B scenes and check them for regressions.

## Scores

| Stage | Laws | Start | End (Opus / DeepSeek) |
|---|---|---|---|
| A, perception and layout | Aesthetic-Usability, Proximity, Von Restorff, Fitts | 6.0 | 8.6 / 8.6 (R3). Re-checked on the Stage B tree: Opus 8.8. DeepSeek 8.1 before the Try it fixes listed below. |
| B, decisions and memory | Cognitive Load, Hick, Miller, Serial Position, Tesler | 6.6 / 6.0 | 8.9 / about 8.0, a plateau; the owner stopped here |

## Stage A: locked layout (`3b47865d`)

- **Questions, step 1.** Order: description, then Course and Topic side by side, then **Write My Question** with the reference-picture button beside it, then a closed "More options" that shows how many options are set. [Primary action under the eye; fewer things to see at once.]
- **AI card.** Sits in the side column, names the AI that will write, and has a "Change" button. [The form comes first.]
- **One filled (primary) button per state.** This covers Try it ("Try It as a Student"), Add to MyOpenMath, worksheet upload ("Choose worksheet files"), Grade's empty state ("Make an Exam") and the first-run notice ("Got It, Let's Start"). Everything else is secondary or a link. [Von Restorff.]
- **New Question** is a separate command at the end of the progress row, not a fourth step.
- **Columns.** All columns share one right edge. `kit::quiet_scroll_area` gives each a scrollbar that shows on hover, plus a clickable "More below" cue. Figures has a full-width search row and a "More figures below" cue.
- **Inputs.** Text fields use the card fill; the tan fill marks disabled controls only. `control_edges(ui)` keeps control borders at 3:1 contrast, and `square_checks(ui)` draws square checkboxes. [Inputs look like inputs.]
- **Report lives in the dock at every width.** No floating pill.
- **Header and dock state.** Each page header is the title on the left and the wordmark right-aligned to the content edge. Import and Share light up their dock items on the older pages. Home hides counts that are zero.
- Grade hides its floating action button until an exam exists.

## Stage B: locked decisions (`648f9c57`)

- **Dock order:** Questions, Convert, Import, Share, Figures, Settings, with Report also in the dock. [Serial position: the main jobs come first.]
- **AI choice in plain words.** `plain_ais()` in `momui.rs` is the one list, used by both the card and Settings:
  - Claude on this computer
  - Codex on this computer
  - Ollama Cloud
  - OpenRouter
  - D.A.D AI, shown only when `dad_ai_offered()`, which means signed in or a school server built in.

  No model IDs appear in the main UI.
- **The app picks an AI on first open** (`auto_ai`). It keeps the current AI if it's ready; otherwise it tries the D.A.D account, then the Claude command-line tool, then Codex, then the built-in server.
- **When the AI isn't ready,** the card says why ("needs a key" or "needs you to sign in") and fixes it in place: a key field with Save Key and a video, or "Sign In to D.A.D". Write is blocked with a one-line reason. Other AIs are folded under "Use a different AI instead". [Tesler: the app carries the setup.]
- **Course level** defaults to none ("Let the AI choose"). Topic appears once a course is chosen. The saved writing rule moved into More options, where it's called "Save a rule for every question". The setup-video field before writing was removed.
- **Answer types:** the 8 common types (`Answer::COMMON`) come first and the rest sit under "More answer types". MyOpenMath type codes and the comparison table show only in advanced mode. The advanced switch is in Settings → My Questions.
- **Try it:**
  - Approve is one press once the preview has been seen.
  - "Try with text controls" is a link, with a text fallback when the preview fails.
  - The "Not quite right?" card has 3 change buttons and **Rewrite It** on one row, 4 more under "More changes", and a 2-line text box.
  - A failed check hides the revise card.
- **Step 3:** Publish comes first; "Preview in MyOpenMath first" is a link. The manual fallback, "Can't connect? Add it by hand or with the bookmark", is closed and puts the bookmark first.
- **Worksheet conversion** sends Claude-on-this-computer to Codex when Codex is available; otherwise it warns before the upload.
- **New Question** saves the current question first.
- **Import:** restores a full Library backup and links to Convert for worksheets.
- **Share:** downloads only.
- **Settings:** sections are AI, MyOpenMath login (open by default), My Questions (storage, backup, advanced), Help & community, and D.A.D account (optional).
  - The AI section uses the same plain list. The technical AIs (Ollama, workers, custom, and the school server when it isn't offered) sit under "Other AIs (for IT or your own server)".
  - The model sits under "Model: … · Change", and the key sources under "Where D.A.D looks for the key".
  - A found key shows "✔ Key found" plus a Test button.
  - The "Account optional" note appears on the account section only.
- **Words:** "D.A.D AI", "Sign In to D.A.D", and "D.A.D account". Error text points to "Settings, then D.A.D account".

## Choices already decided (don't reopen as new findings)

- **D.A.D AI is gated** on `dad_ai_offered()`. Without a server it led to a dead-end server form. DeepSeek wanted it as the first chip whenever it's offered; it isn't first yet.
- **"Try with text controls" stays as a quiet link.** DeepSeek wanted it removed from the happy path.
- **Settings opens on AI,** the thing most teachers come for.

## Still open, not done (carry forward; may resurface)

- **Readiness probe (size M).** "Ready" means "has a key", not "it answered". This was DeepSeek's main Stage B gap.
- **Owner product decision:** host a default D.A.D server so D.A.D AI works for everyone.
- Figures opens on physics cards.
- The legacy Imports and Shared screens are still reachable from Home and banks. They duplicate the dock's Import and Convert.
- Help & community has 5 buttons. DeepSeek wanted 3.
- Header layouts differ between the new screens and the old ones. Home and Grade don't light a dock item. Home's three cards have unequal widths. The Shared card is about 658 px wide; this is legacy and pre-dates Stage A.

## Builder notes

- **Captures:** `/tmp/dad-ux-council/cap.sh <bin> <out>` (scratch tooling, not in the repo) covers scenes 01–06 and the 10-* screens. `capB.sh` adds scenes 20–25. Seeded stores: `fresh`, `seen` and `chosen`. The QA hook `VERSHION_SCRIPT` takes `click:x,y;text:abc;key:Enter;wait`. The window is 1180×780 under xvfb at a 1600×1000 screen. The build with a school server built in is `/tmp/dad-ux-council/vershion-pool`.
- **Before and after:** `annotateB.py` makes side-by-side images with numbered boxes and a red/blue pixel wash. The owner wants every comparison marked this way.
- **Honesty rules:**
  - The builder never grades.
  - The panel stays the same every round: Opus on high plus DeepSeek v4.1 flash.
  - Run tests before taking screenshots, and make one build per round.
  - Changes of direction go to the owner.
  - Stop on a plateau.

## Stage C: familiar patterns and forgiving input (Jakob, Postel), committed in c6ea351c

Scores: C0 7.2 / 6.0–6.5, then C3 8.9 / 8.5 (Opus / DeepSeek). The build after round 3 (`cfinal`) also has the round-3 fixes, which have not been scored.

Locked from Stage C:
- **Keys:**
  - `ai::clean_key` drops labels, quotes, line breaks and full stops.
  - `key_shape_problem` warns about a key that belongs to another site.
  - A saved key is tried at once (OpenRouter is checked through `/key`).
  - A rejected key is taken off and put back in the field, masked, with Show key. This works in the AI card and in Settings.
- **Import reads what was pasted** (`not_a_question`, `unwrap_reply`, `split_boxes`):
  - **Words:** "Write a Question From It" writes the question.
  - **A link:** says what to copy instead, and offers Open It in Your Browser.
  - **Half a question:** a second box asks for the other MyOpenMath box, in either order.
  - **Code from an AI chat:** the ``` fences are dropped, and two blocks are joined when they are the two boxes.
  - **Prices and $…$ math stay words.** Code is recognised by its shape: `$x =`, `rand(`, or a lowercase macro.
- **Keyboard:**
  - Ctrl/Cmd+Enter sends in the description, the change box and both Import boxes.
  - Enter saves a key or a login.
  - Ctrl/Cmd+N starts a new question; it saves the current one first and clears the description.
  - Esc clears an Import hint.
- **Drops:**
  - Screens that take drops: Questions, Home, Settings, Share, Account and Welcome. The bank Import window and the resolve dialog keep theirs.
  - A full-window "Drop to add" cue shows while dragging, and "or drop files on this window" sits beside the file buttons.
  - A photo dropped on step 1 becomes the reference picture, unless the AI can't read pictures.
  - Each type D.A.D can't read gets its own fix, such as HEIC, .doc or a spreadsheet.
- **Names and icons:**
  - The picker is called "Question type", and MyOpenMath's name is shown where it differs.
  - Part counts use dropdowns.
  - Settings uses the settings icon. In Settings, the MyOpenMath login has a key icon and the account a person icon.
- **The older bank importer** is "Import into a question bank". Home's tile says "Import to a bank", and the page links to the MyOpenMath Import page.

Left open after Stage C:
- HEIC files in the file pickers.
- Whether the dock's Import should light up on the older bank page. That's the owner's call.
- The flaky test `share::bank_links_fix11_tests::c83_i2`, which fails now and then in the parallel run and is unrelated to Stage C.

## Stage D: time, progress and the end (Doherty, Goal-Gradient, Zeigarnik, Peak-End), committed in f2c7cf29

Scores: D0 7.0 / 7.5, then D3 8.8 / 8.6 (Opus / DeepSeek). The build after round 3 (`dfinal`) also has the round-3 fixes, which have not been scored.

Locked from Stage D:
- **Writing:**
  - A written question is saved to My Questions at once.
  - Retrying a description replaces a row that failed its check; a row that passed is never replaced.
  - Writing and rewriting share one progress card: stages, a time range, the time so far after a minute, and Stop. Stop drops the answer by job generation and keeps a rewrite's change in its box.
  - Reading a worksheet has Stop. Converting has Pause and "About a minute per problem".
- **Dock while away:** the Questions item says Writing…, Rewriting…, Adding…, "Ready ✔" or "Needs a look"; worksheet jobs show on the Convert item. Presses made during a job are never replayed afterwards. Convert waits only during a write, a rewrite or an add.
- **Progress counted as done:**
  - A ✔ joins the number on finished steps, and step 3 gets one once the question is published.
  - "N of M worksheet problems converted" and "page N of M".
  - "✔ N in MyOpenMath", then "N ready to add · N to review · N to fix".
  - Chips: "In MyOpenMath" (read from the publish receipt), "Ready to add", "Needs review" and "Needs fixing".
- **The end:**
  - A published question on step 3 shows only the finish: "✔ It's in MyOpenMath", the place by name (remembered from the last publish), MyOpenMath's question ID on a small line, one primary button, "See it in MyOpenMath" in the signed-in panel, and "Add it to a different assessment".
  - The primary is Write Another Question, Next Worksheet Draft, or Back to the Worksheet.
- **Publishing:**
  - "Stop adding" really stops just before MyOpenMath's Save (`poster::STOP_BEFORE_SAVE`), and the result is shown muted, not red.
  - "Save unconfirmed" has an "Open the assessment" link.
  - A revision of a published question is saved as a new question.
- **Errors:** messages already written for teachers pass through as they are; raw technical text and 429/5xx errors become plain sentences. A failed rewrite says why at the top of Try it.
- **Worksheets:** "I'm done with this worksheet" closes a batch out. Convert reopens only while a batch still has work left, including approved drafts not yet published.
- **Tests:** the test build saves to a scratch folder (`store::default_backend` under `cfg(test)`), never the real store.

## Owner decisions after Stage D (2026-11-24)

- **Default server: yes.** The owner hosts the D.A.D server and sets the repository Actions variables `VERSHION_POOL_URL` and `VERSHION_LEGAL_URL`; release CI reads them (`d23644d5`). D.A.D AI is listed first wherever it is offered.
- **Older bank page:** it keeps lighting the dock's Import item (Stage A lock kept).
- **iPhone photos:** the pickers offer HEIC files and say to share them as a JPEG (`d23644d5`).

## Follow-up work

Each piece runs in its own thread and branch. The coordinating thread merges finished branches into `dad/engine` locally; nothing is pushed without the owner.

| # | Work | Branch | Status |
|---|---|---|---|
| 1 | End-to-end test on real MyOpenMath (write, try, publish, the finish, Stop adding, the In MyOpenMath label) | `dad/e2e-mom` | merged 2026-11-25 (`653b1d5b`); one follow-up, see below |
| 2 | Server launch prep: deploy steps for `server/`, release build with the URL, D.A.D AI sign-in and credits (every account or paid step waits for the owner) | `dad/server-launch` | merged 2026-11-25 (`9311ad48`); owner steps remain, see below |
| 3 | Older screens' UX (headers on Home, Grade and Shared; duplicate import and share screens; Help's five buttons; Figures opening on physics) | `dad/legacy-ux` | merged 2026-11-25 (`7ba4e770`), Opus 8.6 / DeepSeek 8.5 |
| 4 | Housekeeping: full test suite including figures, the flaky Figures drag test, compiler warnings | `dad/housekeeping` | merged 2026-11-25 (`3b83c711`) |
| 5 | Pause just before MyOpenMath's Save so Stop adding can land (owner decision 2026-11-25) | `dad/stop-adding` | started 2026-11-25 |
| 6 | Say D.A.D, not verSHion, in what teachers read (`src/share/`, server emails) | `dad/rename-dad` | started 2026-11-25 |
| 7 | Older screens: wordmark, the bank importer's two names, "Make an Exam", zero counts, "Shared" (the thread asks the owner each) | `dad/legacy-names` | started 2026-11-25 |
| 8 | Tell a revision of a published question from its original in My Questions (the thread asks the owner how) | `dad/revision-titles` | started 2026-11-25 |
| 9 | Slow tier: 4 timing tests that fail only under parallel load | `dad/test-timing` | started 2026-11-25 |

After all four merges, `dad/engine` passes `cargo test --lib` (5,490 passed, 0 failed, 288 ignored, about 2 minutes), and the desktop build and the wasm check have no warnings.

### End-to-end on MyOpenMath (`dad/e2e-mom`): what changed and what is open

Run against the hidden "D.A.D test" assessment in Global Question Class, with Claude on this computer as the AI. Screenshots are in `/tmp/dad-e2e/shots/`. Writing, Try it, Publish (question 1992176), the finish, See it in MyOpenMath, the In MyOpenMath chip, Stop adding (nothing saved), reopening after a restart, and a revision saved as a new question (1992209) all worked once these fixes were in:
- `42e2571f`: the finish names the place after a restart, and every place published to is remembered by name.
- `9c8b4cb4`: "Saved as a new question" stays on screen after a revision.
- `61816ab4`: Codex at its usage limit says "usage limit", not "out of credit" or "isn't signed in".
- `5501b5de`: step 2 of a published question says "✔ In MyOpenMath" and offers "See Where It Is", not "Add It to MyOpenMath".
- `385be8e3`: the MyOpenMath panel closes once Connect has read the courses, so the pickers show.

Open:
- **Stop adding is hard to hit.** With a saved login an add takes 1–2 s, so most presses land after Save. **Owner decision (2026-11-25): pause just before Save so Stop can land.** Not built yet.
- A revision has the same name as its original, so My Questions shows two identical titles told apart only by their chips.
- In a worktree the checker files (`engine/browser/dist`) are missing; point `DAD_ENGINE_FILES` at the installed copy.
- Test leftovers for the owner to delete: questions 1992176 and 1992209 in "D.A.D test", and three test rows in My Questions.

### Housekeeping (`dad/housekeeping`): what changed and what is open

- A plain `cargo test --lib` takes about 1–2 minutes, down from 77. The 239 tests that took over 15 s are behind the test-only `slow-tests` feature, and none were deleted. Run everything with `cargo test --profile figtest --lib --features slow-tests` (about 29 minutes).
- Fixed flaky tests: the Figures label drag (the preview redrew during the drag) and `c83_i2` (matched ids by their first letter, which collided with the shared id counter).
- Open: 4 timing tests in the slow tier fail only under full parallel load (they pass alone). Measuring with thread CPU time would fix them. `conics::general` ignores its `dec` argument in a fallback path no caller reaches.

### Older screens (`dad/legacy-ux`): what changed and what is open

Scores: L0 7.2 / 7.0, L1 8.3 / 8.4, L2 8.6 / 8.5 (Opus / DeepSeek). Reports and marked images are in `~/.cache/dad-legacy-ux/`.

Now settled (keep):
- Figures opens on math: "Math and computing" is the first tab and leads the All view.
- Home, Grade, Banks, Exams and the bank importer have no breadcrumbs. Home's counts are one subtitle line with zeros left out, its three tiles are equal, and its filled tile comes first in both states.
- Help & community has three buttons: Contact us, Known issues, Buy me a coffee. Report stays in the dock.
- Shared fills the page width, with a "What sharing does" panel and a full-width legal strip.
- The bank importer lists Canvas, Blackboard, Moodle, Kahoot, Quizlet and "Other quiz exports", and its button is "Convert with AI". Grade hides its checking keys until an exam exists.

Waiting for the owner:
- Home, Banks, Exams, Grade, the bank importer and Shared can't be reached from the app; the owner chose to polish them in place without adding a way in.
- Only Questions carries the wordmark that Stage A says every header has.
- The bank importer has two names ("Import into a question bank" and "Import to a bank"), both locked in Stage C. Grade says "Make an Exam" (Stage A); Home and Exams say "New exam".
- Zero counts on Banks and Exams ("1 bank · 0 questions"), and whether the Shared page should be renamed "Shared by teachers" so it isn't confused with the dock's Share.

Left open (small): about 100 "verSHion" strings in `src/share/`; three Shared states not captured; card layout differences between Banks and Exams; missing footers on Banks, Exams and Home; small copy. Narrow width and dark mode were not checked.

### Server launch (`dad/server-launch`): what changed and what is open

- The server is the Rust server `server/api` on the VPS (`docs/hosting-port.md`), not the Worker. The runbook is `docs/server-launch.md`.
- The server has a `question` service at 1 credit. The app opens a credit session for every write, rewrite, worksheet problem and worksheet file, and turns server refusals into teacher sentences (`ai::account_message`).
- Tested locally with an Ollama model: sign-up, sign-in, writing on credits, running out of credits, an ended sign-in and an unreachable server. Worksheet conversion on D.A.D AI was not tested end to end.
- Open: `AI_RELAY_KEY` must be set, or calls without a sign-in get the AI free. The server's emails still say "verSHion". `VERSHION_LEGAL_URL` is the site root, not `/legal`. The 1-credit price is the builder's choice.
