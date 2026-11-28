# D.A.D Laws-of-UX council: what Stages A–D settled (read before changing the Questions flow)

Repo: `/home/shuff57/Documents/GitHub/D.A.D`. Stage A is in commit `3b47865d`, preceded by `fc930136`. Stage B is in commit `648f9c57`. Stage C starts from `648f9c57`.

**Rule for every later stage:** each item below was scored and kept on purpose. Do not undo, move back or re-expand any of them unless the owner says so. If a Stage C fix has to touch one of them, keep the property it protects; that property is in brackets after each item. At the end of every stage, re-capture the Stage A and B scenes and check them for regressions.

## Scores

| Stage | Laws | Start | End (Opus / DeepSeek) |
|---|---|---|---|
| A, perception and layout | Aesthetic-Usability, Proximity, Von Restorff, Fitts | 6.0 | 8.6 / 8.6 (R3). Re-checked on the Stage B tree: Opus 8.8. DeepSeek 8.1 before the Try it fixes listed below. |
| B, decisions and memory | Cognitive Load, Hick, Miller, Serial Position, Tesler | 6.6 / 6.0 | 8.9 / about 8.0, a plateau; the owner stopped here |

## Scorecard (`ux-council` skill)

This file is the ledger for the `ux-council` skill (agent-evo): its Stage A–D sections are Phases A–D. Run 1 was grade-only (no changes) on `dad/engine` at `68f8b4a2`, 2026-11-27. The panel was Claude Opus 5.5 (effort high) and DeepSeek V4.1 Flash (OpenCode, variant high), the same two models as the stages.

| Run | Phase | Opus 5.5 | DeepSeek V4.1 Flash | Mean |
|---|---|---|---|---|
| 2026-11-27 | A, perception and layout | 6.8 | 6.5 | 6.7 |
| 2026-11-27 | B, decisions and memory | 7.1 | 6.3 | 6.7 |
| 2026-11-27 | C, familiar patterns and forgiving input | 6.8 | 7.4 | 7.1 |
| 2026-11-27 | D, time, progress and the end | 7.0 | 7.8 | 7.4 |
| 2026-11-27, run 2 | A after rounds 1 and 2 | R1 7.2, R2 7.2 | R1 7.4, R2 6.7 | R1 7.3, R2 6.95: a plateau; the owner approved and locked it |
| 2026-11-27, run 2 | B after rounds 2 and 3 (round 1 void: a missed scripted tap) | R2 7.2, R3 7.6 | R2 7.7, R3 7.8 | R2 7.45, R3 7.7: three rounds; the owner approved and locked it |
| 2026-11-27, run 2 | C after rounds 1 to 3 | R1 7.3, R2 8.2, R3 7.8 | R1 7.8, R2 7.6, R3 8.0 | R1 7.55, R2 7.9, R3 7.9: a plateau; the owner approved and locked it |
| 2026-11-27, run 2 | D after rounds 1 to 3 | R1 7.6, R2 8.1, R3 7.9 | R1 7.0, R2 7.6, R3 7.0 (two of R3's top gaps were misreads) | R1 7.3, R2 7.85, R3 7.45: three rounds; the owner kept all and locked it |

**Run 2 final scorecard** (2026-11-27, `dad/engine` at `a21351dd`). Same panel, scenes and grade-all mode as run 1, so the two compare directly:

| Phase | Start (run 1) | Opus 5.5 | DeepSeek V4.1 Flash | Mean | Weakest law: top gap |
|---|---|---|---|---|---|
| A, perception and layout | 6.7 | 7.6 | 7.0 | **7.3** | Fitts 6.8. Delete is a small text link about 55 px from Open. Disabled buttons have three different looks. |
| B, decisions and memory | 6.7 | 8.0 | 8.0 | **8.0** | Cognitive Load 7.0. Try it's heading is the raw "Name - Description". The Needs fixing reason "3/2 … reduced fraction" reads as contradictory. |
| C, familiar patterns and forgiving input | 7.1 | 8.6 | 8.3 | **8.4** | Jakob 8.5. Show key is a text line, not an eye in the field. An OpenRouter key still shows Ollama's key help. |
| D, time, progress and the end | 7.4 | 8.6 | 7.9 | **8.2** | Doherty 8.0. The step pills grey out while writing. The bar sits at its first dot for the first stage. |

Phase A is the one below 8 and the place to start next. A small bug the final grade found: "or drop a photo" shows beside the picture button while the AI isn't ready.

**These numbers do not compare with the stage scores above.** The stages graded each stage's own scenes, one stage at a time. This grade covers 29 scenes across the whole app, graded against all four phases together. The scenes include the older screens, a narrow window, no AI set up, pasted input, a write in progress and an error. Use this table as the starting point for the next run.

Void and re-graded: the first C and D scores were voided. One scripted click missed Import (C03), and the seeded store had no remembered assessment name (D04). Both scenes were captured again, and both members graded C and D again on the full set. One gap in DeepSeek's D grade is a misread: it says step 3 has no tick on the finish, but it does.

Found by run 1 (not fixed; carry forward):
- ~~**Postel:** a key shaped for another service (OpenRouter's `sk-or-…`, with a label and spaces) pasted into the Ollama Cloud box gets no warning. Enter then shows "✔ Ollama Cloud accepted the key". Both members ranked this first in C.~~ Fixed 2026-11-27 on `dad/key-check`, merged as `b1d9279b`. A key that plainly belongs to OpenRouter, Anthropic or OpenAI is now named as soon as it is pasted into the Ollama Cloud box. An Ollama Cloud key is tested with a 1-token chat request: Ollama's model list is public and had answered any key. A bad key is now turned down and put back for checking. Marked images: `~/.cache/dad-ux-council/2026-11-27/keyfix/`. **Not yet ported** to verSHion's `merge/dad-into-vershion`; the owner chose to port it later. Not re-graded by the panel.
- **Doherty:** while a question is written, every Open and Delete in My Questions turns grey with no reason given, and the progress card shows one line with no stages.
- **Fitts and Jakob:** the Course level list opens detached, above and to the right of its field, over the page title (B03).
- **Cognitive Load:** Settings, My Questions shows the raw data-folder path and a sentence about "publication receipts".
- **Zeigarnik:** My Questions lists questions in the order they were made, so "Needs fixing" is last and under the fold. The "1 to review · 1 to fix" counts can't be pressed.
- **Layout:** at 860 px wide a lone "My Questions" heading floats above the form, and the Your AI card drops below it (A09). Report is missing from the dock on first open (A01).

Captures, stores and scripts: `~/.cache/dad-ux-council/2026-11-27/`. `scenes.py <outdir> [ids]` recaptures any scene from `target/debug/vershion`, using a scratch HOME and a local fake AI. Each member's lines are in `grade-r0/out/`, and the voided ones are in `grade-r0/void/`.

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

## Phase A, run 2: locked (2026-11-27, `dad/council-a`)

Push-to-8 run, same panel. It stopped at a plateau (6.7, then 7.3, then 6.95) and the owner approved it. Marked images: `~/.cache/dad-ux-council/2026-11-27/phase-a/`. Still open for A: the Figures grid's three right edges, the solid gold active dock item, two different disabled looks (Looks Good's tan fill and Rewrite It's outline), a top fade when a column is scrolled, and Delete as a text link.

- **My Questions status is a coloured dot and the word** (`kit::status_tag`), not a filled chip. The words and colours are the Stage D ones; Needs review uses `theme::on_attention`. [Von Restorff: Open is the action; the status is read]
- **Hover and open states keep the card fill and get a 1.5 px accent edge.** The light theme's `hover_bg` is a green tint (`#ecf2e6`). [Stage A: tan marks disabled controls only]
- **`momui::dropdown` caps its list to the room below the field** (or above, when there's more room there), at most 520 px, and scrolls. [the list stays attached to the field it changes]
- **Narrow window (under 900 px):** "↓ My Questions" sits on the step row beside New Question; the stray label is gone. When the AI isn't ready, the Your AI card comes above the form. [Proximity: the fix is on screen]
- **Your AI card key field:** a key that's turned down and looks like another site's key gets one message under the field, with that cause first. Save Key is the filled button once the box has a key. [one filled button per state; one message]
- **Import:** the paste boxes have `control_edges` and a 12×8 inset, like the Questions description. [inputs look like inputs]
- **Settings, My Questions:** Back Up and Open data folder share one row. Try it's gate says "Try it as a student first, with either button above."
- **The quiet scrollbar** is 3 px, a lighter handle, and 9 px in from the column edge. [the card's edge stays whole]

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

## Phase B, run 2: locked (2026-11-27, `dad/council-b`)

The run went 6.7 → void → 7.45 → 7.7, and the owner approved it. Marked images: `~/.cache/dad-ux-council/2026-11-27/phase-b/`. Still open for B:
- The picture button's reason shows only on hover.
- The rate-limit error offers no retry and no other AI.
- My Questions repeats beside Convert, Import and Share.
- The course list can run past the window with no cue.
- Settings' AI chips don't show which AIs are ready.

- **First open with no AI ready and none chosen** shows "Pick how D.A.D writes your questions" with the plain list, not a key form for an AI the app picked (owner decision 2026-11-27). The recommended choice is the filled button: D.A.D AI where it's offered, otherwise Ollama Cloud, marked "Recommended". Write's gate says "Pick an AI in the Your AI card first." An AI that's ready is still picked on its own (`auto_ai`). [Tesler: the app carries the setup]
- **My Questions order:** to fix, then to review, then ready to add, then in MyOpenMath. Saved order holds within each group. Each row shows MyOpenMath's name before " - " as its title, and the description as a quiet line under it. The question on screen shows "On screen" instead of Open. During a job, a line says why Open and Delete wait. [Serial position; Miller]
- **Your AI card:**
  - An OpenRouter key in the Ollama box gets one line ("This is an OpenRouter key, not an Ollama key."). Its one action is the filled "Use This Key With OpenRouter", which switches the AI and keeps the key; Save Key and Enter are withheld for it.
  - "Get a free key on ollama.com" (or openrouter.ai) comes before the video, in the card and in Settings.
  - While the AI isn't ready, the other AIs are one-press buttons with their needs.
  - "Models and other AIs: Settings, then AI" shows only once the AI works.
  [Tesler; one action]
- **Plain words:**
  - Settings › My Questions: "Your questions and worksheet drafts are kept on this computer…". The path is in Open data folder's hover.
  - Claude's model sits under "Model: … · Change".
  - Convert's card says "PDF, Word, text or photos.", with which AI reads, Word with equations and the local copy under "How D.A.D reads worksheets". Claude's red warning still shows.
  - Help says "Something wrong? Press Report in the dock, on any page, or contact us." Contact us is the filled button.
  [Cognitive load]
- **Smaller changes:**
  - The writing card quotes what was asked (or "Change: …").
  - Share rows show the status dot and a small Download.
  - The course list has "Integrated Math" and "Traditional courses" headings; the order and indexes are unchanged.
  - Disabled Rewrite It says "Pick a change above, or type one below, then press Rewrite It."
  - Step 3 opens with "✔ Approved and saved. Last step: choose where it goes.", with the short name on its own line.
  - The disabled picture button explains itself in its hover text.

## Phase C, run 2: locked (2026-11-27, `dad/council-c`)

The run went 7.1 → 7.55 → 7.9 → 7.9, and the owner approved it. Marked images: `~/.cache/dad-ux-council/2026-11-27/phase-c/`. Still open for C:
- A pasted MyOpenMath link could open the question editor itself (the link has the question ID).
- Figures and Home take drops without saying so.
- The second Import box isn't focused after Ctrl+Enter.
- A misread paste (words or a link) has no "import it as it is".
- Import's dock icon now matches My Questions' storage icon.

Capture tooling: the QA hook (`App::raw_input_hook`) reports the window as focused while `VERSHION_SCRIPT` runs. Under Xvfb the window never has OS focus, and egui's `has_focus()` then ignored every scripted Ctrl+Enter. Teachers aren't affected.

- **A reply pasted from an AI chat (it has ``` fences)** is cleaned in the box to its code, with "Removed the AI chat's own words around the code: this is the code D.A.D will import. · Undo" (`import_original`). If the code has no question text, the hint is "Now paste the question's text" and suggests asking the chat for it. [Postel: what's imported is what's shown]
- **Import:**
  - The box is labelled "Or paste a question".
  - A pasted link gets numbered steps ("…paste it in the box above, in place of the link"), and Open It in Your Browser empties the box.
  - Code without question text gets "Now paste the question's text", naming MyOpenMath's box labelled Question Text.
  [Postel]
- **Shortcuts are shown where they work:**
  - "Ctrl+Enter writes it" right-aligned under the description box.
  - "or Ctrl+Enter" beside each Import into My Questions.
  - "Ctrl+N" beside New Question.
  [Jakob]
- **Your AI card:**
  - Enter on an OpenRouter key presses "Use This Key With OpenRouter", and the card then says "✔ Now using OpenRouter with the key you pasted (its label and spaces removed)."
  - A pasted key that came with a label or spaces shows "D.A.D drops the label and spaces that came with the key when it saves."
  [Postel]
- **Familiar shapes:**
  - The dock's Import uses `hand::DOWNLOAD` (the arrow points into the tray).
  - The first-run notice has a corner × named "Close"; Esc and a click outside also close it.
  - "or drop a photo on this window" sits beside Add a reference picture when that button can be used.
  [Jakob]

## Phase D, run 2: locked (2026-11-27, `dad/council-d`)

The run went 7.4 → 7.3 → 7.85 → 7.45. The owner kept all three rounds. Marked images: `~/.cache/dad-ux-council/2026-11-27/phase-d/`. Still open for D:
- The bar sits at its first dot for the first stage, so it can look stalled.
- There's no Stop away from Questions.
- The dock's "Writing…" has no motion.
- The busy error gives no countdown.
- Show on the being-written row does nothing while its card is on screen.
- The rows' generic Open doesn't say what's left.

- **The writing card** shows a thin bar with "N of 3 stages done" and all three stages from the start: Write it (or Rewrite it), Test it with 3 sets of random numbers, Ready to try. Each is marked ✔ done, current (spinner) or to come (○). A detail line shows only when it says more than the stage name. [Goal-Gradient; Doherty]
- **My Questions:**
  - A question being written heads the list: "Being written · stage · m:ss" with Show.
  - "✔ N of M in MyOpenMath".
  - Needs fixing rows show the first problem the check found.
  - The paused-rows note names the job and Stop.
  [Zeigarnik]
- **A busy AI on step 1** is a calm gate line, not red: "The AI is busy right now. Wait a minute and press Write My Question again, or use a different AI." It has a "Use a Different AI" button that opens the Your AI card, and Show details. There's no second retry button (Write My Question is the retry). `form::plain_error` says "Wait a minute and press it again, or use a different AI." [Peak-End]
- **The finish** quotes the short name and has no New Question in the step row, so Write Another Question is its one next step. Step 3's connect card says "Last added to <place>." in strong text (`Poster::last_place`). [Peak-End; Goal-Gradient]

## Phase A, run 3: locked (2026-11-28, `dad/council3-a`)

Push-to-8.5 run with Opus 5.5 and GPT-6.2 Sol, both at high. Phase A went 7.4 → 7.4 → 7.6, reaching 7.6 from both members, and the owner locked it.

- **One disabled look:** a disabled filled or outlined button uses `pageshell::unavailable_colors`, and so does a paused Delete. [Aesthetic-Usability: a disabled look only on disabled controls]
- **My Questions rows:**
  - The status sits in a 112 px slot, so every Open starts on one edge.
  - A hairline separates the rows.
  - Delete is a painted 32 × 32 trash button named "Delete", matched to Open and well apart from it. It still asks first.
  - During a job the status stays at full strength; only Open and Delete pause.
  [Fitts; Proximity]
- **Scrolling:**
  - Scrolled columns fade into the page at the top.
  - Import and Share use `column()`, with the quiet bar and the clickable More below cue.
  - The quiet bar is 3 px, light and 9 px in.
  - Figures' grid and its cue end on the search row's edge: no 18 px allowance, 8 px held back for card frames, measured.
  [Stage A: one shared right edge]
- **Page frame:**
  - New Question meets the column edge, with Ctrl+N to its left.
  - Show key sits on the key field's label row.
  - Choose question files is filled while nothing is pasted or chosen.
  - Share's Download follows its question.
  - More options is a 32 px row.
  - The step pills keep their look during a job (presses are ignored).
  - "or drop a photo" shows only when the AI is ready.

## Phase B, run 3: locked (2026-11-28, `dad/council3-b`)

Same panel, starting from the clean set. Phase B went 8.05 → 7.9 → 8.35 → 8.1. The owner kept round 3, minus its two changes both members disliked, so the final version wasn't regraded.

- **Write's blocked line names what's missing:**
  - Pick an AI.
  - Paste your Ollama Cloud / OpenRouter key.
  - Sign in to D.A.D.
  - Use your pasted key with OpenRouter.
  [Tesler: a blocked action says why and points at the fix]
- **The Your AI key form starts with where to get a key** (the ollama.com or openrouter.ai link), then a field labelled for that AI ("Paste your Ollama key here"), with Show key on its label row. "needs a key" is in the warning colour. The field has a fixed id (`mom_ai_card_key`), so focus and Enter survive Show key appearing. The OpenRouter switch says it needs a few dollars of credit. [Tesler; Cognitive Load]
- **Plain words:**
  - Try it's heading is the short name, with the description on a quiet line.
  - Try it's instruction is one line.
  - The common check failure reads "The answer is wrong for every set of numbers tried." (`plain_problem`), in quiet text beside the red dot.
  - The My Questions summary runs in row order (to fix, to review, ready).
  [Cognitive Load; Miller]
- **Step 3:** with a remembered place, it says only "Last added to …" and "Connect, and D.A.D picks it again." (it does).
- **Share:** "Download N ready questions" (in MyOpenMath, or approved and passing) is filled. "Download all questions" is quieter and says it includes drafts.
- **Settings:**
  - A line under the AI chips says which are ready here and which need a key or sign-in.
  - Other AIs are folded last.
  - Back Up My Questions is filled, and the card says how to restore.
  - The checkbox has 3:1 edges.
- **Smaller changes:**
  - Convert's Your AI card says "Worksheets are read by Codex on this computer."
  - A disabled picture button says "This AI can't read pictures".
  - The being-written row reads "Writing it · 0:03".
  - The paused-rows note says to press Show, then Stop.

## Phase C, run 3: locked (2026-11-28, `dad/council3-c`)

Phase C went 8.1 → 8.2 → 8.2 → 8.05, a plateau. The owner locked it. Still open, as features:
- Import a question from its MyOpenMath link (the link carries the question ID, and D.A.D signs in to MyOpenMath).
- Import code that has no question text.
- Take SVG/PNG drops on Figures, with a cue.
- A scroll cue inside long dropdowns.

- **Focus is visible and lands where the next keystroke goes:**
  - A focused text field has a 2 px edge (`selection.stroke`); hover is a 1 px edge, so it never reads as focus.
  - After Ctrl+Enter raises the second Import box, the cursor moves into it.
  - Both Import boxes (`mom_import_paste`, `mom_import_second`) and the key box have fixed ids, so focus survives rows coming and going.
  [Jakob]
- **Familiar shapes:**
  - The dock's Share is the paper plane (`hand::PUBLISH`).
  - Figures' search uses `kit::search_field`, with its magnifier.
  - Settings' My Questions has the shelf icon.
  - Home's import tile says "Pick or drop spreadsheets, quiz exports".
  [Jakob]
- **Forgiving input:**
  - A pasted link's first step says to go to the question's editor, or to ask its author for its Common Control and Question Text boxes.
  - An OpenRouter key in the Ollama box is named in the warning colour, with "or press Enter in the key box" and a line for anyone who meant Ollama Cloud.
  - The header says "key not saved yet" once a key is typed.
  [Postel]
- **QA hook:** a scripted run reports window focus to its end.

## Choices already decided (don't reopen as new findings)

- **D.A.D AI is gated** on `dad_ai_offered()`. Without a server it led to a dead-end server form. DeepSeek wanted it as the first chip whenever it's offered; it isn't first yet.
- **"Try with text controls" stays as a quiet link.** DeepSeek wanted it removed from the happy path.
- **Report joins the dock once the first-run notice is closed.** The notice has its own "Report an issue" link (2026-11-27).
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
| 5 | Pause just before MyOpenMath's Save so Stop adding can land (owner decision 2026-11-25) | `dad/stop-adding` | merged 2026-11-25 (`911ff427`) |
| 6 | Say D.A.D, not verSHion, in what teachers read (`src/share/`, server emails) | `dad/rename-dad` | merged 2026-11-25 (`e2197a2e`); legal and contact pages still say verSHion |
| 7 | Older screens: wordmark, the bank importer's two names, "Make an Exam", zero counts, "Shared" | `dad/legacy-names` | merged 2026-11-25 (`9178e8d5`) |
| 8 | Tell a revision of a published question from its original in My Questions | `dad/revision-titles` | merged 2026-11-25 (`36f536f0`) |
| 9 | Slow tier: 4 timing tests that fail only under parallel load | `dad/test-timing` | merged 2026-11-26 (`4769ff12`); the four are timed in instructions (`src/cpu.rs`), full slow tier passed twice (5730 passed) |

After all four merges, `dad/engine` passes `cargo test --lib` (5,490 passed, 0 failed, 288 ignored, about 2 minutes), and the desktop build and the wasm check have no warnings.

### End-to-end on MyOpenMath (`dad/e2e-mom`): what changed and what is open

Run against the hidden "D.A.D test" assessment in Global Question Class, with Claude on this computer as the AI. Screenshots are in `/tmp/dad-e2e/shots/`. Writing, Try it, Publish (question 1992176), the finish, See it in MyOpenMath, the In MyOpenMath chip, Stop adding (nothing saved), reopening after a restart, and a revision saved as a new question (1992209) all worked once these fixes were in:
- `42e2571f`: the finish names the place after a restart, and every place published to is remembered by name.
- `9c8b4cb4`: "Saved as a new question" stays on screen after a revision.
- `61816ab4`: Codex at its usage limit says "usage limit", not "out of credit" or "isn't signed in".
- `5501b5de`: step 2 of a published question says "✔ In MyOpenMath" and offers "See Where It Is", not "Add It to MyOpenMath".
- `385be8e3`: the MyOpenMath panel closes once Connect has read the courses, so the pickers show.

Open:
- **Stop adding is hard to hit.** With a saved login an add takes 1–2 s, so most presses landed after Save. **Owner decision (2026-11-25): pause just before Save so Stop can land.** Built on `dad/stop-adding`: a publish waits `poster::SAVE_HOLD` (2.5 s) after it has found Save, counting down in the panel beside "Stop adding" ("Adding it to MyOpenMath in 2… Press Stop adding to cancel."). A press in that window saves nothing and shows the muted "Stopped before saving. Nothing was added to MyOpenMath." The finish after a publish is unchanged, 2.5 s later. The editor takes 3.6–4.5 s to open and fill first, so a whole publish is about 6–7 s; `SAVE_HOLD` is the one constant to change.
- ~~A revision has the same name as its original, so My Questions shows two identical titles told apart only by their chips.~~ Fixed on `dad/revision-titles`, see below.
- In a worktree the checker files (`engine/browser/dist`) are missing; point `DAD_ENGINE_FILES` at the installed copy.
- ~~Test leftovers: questions 1992176, 1992209 and 1992297 in "D.A.D test", and three test rows in My Questions.~~ Deleted 2026-11-26 at the owner's request: the three questions on MyOpenMath (each ID searched again and gone) and the three rows and their publication records in the local My Questions store.

### Revision titles (`dad/revision-titles`): owner decision (2026-11-25) and what changed

**Owner chose:** the name stays the same; the revision's row gets a muted second line. Not a suffix in the name (it would reach MyOpenMath as the Description) and not a "Revised" chip.

Locked:
- A revision of a published question shows "Revised from the question added on Nov 25" under its name in My Questions, and the same line on step 3's finish. A revision of a published revision reads "Revision 2, from the question added on …", so two revisions made on one day still differ. The year is added only when the date is not this year.
- The name is unchanged, so MyOpenMath's Description is the same for every version; the line never reaches MyOpenMath. The chips, the "Saved as a new question" note and the finish naming its place are as they were. The original has no line.
- `Saved.revised_from` (`Revision { added_at, number }`) holds it; old libraries open unchanged, and a rewrite of a question that was never published adds nothing. The date is the original's newest publish receipt (its saved time if no receipt can be read).

Open: dates are UTC like the rest of the app, so a revision made on a US evening can show the next day's date. Images: `/tmp/dad-revtitles/compare/`.

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

Still waiting for the owner:
- Home, Banks, Exams, Grade, the bank importer and Shared can't be reached from the app; the owner chose to polish them in place without adding a way in.

### Older screens' names and headers (`dad/legacy-names`): owner decisions (2026-11-25), merged `9178e8d5`

The five questions left open above were put to the owner and built (marked images: `~/.cache/dad-legacy-ux/compare/names-*-marked.png`). Locked from here:
- **Wordmark on every page.** `pageshell::header` draws it, and a page's own actions sit right-aligned under it. This makes Stage A's rule true everywhere. Banks, Exams and Settings content sits about 42 px lower.
- **"Import into a question bank" everywhere:** the page, Home's tile and the Banks button. *This replaces Stage C's locked "Import to a bank" on the tile and button.*
- **Grade's empty-state button says "New exam"**, as Home and Exams do. *This replaces Stage A's "Make an Exam" label; the one-primary-button rule is unchanged.*
- **Zero counts are hidden on Banks, Exams and Home.** *This extends Stage A's hide-zeros rule, which named Home only.*
- **The Shared page is titled "Shared by teachers"**; the dock's Share is unchanged.
- Not checked: narrow windows, dark mode, whether the longer Home tile fits at narrow widths, and the Shared page's state with its Share button showing.

### "verSHion" renamed to "D.A.D" (`dad/rename-dad`), merged `e2197a2e`

About 155 teacher-facing strings in the app and in both server mailers (`server/api`, `server/worker`) now say D.A.D, and "the Account window" is "Settings, then D.A.D account". Left alone on purpose: the keychain service name (renaming it would sign teachers out), the `VERSHION_*` variables, the `vershion://` link scheme, the GitHub URLs, the backup file name and comments. Open: `server/web/legal/*.html`, `server/web/contact/index.html` and `server/web/account/credits.html` still say verSHion; the owner has not said whether to edit legal and contact pages. The Worker test `round42.test.ts` could not run in a fresh worktree (it needs `server/share-core/pkg` built).

Left open (small): three Shared states not captured; card layout differences between Banks and Exams; missing footers on Banks, Exams and Home; small copy. Narrow width and dark mode were not checked.

### Server launch (`dad/server-launch`): what changed and what is open

- The server is the Rust server `server/api` on the VPS (`docs/hosting-port.md`), not the Worker. The runbook is `docs/server-launch.md`.
- The server has a `question` service at 1 credit. The app opens a credit session for every write, rewrite, worksheet problem and worksheet file, and turns server refusals into teacher sentences (`ai::account_message`).
- Tested locally with an Ollama model: sign-up, sign-in, writing on credits, running out of credits, an ended sign-in and an unreachable server. Worksheet conversion on D.A.D AI was not tested end to end.
- Open: `AI_RELAY_KEY` must be set, or calls without a sign-in get the AI free. The server's emails still say "verSHion". `VERSHION_LEGAL_URL` is the site root, not `/legal`. The 1-credit price is the builder's choice.
