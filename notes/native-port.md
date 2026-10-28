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
