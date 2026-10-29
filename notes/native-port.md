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
