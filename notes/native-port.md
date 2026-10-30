# Native Android port (GridCraft 0.1.0, debug x86_64)

## What was done

- `apps/gridcraft-android`: new cdylib crate `gridcraft_android`. `android_main` runs eframe with
  `android-game-activity` and wgpu and hosts `gridcraft_ui_egui::SheetApp` (a new workbook, as
  `apps/gridcraft/src/main.rs` does). Desktop-only pieces (control server, native menu, Apple
  events, printing, preferences, desktop theme) are left out. On desktop the crate is an empty
  library; `cargo clippy -p gridcraft-android` is clean.
- `packaging/android-native/`: copied from the WordCraft fork's Gradle project, without build
  outputs, `.gradle`, `.kotlin`, `jniLibs` and the WordCraft icon source. Renamed to
  `ai.storyteller.gridcraft.native` (applicationId), `ai.storyteller.gridcraft.nativeapp` (Kotlin
  package), label "Custom GridCraft", library `gridcraft_android`, version 0.1.0 (code 1).
  `build-native.sh` builds `-p gridcraft-android`.
- Launcher icon: GridCraft's upstream placeholder `assets/app-icon/gridcraft-1024.png` at 2/3 size
  on a GridCraft-green background (`#1f9d55`). Rows in `ATTRIBUTION.md`.
- Intent filters and the Open picker list spreadsheet types (xlsx, xlsm, xls, xlsb, ods, csv, txt).
- `.gitignore`: build output, `.gradle`, `.kotlin`, `local.properties` and `jniLibs` are excluded.

## Adaptations

- Soft keyboard: the WordCraft `ImeView` is kept. Committed text becomes `egui::Event::Text`;
  composing text is replaced as it changes (backspace the old, type the new); Delete and key codes
  (Enter, Backspace, forward Delete, Escape) become key presses. The editing context Kotlin reads
  back is always empty, so the keyboard sees no text around the caret.
- Documents: the Open action asks Kotlin for the system picker, reads the cache copy and hands
  the bytes to GridCraft's own `Services::inbox`, the same path the web build uses.
- Save As: `Services::download` writes the file to the app's private folder and reports the path on
  the message box. The system save dialog is not wired yet.
- Clipboard: copies go to the Android clipboard through the existing `clip.rs` queue; paste reads
  the last copy or the clip Kotlin reports.
- Touch insets and night mode are passed through as in WordCraft.

## What works

Emulator (wc35, x86_64 debug): GridCraft launches and shows the grid (`~/.cache/gc-spike/01-launch.png`). Tapping A1 and typing `123` then Enter put 123 in A1 and moved the selection to A2 (`~/.cache/gc-spike/02-typed.png`). No panic or crash in logcat.

## What does not work / TODO

- Save to the device's documents: needs a SAF `CREATE_DOCUMENT` flow (WordCraft's `files.rs` save
  ticket, not ported). Until then Save As goes to the app's private folder.
- IME context: Kotlin's context is empty, so composition across word boundaries and autocorrect
  that needs text before the caret do not work. Rework `ime::context` if this matters.
- Read Aloud and printing: left out. `nativeTakeSpeech` returns nothing and the TTS engine in
  `MainActivity.kt` stays idle. Remove the dead Kotlin code (speech, print, save tickets) in a
  cleanup pass.
- Recovery copies on background (`nativeFlushRecovery`) are a no-op; no recovery store yet.
- Back and close: they always leave the app, with no unsaved-changes prompt.
- Open-with and Share intents deliver the file bytes through the same picker path, but only the
  `Open` request is exercised so far.
- The system IME may also send text through GameActivity; not checked on the emulator yet.

## Update 1: phone UI

A narrow window (shorter side under 600 pt, the WordCraft break) gets a touch layout in place of the
ribbon, title bar and status bar. Code: `crates/ui-egui/src/phone.rs`; hooks in `lib.rs`
(`SheetApp::ui`), `tabs.rs` and `formula_bar.rs` (bars at least 44 pt on the phone only), `grid.rs`
(a press taken by an open sheet is not a grid gesture) and `ribbon.rs` (`theme_colors` made
`pub(crate)`). Desktop layout is unchanged.

- App bar (64 pt): File (opens Backstage at Home), the workbook name with the unsaved dot, Undo,
  Redo, Save. Undo and Redo grey out as the commands report.
- Floating tab bar at the bottom: Home, Insert, Formulas, Data, View. It is a bottom panel, so the
  grid stops above it and nothing is drawn under it.
- Each tab opens a bottom sheet of 48 pt rows, each running a command id from `ribbon.rs`:
  Home (Bold, Italic, Underline, Fill and Font colour palettes, Align, Number Format dialog,
  Cut/Copy/Paste, Insert and Delete Row and Column), Insert (Chart, Picture, Function dialog,
  Comment), Formulas (AutoSum, Insert Function), Data (Sort A to Z, Sort Z to A, Filter), View
  (Freeze Panes, Gridlines, Zoom In, Zoom Out).
- A press outside an open sheet closes it and does not reach the grid (`phone::take_pointer`).
  Escape closes it (`phone::back_step`). `SheetApp::can_go_back` and `phone::back_step` are there
  for the Android host to close the sheet on Back; the host side is not wired (see below).
- Bold and the other font commands run the same command the ribbon runs. On the emulator the
  bold change does not show in the cell: the build's cell fonts have no bold face, so the renderer
  falls back to regular (a font issue, shared with desktop). Center alignment does show.

### Open

- Android Back does not close the sheet yet. The `nativeCanGoBack` stub in
  `apps/gridcraft-android/src/android.rs` still returns false, and that crate was off limits in this
  pass (another agent is editing it). Wire it to `SheetApp::can_go_back` and `back_step`.
- No keyboard handling on the phone: the tab bar and sheets stay up while the keyboard is open.
- Search, Share, Comments, AutoSave and the theme switch from the desktop title bar are not on the
  phone.
- `header_autofit::table_filter_autofit_works_through_command_and_header_double_click` fails
  intermittently on the base commit too (pre-existing, not from the phone change).

## Update 2: system save dialog

- Save As (`Services::download`) now queues a SAF create-document request (`ACTION_CREATE_DOCUMENT`,
  `.xlsx` MIME, name from the workbook title). The bytes are staged in a cache file in the app folder
  and copied to the chosen URI in `MainActivity.kt`. Cancel does nothing; failures go to the message
  box. Nothing is written to private storage any more.
- Save on a workbook opened from the picker (or shared in) writes back to its content URI. A picked
  `.xlsx` gets its URI as the document `path`, so `file.save` finds it. Only `.xlsx` keeps a
  location: CSV, ODS and other formats open as copies and Save asks where to put the workbook.
  A successful Save As of an `.xlsx` also adopts the new URI, so the next Save writes there.
- `ui-egui`: new optional `Services::save_in_place(path, bytes) -> bool`, checked first in
  `run_command` for `file.save` without a path. Desktop and web leave it unset, so they are unchanged.
- A successful save (Save As or write-back) marks the revision it saved as clean, so the title
  drops "Edited". The revision is the one current when the save was asked for: edits made after
  that leave the workbook dirty. Failures and cancels change nothing. No shared-crate change for this.
- Removed the WordCraft leftovers: Read Aloud (TTS, `nativeTakeSpeech`/`nativeSpeechEvent`), print
  (`print` requests, `printPdf`), the Recent `read` request and the picture picker type.
  `grep -ri wordcraft apps/gridcraft-android packaging/android-native` is empty.
- Failures: Save, write-back and open errors show on the message box. The JNI export
  `nativeSaveDone` was declared in Kotlin but missing in Rust; it now exists.
- Autosave stays off on Android: it would call the engine save with a content URI, which fails.

Not tested on a device: the emulator was not used (another agent had it). Checked: clippy clean
for `gridcraft-android`, the host tests for `files` and `clip`, and the x86_64 debug APK build.
Also checked: `cargo xtask wasm` passes (own target dir). Still to do on a device: Save As to a new
file, Save back to a picked file (title loses "Edited"), Cancel in the dialog, a read-only provider,
and a non-xlsx open followed by Save.

## Update 3: Back, fonts, save check

- Android Back: `nativeCanGoBack` returns `ime::CAN_GO_BACK`, set each frame from `SheetApp::can_go_back`.
  Kotlin sends Escape when it is true, so Back closes the open sheet or palette first and otherwise
  leaves as before. Checked on the emulator: the Home sheet closed on Back and the app stayed focused.
- Floating tab bar: the launch screen shows no black strip behind it.
- Save As: File, then Save As, then Download (.xlsx) opens the system save dialog (`Book1.xlsx`). Tapping
  Save writes the file, shows "Saved Book1.xlsx", and the title loses the unsaved dot. No panic in logcat.
- Bold: NOT fixed. Root cause found in part: `candidates()` in `theme.rs` listed only desktop font
  files (Carlito, Liberation, DejaVu, Calibri, Arial), so on Android the cell-bold role had no file and
  fell back to the regular face. Android now uses `/system/fonts/DroidSans(-Bold).ttf`, and
  `system_font_dirs` includes `/system/fonts`. On the device the bold chain is `sys-cell-bold`, so the
  font is registered, yet A1 still renders the same pixels after Bold. `home.bold` with empty params
  returns Ok, so the style change happens somewhere else. Next: check the cell style after Bold (a log in
  `apply_style` or a unit test of `home.bold` on a SheetApp selection), and check whether the grid
  caches its galleys across the style change.
- Typing into the emulator is lossy (a `Hello` entered as `HI`, and later `Hllo`). Recheck the IME path
  before trusting typed text in screenshots.
- `header_autofit::table_filter_autofit_works_through_command_and_header_double_click` fails
  intermittently (pre-existing).

## Update 4: bold root cause, typing fix (device check pending)

- Bold, root cause: the emulator (Android 16 image) has no bold Droid file. `/system/fonts/DroidSans.ttf`
  and `DroidSans-Bold.ttf` are both symlinks to `Roboto-Regular.ttf`, which is a variable font. The
  phone's `home.bold` call is correct (unit test `phone_bold_sets_the_selected_cell_bold` passes), so the
  style is set and the cell-bold role was bound, but its file drew regular weight. Fix in
  `theme.rs::font_definitions_with`: on Android, bold roles set the variation coordinate `wght=700`
  (static bold files have no `wght` axis and ignore it).
- Typing, root cause (likely): `replace_composing` erased the whole previous composition with one
  Backspace per letter and retyped it, so each keystroke sent a burst of Backspace presses. A run of
  `input text Hello` gave "Hllo" in A1 once. Fix in `app.rs::replace_composing`: erase and retype only
  after the common prefix.
- Bold verified on the device (`~/.cache/gc-spike/v3-bold.png`): A1 bold, A2 regular, after the
  Home sheet's Bold.
- Typing: still lossy on the device (letters dropped, e.g. "Thursday" came out "Thursda"/"hurday").
  Not fixed yet; the composing diff did not help. Checked earlier runs may have been contaminated by
  another app in the foreground; retested cleanly after that.
- Earlier note, superseded: the emulator dropped off adb (`adb devices` empty) during this pass,
  and it was not started again. Still to check: `v3-bold.png` (bold A1 next to regular A2), and typing
  "Hello" several times for lost letters. Italic on Android still draws regular (no italic Droid file).

## Update 5: typing letters lost after Enter (fix pending device check)

- Logged run (temporary logs, removed): the first key after Enter (T) reached Kotlin through the
  IME connection's `sendKeyEvent`, which sent only its key code. Rust drops key codes other than
  Enter, Delete and Escape, so the letter vanished. Other keys reached Rust as `Text`.
- Fix: `Connection.sendKeyEvent` in `MainActivity.kt` sends a printable key's character as a commit.
- Verified on the device (fresh boot, `~/.cache/gc-spike/v3-typing.png`): A1 "Thursday" by `input text`,
  A2 "thursday" by keyevents T H U R S D A Y. No letter lost. The lowercase t is from the unshifted
  keycode 48 (a hardware key without Shift), not a loss. Logs: `~/.cache/gc-spike/typing-dbg2.log`.
- Earlier note, superseded: the emulator detached again before the retest. Check: type `Thursday`
  by `input text`, then T H U R S D A Y by keyevent in a fresh cell, and expect both to be complete.

## Update 6: bottom pill + sheet list

The phone drops the sheet tab row (`tabs::sheet_tabs` is not drawn on the phone). The bottom bar is
now one rounded pill (`phone.rs`):

- Sheet button at the left (48 pt tall, 96 pt wide): the sheet icon and the active sheet's name, cut
  with an ellipsis. Tapping it opens the Sheets sheet.
- A thin divider, then the command tabs (Home, Insert, Formulas, Data, View) as icons with short
  labels. If the five do not fit at 52 pt each (narrow screens), View moves into a More (⋯) tab that
  opens a small sheet listing it.
- Sheets sheet (title "Sheets", Close): one 48 pt row per visible sheet, the active one in the accent
  with a check. Tap a row to switch to it and close. The ⋯ button (48 pt) opens the row actions:
  Rename (the `renameSheet` dialog, now taking a `sheet` index), Duplicate (`sheet.move` copy after
  it), Delete (`home.deleteSheet`, disabled when there is one sheet). "New sheet" is the last row.
- Back and tap-outside close the innermost part first: row actions, then the palette, then the sheet.
- Desktop layout is unchanged.

Checked offscreen (`snapshot --size 412x915 --scale 2.625 --phone-sheet ...`), not on the emulator:
pill with three sheets, the Sheets sheet, and the row actions for a sheet. The test
`phone_sheets::tapping_a_sheet_in_the_sheets_sheet_makes_it_active` taps the sheet button and a row.

Not done: no confirmation before Delete (desktop has none either); no emulator run; no keyboard
handling on the phone yet (see Update 1).

Checked on the emulator (x86_64 debug build, emulator-5554, 2026-10-29), no fixes needed:
- Launch: one pill with the Sheet1 button and Home/Insert/Formulas/Data/View; no sheet-tab row (pill-dev-1).
- Sheet button opens the Sheets list with Sheet1 checked and "New sheet" (pill-dev-2).
- New sheet: a grid opens and the pill shows "Sheet2" (pill-dev-3).
- Tapping the Sheet1 row switches back; the pill shows "Sheet1" (pill-dev-4).
- ⋯ on Sheet1 opens Rename/Duplicate/Delete (pill-dev-5); Rename opens the "Rename Sheet" dialog (pill-dev-5b).
- Cancel closes the dialog and also closes the Sheets list, returning to the grid. The list does not
  stay open under the dialog (pill-dev-5c).
- Back with the Sheets list open closes it; the app stays focused (pill-dev-6).
- `logcat -d` has no panic or FATAL lines.

## Update 7: touch cell editing

Before (x86_64 debug build, emulator-5554, 2026-10-29, `edit-before-*.png`):
- Single tap on a cell selects it; no keyboard (`edit-before-1-single-tap.png`).
- Formula bar tap opens the keyboard; the pill stays drawn above it (`edit-before-2-formulabar-tap.png`).
- Two quick taps on a cell did not start an edit in two tries (`edit-before-3-double-tap.png`,
  `edit-before-5-double-tap-b3.png`). Cause not found; the grid's existing 450 ms double-click rule is unchanged.
- Typing starts an edit and shows the keyboard; the edited cell stays visible, the pill is still drawn
  above the keyboard (`edit-before-4-typed.png`).

After (`edit-after-*.png`):
- Single tap selects, no keyboard (`edit-after-1-single-tap.png`).
- Double tap starts the edit: the keyboard opens, the pill is hidden, and a 56 pt edit bar sits directly
  above the keyboard with a red ✗ at the left and a green ✓ at the right, both 48 pt
  (`edit-after-2-double-tap.png`).
- Typing "123" into B3 shows in the cell (`edit-after-3-typed.png`); ✓ commits, moves to B4, closes the
  keyboard and brings the pill back (`edit-after-4-confirmed.png`).
- Row 30 (B30): the grid scrolls so the cell sits above the edit bar and keyboard (`edit-after-5-row30.png`).
- ✗ on B30 cancels: the keyboard closes, the pill returns, B30 stays empty (`edit-after-6-cancel.png`).
- Back: the first press only hides the keyboard. Android handles Back for the soft keyboard before the
  activity sees it, so the edit stays open with the edit bar (`edit-after-8-back-cancelled.png` is the
  second press). The second Back reaches Rust as Escape and cancels the edit, closing the keyboard and
  bringing the pill back (`edit-after-8b-back-second.png`). Open: make the first Back cancel too; it
  needs an IME-aware change in MainActivity.kt, which another agent is editing.

Code: `phone.rs` (`edit_bar`, `edit_button`, `track_edit`); `lib.rs` `can_go_back` also counts an open
edit. Tests: `tests/it/phone_edit.rs` (✗ cancels without committing; ✓ commits and moves down).

## Update 8: recovery after Android closes the app

- Kotlin already calls `nativeFlushRecovery` from `onPause`/`onStop` (and on Close). Rust now sets a flag
  there (`recovery::request_flush`) and wakes the UI. The next frame (`Shell::logic`) writes a copy of
  every dirty document.
- Where: `<app files>/recovery/doc-N.xlsx` (the workbook, written with `io::save_bytes`) and
  `doc-N.txt` (display name, then the content location if the document has one). Temporary names start
  with a dot; each file is written then renamed, so a failed write keeps the old copy.
- When deleted or rewritten: `sync_recovery` makes the folder match the dirty documents. It runs on
  background, and when a document that had a copy is saved (now clean) or closed (gone).
  No dirty documents means no copies.
- Launch: `Shell::new` reads the copies, reopens each one as a dirty document (`DocState::new` with
  `Trust::FromFile`, the copy's content made dirty), keeps the original content URI when it is an `.xlsx`
  location (Save writes back there), and shows "Recovered unsaved changes" on the status line. Without
  copies, a blank book is made as before.
- Reused: `io::save_bytes`, `io::open_bytes`, `DocState::new`, `Session::add_document`, `files::parse_location`
  and `files::keeps_location`. The engine has no recovery store for GridCraft (WordCraft's `recovery`
  module is not in this codebase), so this is a small one in the Android crate. No engine or UI-crate change.
- Ceilings: one copy per document, rewritten in full (no incremental store). No timed refresh while
  editing: a process killed in the foreground (rare) loses edits since the last background or save.
  Like WordCraft's flush, the sync runs on the next frame after onPause; confirmed on the emulator (x86_64).
- Tests: `recovery::tests` (round trip, empty set, shrinking set, unrelated files kept, newline tidy,
  name parsing). `cargo clippy -p gridcraft-android --all-targets` clean. `build-native.sh x86_64` builds.
- Device check (x86_64 debug build, emulator-5554, 2026-10-29, `rec-*.png`):
  - Type 42 into A1 and commit with Enter: the title shows the unsaved dot (`rec-1.png`).
  - HOME, then `run-as ai.storyteller.gridcraft.native ls files/recovery` lists `doc-0.txt` and `doc-0.xlsx`;
    `am kill` leaves no process (`pidof` empty).
  - Relaunch: A1 = 42, the unsaved dot is back, and the dialog "Recovered unsaved changes" shows (`rec-2.png`).
  - Save (app bar) then the system picker in Downloads: the title becomes `Book1 (1).xlsx` with no dot, and the
    status dialog says "Saved Book1 (1).xlsx" (`rec-3.png`). The picker added " (1)" because Downloads already
    had `Book1.xlsx`. `run-as ... ls files/recovery` is then empty.
- Found while testing: the recovery dialog's OK is not in the UI-automator dump (app window only), so it
  was tapped by position. The system picker's SAVE was found in the dump and tapped by its bounds.

## Update 9: selection handles and long-press actions (device checked 2026-10-30)

Code: `phone.rs` (`touch_gestures`, `action_bar`, `handle_range`), called from `grid::show` after the
grid is painted and before its own input (`grid.rs`). `grid::scroll_by` is now `pub(crate)` for the
handles' edge scroll.

- Handles: when the phone has a selection, a 12 pt dot at the top-left and bottom-right corners, each
  with a 44 pt touch area. Dragging one moves that corner and snaps to cells; the opposite corner stays
  (`handle_range`). Near a cell edge the grid auto-scrolls. Dragging elsewhere still pans the grid.
- Long press: a press held 500 ms without moving on a cell selects that cell (a press inside the current
  area keeps it) and opens the action bar. The bar sits above the selection, or below it when there is no
  room. Its buttons, in a horizontal scroll: Cut, Copy, Paste, Clear, Fill Down, Insert… and Delete…
  (`edit.clearContents`, `edit.fillDown`, and the `insertCells` and `deleteCells` dialogs, which the
  ribbon uses). A tap on an action runs it and closes the bar. A tap outside or Back closes it.
- Tests: `tests/it/phone_touch.rs` (Clear clears, Delete opens its dialog, Copy fills the clipboard) and
  unit tests of `handle_range` in `phone.rs`. The handle drag and the long-press timer are not covered by
  kittest; check them on the device.
- Snapshots: `~/.cache/gc-spike/sel-a-range-handles.png` (A1:B4 with handles) and
  `~/.cache/gc-spike/sel-b-action-bar.png` (the bar open). `snapshot --phone-sheet actionbar` opens it.
- Checks: `cargo clippy -p gridcraft-ui-egui --all-targets` clean; `cargo test -p gridcraft-ui-egui` passes
  except the known `header_autofit` flake.

Emulator recipe (not run here, another agent had the device): a 1080x2400 screen, cell A1 near (140,487),
B4 near (275,610).
1. Tap A1, then drag the top-left handle down-right: the area follows the finger, the bar is closed.
2. Long-press B4 for about 600 ms: the bar opens below or above the area; Clear empties the area.
3. Drag the bottom-right handle past the screen's bottom edge: the grid scrolls.
4. Tap outside the bar, then Back: the bar closes and the app stays open.

## Update 10: dialogs as bottom sheets

- Phone (`phone::active`): every dialog is a bottom sheet, full width, with a title row and Close, a
  scrolling body in 16 pt gutters, and its buttons pinned at the bottom (48 pt tall, 12 pt above and below).
  Desktop keeps the floating window and the same body code (`dialogs::show` draws the body once, as a closure).
- `ok_cancel` (used by most forms) pins Cancel left and OK right. Save changes pins Don't Save, Cancel, Save.
  Enter presses OK; Escape, Back and Close cancel; a tap on the dimmed screen above the sheet cancels.
- Message boxes are a bottom sheet with the warning disc and message, and OK pinned (`phone::message_sheet`).
  The desktop message box is unchanged.
- Back closes an open dialog or message: `SheetApp::can_go_back` counts them, so the host sends Escape.
- Single-line dialog text fields are 48 pt tall on the phone (`phone::touch_edit`). Body controls use
  `phone::touch_style` (48 pt interact size, 16 pt text).
- Helpers added to `phone.rs` (additive): `dialog_sheet`, `footer`, `footer_row`, `message_sheet`,
  `touch_style`, `touch_edit`, `TOUCH_H`. The sheet's pinned row is reserved one frame after first use,
  as in WordCraft's `phone::dialog_sheet`, so a dialog's first frame draws its buttons in the body.
- Ceilings: the sheet is as tall as its content was last frame (the first frame of a sheet may be full
  height); DragValue and ComboBox rows use egui sizing, not checked on the device; the keyboard relies on
  the window's content rect shrinking with the soft keyboard, and the focused field's scroll into view
  relies on egui's TextEdit scrolling its ScrollArea. Not yet confirmed on the device.
- Tests: `cargo clippy -p gridcraft-ui-egui -p gridcraft-android` clean;
  `cargo test -q -p gridcraft-ui-egui -p gridcraft-android -- --skip header_autofit` passes.
  `build-native.sh x86_64` builds.
- Device check: NOT DONE. The emulator-5554 detached from adb during the run and did not come back
  (not restarted, per the rules). Pending: recovery message, Rename Sheet with the keyboard up,
  Format Cells (Number), Insert Function, Sort.

### Update 10 device results (x86_64 emulator, 2026-10-29)

- Recovery message: a bottom sheet with the warning disc and OK pinned full width (`dlg-recovery.png`). Passed.
- Rename Sheet (sheet pill, then the sheet's options, then Rename): the sheet stays above the keyboard while typing
  "Budget" and the field is 48 pt tall (`dlg-rename-kb.png`). The Cancel and OK buttons are about 46 pt tall on the
  emulator, just under 48 pt: not fixed yet.
- Format Cells: the tab row and the Number category overflowed the right edge (`dlg-format-cells.png`). Fixed:
  the tab row scrolls sideways, the preset format buttons wrap, and the category and tab rows are 48 pt on the phone
  (`dlg-format-cells2.png`). Commit a22880d2.
- Insert Function: the rows were a fixed 480 pt wide and ran past the sheet. Fixed: rows fill the sheet width and are
  48 pt tall (`dlg-insert-function2.png`). Commit 0b28bef9. The keyboard's number row shows above the search field.
- Back: the first Back press with the keyboard up hid the keyboard; a later Back closed the dialog. A further Back left
  the app to the home screen, which is expected with no dialog open.
- Not done: Sort (the Data tab was covered by the Formula Builder during the run, and the sort commands have no dialog
  to check on the phone yet).
## Update 11: relaunch in the same process

- Bug: eframe starts its winit event loop once per process. When Android destroyed the activity but kept
  the process (a font-scale or density change recreates the activity; memory pressure and Recents do the
  same), the new activity ran `android_main` again, eframe failed with `EventLoop can't be recreated`, and
  the window stayed black.
- Repro (x86_64 emulator, `bug1-before.png`): edit A1, then `settings put system font_scale 1.3`. Logcat shows
  `APP_CMD_DESTROY`, the process stays alive, and the activity is black.
- Fix, in two parts:
  - `AndroidManifest.xml`: `configChanges` now also lists `fontScale|density|locale|smallestScreenSize|touchscreen|colorMode`,
    so these changes are handled in `onConfigurationChanged` and the activity is not recreated.
  - `MainActivity.kt` `onDestroy`: a background thread ends the process 300 ms later (after the recovery flush).
    The next launch is a cold start and restores unsaved work from recovery. The kill runs on its own thread
    because `super.onDestroy()` can wait on the native loop.
- Device check (`bug1-after.png`): after the font change the grid is drawn, A1 still shows 42 and the title has
  the unsaved dot. Font scale reset to 1.0 afterwards.
- Not verified on the device: the destroy path without a configuration change (memory pressure, Recents,
  "Don't keep activities"). `always_finish_activities` did not destroy the activity on this emulator. The
  kill-after-destroy code runs there, but its cold-start and recovery restore are unconfirmed.
- Tried and dropped: relaunching from `onCreate` with an AlarmManager PendingIntent and killing the process. The
  process died during the activity relaunch and nothing started again, so the window stayed black.
- Rust is unchanged; `android_main` is not called twice now that the activity is not recreated.

### Update 10 follow-up (code only, device check pending)

- Footer buttons: device-checked 2026-10-30. Rename Sheet with the keyboard up shows Cancel and OK fully, 120 px tall
  (chk-2d), which is 48 pt at the app's 2.5 px per point (the 126 px figure assumed 420 dpi). The recovery sheet check
  failed: see the recovery bug below.
- Format Cells "Type (custom format code)" field uses `phone::touch_edit` (48 pt).
- Sort: checked on the device 2026-10-30. Data lists Sort A to Z, Sort Z to A and Filter as full-width 48 pt rows (chk-3a).

### Update 9 follow-up (code only, device check pending)

- The action bar shows four actions and "More" (`BAR_VISIBLE`); Fill Down, Insert and Delete are 48 pt rows
  under the bar, in the same frame, so the bar never scrolls sideways. Test `delete_in_the_action_bar...`
  goes through More.
- Merged `phone-touch-select` into custom-android (`bbda12c3`); worktree removed, branch kept.
- Device check (x86_64 emulator, 2026-10-30, `chk-*.png`): passed. Handle drag A1 to C4 selects A1:C4 (chk-4). Long-press
  B2 opens Cut, Copy, Paste, Clear, More (chk-5). More adds Fill Down, Insert and Delete (chk-6). Clear empties A1:A3
  (chk-7). Back closes the bar and the app stays open (chk-8d). A long-press at A2 right after a previous gesture
  did not open the bar; a second press did, so the 700 ms hold is timing-sensitive.

### Recovery copies are not written when GridCraft is backgrounded (open, 2026-10-30)

- Repro (x86_64 emulator): tap A1, type 42, Enter, then Home. `files/recovery` is never created, with or without
  `am kill` after it. Logcat shows the activity paused (`visible=false`).
- Cause: `MainActivity.onPause`/`onStop` call `nativeFlushRecovery`, which only sets `recovery::FLUSH`. The copy is
  written by `Shell::ui`'s `sync_recovery` on the next frame, and no frame runs while the activity is stopped.
- Not fixed here (needs a design change): either write the copy on each edit, or make the JNI call write from the
  shared state. Until then, unsaved work is lost when Android kills GridCraft in the background.

## Update 12: recovery written while editing

- Cause of the lost copy: `onPause`/`onStop` only set a flag, and the copy was written by the next frame. With the
  window gone no frame runs, so nothing was written. The 10:51 pass was a race with a frame that happened to run.
- Fix, two parts (`recovery.rs`, `app.rs`, `android.rs`, `MainActivity.kt`):
  - Write while working: each change to the dirty documents (uid, revision) restarts a 2 s idle timer
    (`recovery::write_due`, pure, unit-tested). `track_recovery` also keeps the latest state in `app::PENDING`
    (cheap Arc clones of the workbooks), so the copies' source is current as soon as the edit lands. When the timer
    runs out, the frame writes the copies; `request_repaint_after` wakes the loop for that.
  - Pause flush needs no frame: `nativeFlushRecovery` calls `app::write_pending()` on the calling thread. It encodes
    the pending workbooks and writes them. A `WRITE` lock serializes it with the timed write.
    (The first try waited up to 500 ms for a frame; the window does not draw while Java's main thread is blocked in
    onPause, so it always timed out.)
  - `onDestroy` flushes before the kill thread starts (the 300 ms sleep is gone), so the kill is ordered after the write.
- Clippy (`-p gridcraft-android -p gridcraft-ui-egui --all-targets`) clean. Host tests pass; `header_autofit` is the known flake.
  `build-native.sh x86_64` builds.
- Device results (x86_64 emulator, 2026-10-30, final build):
  - a (wait 4 s in the foreground, earlier build): `files/recovery` lists `doc-0.txt`, `doc-0.xlsx`; the timer fires at about 2 s.
  - b (type 42, Enter, HOME 0.3 s later, `ls` right after HOME, no wait): `doc-0.txt`, `doc-0.xlsx` present. The pause flush writes
    the copy with no frame. Kill and relaunch: A1 = 42, title `Book1 •` (`recb-3.png`).
  - c (app-bar Save, system picker, SAVE): "Saved Book1 (2).xlsx", title clean, `ls files/recovery` empty (`recb-c6.png`).
  - Note: the helper's focus check refuses taps while the system picker has focus. The picker SAVE was sent as raw
    `adb shell input tap` inside the helper call.
- Timing: a small workbook encodes and writes well under the 300 ms the Close path waits; no measurement beyond that.

## Update 13: File side drawer and unsaved prompt

- Phone app bar: the first button is the menu (three bars, 48 pt), as in WordCraft. It opens the File
  drawer: a 296 pt panel under the app bar with 48 pt rows, in WordCraft's order: Home, New, Open, Info,
  Save, Save As, Export, Options, Close. Pages open the Backstage; Save, Save As and Close run the File
  commands. Recover and Print are left out. Escape, Back and a press outside close it; the menu button
  itself toggles it. Code: `phone.rs` (`file_drawer`, `run_file_row`, `take_pointer`). Tests:
  `tests/it/phone_drawer.rs`.
- Close (drawer and Backstage) and Back at the top level with unsaved changes open the Save changes sheet.
  On Android `Services::close_quits` is set: Close and Back then end the app once every workbook is
  answered for (`SheetApp::continue_quit`, `quit_requested`, `ime::QUIT`, read by the Kotlin poll).
  Don't Save closes the workbook; Save runs the save and closes once it writes the file (`close_after_save`,
  `save_finished`, `files::save_done` reports cancels); Cancel leaves everything open. Without unsaved work
  Back still leaves the app (`moveTaskToBack`). Desktop and web keep the old Close (`close_quits` is false).
  Tests: `tests/it/quit_prompt.rs`.
- Checked on the emulator (x86_64 debug build): the drawer lists the nine rows; Save As opens the system
  save dialog and Cancel returns to the sheet with the workbook open and unsaved; Back after typing a
  value opens "Save Changes?"; Cancel keeps the sheet open; the drawer's Close opens the same sheet.
- Not yet checked on the device: Don't Save (app gone, relaunch shows a blank workbook with no
  recovery message) and Save then the system dialog closing the app. The emulator dropped off adb
  during this pass and was not restarted.
- Open: the Save As row's icon is the folder glyph, drawn in the accent colour (brown in this theme),
  so it looks heavier than the other rows; pick a neutral glyph. The Backstage pages opened from the
  drawer still use GridCraft's 196 pt side column on the phone (unchanged from before).
- Fixed (2026-10-30): a save that cannot be staged now reports as failed (`files::not_staged`), so the
  Save changes close flag clears. Test: `files::tests::a_save_as_of_a_workbook_is_adopted_and_an_export_is_not`.
- Fixed (2026-10-30): the Save As row uses its own line-art glyph (`Icon::SaveAs`) in the text colour.
  Device check (x86_64, 2026-10-30): drawer shows nine rows, Save As glyph matches the others;
  Close opens Save Changes?; Don't Save tapped, PID empty right after (app ended).
- Device check not finished: the emulator left adb while this pass ran. Not checked: the recovery
  folder listing (`run-as ... ls files/recovery` timed out), the relaunch shot (blank workbook, no
  recovery message), and Save then the system dialog closing the app.

## Update 14: pinch zoom

- Two fingers on the grid zoom the sheet on the phone (`phone::active`). The zoom follows the fingers'
  spread with egui's `multi_touch().zoom_delta`, is clamped to 10–400%, and keeps the sheet point under
  the fingers' centre fixed; moving the two fingers pans the sheet. Code: `grid.rs` (`Pinch`, `pinch_zoom`,
  `pinch_point`, `pinch_scroll`, the block at the top of the scroll section in `show`).
- The zoom is stored each frame with `view.zoom`, as the zoom slider does (rounded, no undo step). The scroll
  is re-anchored every frame, the last one included, because a zoom step takes effect the frame after it is set.
- While a pinch is on, and until every finger is up (the release included, so its click does not fire), the grid
  takes no selection, handle or long-press gesture (`pinch_busy` skips `phone::touch_gestures`). Desktop and the
  wheel's Ctrl zoom are unchanged; the wheel code skips a frame that has two touches on the phone.
- A pinch starts only once the fingers move, so a quick double tap, or two fingers held still, still tap.
- No pinch while a sheet, dialog, action bar or the in-cell editor is open (`SheetApp::can_go_back`).
- Ceilings: the action bar and selection handles are not drawn while fingers are down during a pinch (they come
  back after). Page Layout view does not pinch. The anchor uses the sheet's cell origin, so it is exact for
  frozen panes too (the frozen region is at the origin's scale).
- Tests: unit tests `pinch_keeps_the_point_under_the_fingers_fixed`, `pinch_zoom_is_clamped_to_the_sheet_range`,
  `pinch_out_at_the_top_left_never_scrolls_past_the_first_row_and_column` (grid.rs); kittest
  `tests/it/phone_pinch.rs` injects two touches (`egui::Event::Touch` with two `TouchId`s): spreading two
  fingers zooms 100% to 200% with the sheet point under the centre kept within 2 pt and no selection change;
  still fingers do not zoom; two quick taps still tap and do not zoom.
- Checks: `cargo clippy -p gridcraft-ui-egui --all-targets` clean; `cargo test -q -p gridcraft-ui-egui -- --skip
  header_autofit` passes; `cargo xtask wasm` passes.
- Device check: NOT DONE (the emulator is shared; no pinch was run). Recipe to try after merging:
  - `adb shell input` has no multi-touch (`tap`, `swipe`, `draganddrop`, `text`, `keyevent` only), so two
    fingers need `sendevent` on the touch device or UiAutomator (`UiObject2.pinchOpen`/`pinchClose`).
  - Find the touch node: `adb shell getevent -lp` (look for ABS_MT_POSITION_X/Y). Then write two type-B slots
    with `adb shell sendevent /dev/input/eventN <type> <code> <value>`: `EV_ABS ABS_MT_SLOT 0`, `ABS_MT_TRACKING_ID`,
    `ABS_MT_POSITION_X/Y` for each finger, `SYN_REPORT`, then move both apart over ~10 steps, then lift with
    `ABS_MT_TRACKING_ID -1`. The coordinates are in the touch device's range, not pixels; scale them from `getevent -lp`.
  - Expected: pinch out on the middle of the grid zooms in and the cell under the midpoint stays put; a pinch
    selects nothing; two quick taps on a cell still edit it.
  - Not verified: this recipe is not run yet; the exact `sendevent` values depend on the device.

## Update 15: Backstage and Options as phone pages

Code: `backstage.rs` (`phone_bar`, `back_button`, `page_body`, `nav_panel`, `options`, `category_row`,
`toggle_row`), `lib.rs` (`SheetApp::ui` draws the Backstage's own bar on a phone, the File drawer closes with
it; `open_backstage` resets the Options category), `phone.rs` (`close_file_drawer`). Desktop keeps the 196 pt
column, the 28 pt page heading, and the one-page Options with checkboxes.

- Phone Backstage: no left column. A 56 pt top bar with a 48 pt Back and the page name. Back returns to the
  sheet. Pages are full width with 16 pt gutters and 48 pt controls (`phone::touch_style`).
- Open: a Browse row (56 pt, opens the system picker), then recent files as 48 pt rows.
- New: blank workbook and the samples as two large tiles across.
- Info: one column, each label above its field; Protect Workbook is a full-width row.
- Save As: full-width type list and Browse row. Export: rows as before, full width.
- Options on a phone: four 48 pt category rows (General, Editing, Save, Formulas) with chevrons. A category
  opens full width under a "< Options" bar; that bar returns to the list (Back or Escape), and Back from the
  list returns to the sheet. On/off settings are switch rows (`toggle_row`, copied in look from WordCraft's
  `phone::toggle_row`); combo boxes are full width at 48 pt.
- Ceilings: Print (unreachable from the phone's drawer) keeps its desktop side-by-side layout; Account too.
  Action rows show their detail on one line and can run off the right edge for long text (Protect Workbook).
  The category page's back is the top bar, not a row in the content.
- Snapshot: `--dialog 'backstage={"page":"options"}' --category 1` opens a category (0 General, 1 Editing,
  2 Save, 3 Formulas).
- Tests: `tests/it/phone_backstage.rs` (Open shows Browse; Options list, category, Back, Back).
- Checks: `cargo clippy -p gridcraft-ui-egui -p gridcraft-android --all-targets -- -D warnings` clean;
  `cargo test -q -p gridcraft-ui-egui -p gridcraft-android -- --skip header_autofit` passes; `cargo xtask wasm`
  passes. `cargo fmt --check` still flags `dialogs.rs` and `apps/gridcraft-android/src/{android,app}.rs`
  (already unformatted before this change; not touched).


### Update 15 follow-up: Back walks out of Options (code and device check)

- Bug: Drawer → Options → Editing, then Android Back twice. Back #1 reached the list; Back #2 did nothing.
  Root cause: `backstage::options()` called `consume_key(Escape)` on every frame, including the list. `show()`
  then never saw the Escape that `can_go_back` (which counts an open Backstage) had asked for, so the view stayed.
  Fix in the shared path: `options()` consumes Escape only while a category is open; from the list `show()`
  closes the Backstage like the top-bar "‹".
- The top bar on a category shows the category name (Editing, Save, …); its back arrow returns to the list.
- Test: `phone_backstage::back_walks_out_of_options_to_the_sheet` (fails on the old code at the sheet assertion).
- Checks: `cargo clippy -p gridcraft-ui-egui -p gridcraft-android --all-targets -- -D warnings` clean;
  `cargo test -q -p gridcraft-ui-egui -p gridcraft-android -- --skip header_autofit` passes; `cargo xtask wasm` ok.
- Device check (x86_64 emulator, 2026-10-30, `bs-*.png` in `~/.cache/gc-spike/`): Options → Editing shows
  "Editing" in the bar (bs-a-editing); Back returns to the Options list (bs-b-options-list); Back again returns to
  the sheet with the bottom pill (bs-c-sheet). Drawer → Open → Back returns to the sheet (bs-f-open, bs-g-after-open-back).

## Update 16: opening files on the device

Checked on the x86_64 emulator (2026-10-30). Test files made by our CLI (`test-open.xlsx`, `test-second.xlsx`)
and LibreOffice (`test-open.ods`, `test-open.csv`) in `/sdcard/Download`, screenshots `open-*`, `v2-*`, `v3-*`,
`v4-*`, `f3-*` in `~/.cache/gc-spike/`.

- In-app Open: drawer, Open, Browse, picker, `test-open.xlsx`: opens, title is the file name, A1/B2 shown. Pass.
- Plain Save of a picked `.xlsx`: rewrites the same file with no picker ("Saved test-open.xlsx"); the file on
  disk changed (checked with `adb pull` and `gridcraft-cli cat`). Pass.
- Fixed: after that Save the title kept its unsaved dot. The Save bytes include a cell edit still in progress,
  which is committed inside the same command, so the revision taken at the start of the frame was older than
  the one written. The dirty state then stayed, and a relaunch restored a "Recovered unsaved changes" copy.
  Fix: the revision of the saved bytes is passed to `save_in_place` (commit b1b1f874).
- Fixed: a VIEW intent grants read access only (`--grant-read-uri-permission`). Plain Save then failed with
  "has no access to content://…", with no way to save. Now the write falls back to the system Save dialog
  (the Save As path) with the same name; the workbook adopts the new location (commit 9c2a59c1). On the device
  the dialog suggests `test-open (1).xlsx` because the name exists; the file was not overwritten.
- VIEW cold start (`am start -a VIEW -d content://media/external/downloads/<id>`): opens with the title and
  data. Pass.
- VIEW warm start (app running, clean workbook): opens `test-second.xlsx` as the active workbook. The previous
  workbook stays open, but the phone has no document switcher, so it is only reachable by closing the new one.
- CSV: cold VIEW opens with the grid (`f3-1-csv-cold.png`). ODS: cold VIEW opens with the info bar about data-only
  import (`f3-2-ods-cold.png`); the title is `… (imported).xlsx`, and Save asks for a new .xlsx. Pass.

Open problems (not fixed):
- Warm VIEW while a cell is being edited: the typed text that was not committed is dropped when the second
  workbook opens (the edit bar was cancelled). A typed-but-uncommitted value is lost.
- Warm VIEW has no switcher on the phone: the first workbook is still open, but nothing on the phone lists or
  switches to it. A touch-friendly choice (switch list, or ask "Save changes?" before opening) is not built.
- A read-only VIEW document saved through the fallback gets " (1)" in the name (Android's create rule), not an
  overwrite.
- Typing "Edited" with `input text` on the emulator kept only "Ed" (the rest arrived before the keyboard was
  ready). Not reproduced by hand; the IME path is unchanged.

Checks: `cargo clippy -p gridcraft-ui-egui -p gridcraft-android --all-targets -- -D warnings` clean;
`cargo test -q -p gridcraft-ui-egui -p gridcraft-android -- --skip header_autofit` passes.

## Update 17: switching between open workbooks

- The phone's File drawer lists the open workbooks above the File rows when two or more are open
  (`phone.rs`, `open_workbooks`, `workbook_row`). Each row is 48 pt: the name (an unsaved " •" after it),
  a check on the active one, and a 48 pt ✕. A tap switches (`window.activate`) and closes the drawer. The ✕
  makes that workbook active and runs File › Close (the Save changes sheet when it is dirty).
- A cell being edited is committed before a switch, before File › Open starts the picker, before
  `open_path` (Recent rows), and before the inbox opens documents handed over (VIEW, picked files).
  If that commit fails (a bad formula), the handed-over documents are dropped and the error shows; the
  cell edit stays open. Code: `lib.rs` (`open_dialog`, `open_path`, `logic`).
- Android: closing the active workbook with others open switches to another one and the app stays
  (`close_file`, `after_close`, `save_finished`, the Save changes answers in `dialogs.rs`). The last
  workbook still ends the app as before. Desktop and web are unchanged.
- Not done: the app bar does not show a count (skipped as not trivial).
- Tests: `tests/it/phone_workbooks.rs` (list and tap, single workbook has no list, commit before Open
  keeps typed text, clean close switches and keeps the app, dirty close asks then switches).
- Checks: `cargo clippy -p gridcraft-ui-egui -p gridcraft-android --all-targets -- -D warnings` clean;
  `cargo test -q -p gridcraft-ui-egui -p gridcraft-android -- --skip header_autofit` passes; `cargo xtask wasm` ok
  (two existing dead-code warnings in `engine/src/io.rs` and `ui-egui/src/backstage.rs`, not from this change).
- Device check (x86_64 emulator, 2026-10-30, `sw-*.png` in `~/.cache/gc-spike/`):
  - `sw-5`/`sw-6`/`sw-9`: with two workbooks (test-open.xlsx dirty, test-second.xlsx active) the drawer shows
    "Open workbooks" with both rows, the dirty dot, the check and the ✕s.
  - `sw-7`: a cell edit typed in A1 of test-open.xlsx, not committed, then a VIEW intent for test-second.xlsx.
    `sw-10`: after the switch back, A1 reads "Pnng" (committed) with the dirty dot.
  - `sw-11`/`sw-12`: ✕ on the dirty test-open row shows "Save Changes?"; Don't Save closes it and the app
    stays on test-second.xlsx.
  - Note: a cold/warm VIEW of an untouched blank Book1 replaces it (no second workbook); a bare
    `content://…/<id>` VIEW without the display name shows "Couldn't open <id>". Use `-p` and
    `--grant-read-uri-permission` as the test recipe.
- Open: the "Open with" chooser's first tap did not open the file; the pinned `-p` intent did.
