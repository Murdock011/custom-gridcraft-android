package ai.storyteller.gridcraft.nativeapp

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.result.contract.ActivityResultContract
import java.io.File
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.text.TextUtils
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.FrameLayout
import androidx.activity.OnBackPressedCallback
import android.graphics.Color
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import com.google.androidgamesdk.GameActivity

/**
 * GameActivity with the system-bar and keyboard insets applied as padding on the content view, so
 * the native surface is laid out above the status bar, the navigation bar and the soft keyboard.
 *
 * Text input goes through [ImeView], a 1x1 focusable view with its own InputConnection, so the
 * keyboard composes and commits text the way it does in a text field. GameActivity's own
 * GameTextInput is not used for typing.
 */
class MainActivity : GameActivity() {
    private val handler = Handler(Looper.getMainLooper())
    /** Whether the keyboard is on screen, from the window insets. */
    private var imeShownNow = false
    /** Set across a rotation when the keyboard was hidden before it: the insets listener hides it again. */
    private var keepImeHidden = false
    private var keyboardShown = false
    /** Set on resume: the keyboard is not shown again until the document is edited or the screen is tapped. */
    private var resumeHold = false
    private lateinit var imeView: ImeView

    // The text the input method reads: the paragraph around the caret, with Rust's composition in it.
    // Rust's copy is pulled each poll, and this side's own edits (below) keep it right in between.
    private var ctxText = ""
    private var ctxCaret = 0
    private var ctxComp = -1
    private var ctxCompEnd = -1

    /** Messages sent to Rust. A context from Rust is taken only once Rust has read them all. */
    private var sent = 0L

    private fun send(kind: Int, text: String, a: Int, b: Int) {
        sent++
        nativeIme(kind, text, a, b)
    }

    /** Takes Rust's context when it has read every message sent (its count matches [sent]). */
    private fun pullContext() {
        val raw = nativeContext()
        val f = raw.split('\u0001', limit = 5)
        if (f.size < 5 || f[0].toLongOrNull() != sent) return
        ctxCaret = f[1].toIntOrNull() ?: return
        val comp = f[2].toIntOrNull() ?: -1
        // Rust dropped the composition (the caret moved away from it): the keyboard still has its
        // composing span, so restart the input to make it drop that span too. Posted, so this never
        // runs inside onCreateInputConnection.
        if (ctxComp >= 0 && comp < 0) {
            imeView.post {
                val imm = getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
                imm.restartInput(imeView)
            }
        }
        ctxComp = comp
        ctxCompEnd = f[3].toIntOrNull() ?: -1
        ctxText = f[4]
    }

    /** Replaces the context's UTF-16 range [s, e) with [t]; the composition is [t] when [composing]. */
    private fun edit(s: Int, e: Int, t: String, composing: Boolean) {
        val lo = minOf(s, e).coerceIn(0, ctxText.length)
        val hi = maxOf(s, e).coerceIn(0, ctxText.length)
        ctxText = ctxText.substring(0, lo) + t + ctxText.substring(hi)
        ctxCaret = lo + t.length
        ctxComp = if (composing && t.isNotEmpty()) lo else -1
        ctxCompEnd = if (ctxComp >= 0) lo + t.length else -1
    }

    /** The range a new text replaces: the composition if there is one, else nothing at the caret. */
    private fun replacedRange(): Pair<Int, Int> = if (ctxComp >= 0 && ctxCompEnd >= ctxComp) Pair(ctxComp, ctxCompEnd) else Pair(ctxCaret, ctxCaret)

    /** The index [i] steps back over one code point (a surrogate pair counts once). */
    private fun stepBack(i: Int): Int = if (i >= 2 && Character.isLowSurrogate(ctxText[i - 1]) && Character.isHighSurrogate(ctxText[i - 2])) i - 2 else i - 1

    /** The index one code point after [i]. */
    private fun stepForward(i: Int): Int = if (i + 1 < ctxText.length && Character.isHighSurrogate(ctxText[i]) && Character.isLowSurrogate(ctxText[i + 1])) i + 2 else i + 1

    /**
     * Erases [before] and [after] around the caret in the context, in code points or UTF-16 units.
     * Returns the code points erased before and after, which is what Rust counts.
     */
    private fun eraseAround(before: Int, after: Int, codePoints: Boolean): Pair<Int, Int> {
        val caret = ctxCaret.coerceIn(0, ctxText.length)
        var lo = caret
        var hi = caret
        if (codePoints) {
            var k = before.coerceAtLeast(0)
            while (k-- > 0 && lo > 0) lo = stepBack(lo)
            k = after.coerceAtLeast(0)
            while (k-- > 0 && hi < ctxText.length) hi = stepForward(hi)
        } else {
            lo = (lo - before.coerceAtLeast(0)).coerceAtLeast(0)
            hi = (hi + after.coerceAtLeast(0)).coerceAtMost(ctxText.length)
        }
        val beforeCp = Character.codePointCount(ctxText, lo, caret)
        val afterCp = Character.codePointCount(ctxText, caret, hi)
        ctxText = ctxText.substring(0, lo) + ctxText.substring(hi)
        ctxCaret = lo
        ctxComp = -1
        ctxCompEnd = -1
        return Pair(beforeCp, afterCp)
    }

    private val keyboardPoll = object : Runnable {
        override fun run() {
            nativeTakeRequest().takeIf { it.isNotEmpty() }?.let { files.add(it); nextFile() }
            nativeTakeClip().takeIf { it.isNotEmpty() }?.let { clipboard.setPrimaryClip(ClipData.newPlainText("GridCraft", it.substring(1))) }
            if (nativeQuitRequested()) {
                // Close: the save prompt already ran. End the process too, so the next launch is a cold start.
                nativeFlushRecovery()
                finishAndRemoveTask()
                handler.postDelayed({ android.os.Process.killProcess(android.os.Process.myPid()) }, 300)
                return
            }
            // After a resume the keyboard stays down unless the document was being edited or the user taps.
            val want = nativeWantsKeyboard() && (!resumeHold || nativeCanvasEditing())
            if (want) pullContext()
            if (want != keyboardShown) {
                keyboardShown = want
                if (want) showKeyboard() else hideKeyboard()
            } else if (want && !imeView.isFocused) {
                // A tap on the game surface takes focus from ImeView. Take it back only: restarting the input
                // or asking for the keyboard again on every tap is what made it hide and reappear.
                imeView.requestFocus()
            }
            handler.postDelayed(this, 150)
        }
    }

    // File requests from Rust (files.rs): "save\tticket\tname\tpath", "write\tticket\turi\tpath" or
    // "open\tdocument", one at a time.
    private val files = ArrayDeque<String>()
    private var filesBusy = false
    private var saveTicket = 0L
    private var saveSource = ""

    private companion object {
        const val XLSX_MIME = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    }

    private class CreateDoc : ActivityResultContract<Pair<String, String>, Uri?>() {
        override fun createIntent(context: Context, input: Pair<String, String>) =
            Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType(input.second).putExtra(Intent.EXTRA_TITLE, input.first)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)

        override fun parseResult(resultCode: Int, intent: Intent?): Uri? = if (resultCode == RESULT_OK) intent?.data else null
    }

    private class OpenDoc : ActivityResultContract<Array<String>, Uri?>() {
        override fun createIntent(context: Context, input: Array<String>) =
            Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*").putExtra(Intent.EXTRA_MIME_TYPES, input)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)

        override fun parseResult(resultCode: Int, intent: Intent?): Uri? = if (resultCode == RESULT_OK) intent?.data else null
    }

    private val createDoc = registerForActivityResult(CreateDoc()) { uri ->
        val ticket = saveTicket
        val src = File(saveSource)
        try {
            if (uri == null) {
                nativeSaveDone(ticket, 0, "", "")
            } else {
                val out = contentResolver.openOutputStream(uri, "wt") ?: throw java.io.FileNotFoundException(uri.toString())
                out.use { src.inputStream().use { input -> input.copyTo(it) } }
                keepAccess(uri)
                nativeSaveDone(ticket, 1, uri.toString(), displayName(uri))
            }
        } catch (e: Exception) {
            nativeSaveDone(ticket, -1, e.message ?: e.toString(), "")
        }
        src.delete()
        filesBusy = false
        nextFile()
    }

    private val openDoc = registerForActivityResult(OpenDoc()) { uri ->
        if (uri != null) {
            try {
                val name = displayName(uri)
                val dest = File(cacheDir, "open-${System.nanoTime()}.bin")
                val input = contentResolver.openInputStream(uri) ?: throw java.io.FileNotFoundException(uri.toString())
                input.use { src -> dest.outputStream().use { src.copyTo(it) } }
                keepAccess(uri)
                nativeOpened(name, dest.path, uri.toString())
            } catch (e: Exception) {
                nativeNotice(readFailure(displayName(uri), e))
            }
        }
        filesBusy = false
        nextFile()
    }

    /** Keep read and write access to a picked document after a restart, if the provider allows it. */
    private fun keepAccess(uri: Uri) {
        try {
            contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        } catch (e: SecurityException) {
            // Read only, or not persistable: keep the read access if the provider allows it. Otherwise
            // the picker's access lasts while the app runs.
            try {
                contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
            } catch (e2: SecurityException) {
                // Not persistable at all.
            }
        }
    }

    /** The provider's display name for [uri], or its last path segment when the provider has none. */
    private fun displayName(uri: Uri): String {
        val name = try {
            contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) c.getString(0) else null
            }
        } catch (e: Exception) {
            null
        }
        return name ?: uri.lastPathSegment ?: "document"
    }

    private fun nextFile() {
        if (filesBusy) return
        val line = files.removeFirstOrNull() ?: return
        val f = line.split('\t')
        if (f[0] == "write") {
            // Plain Save of a picked workbook: rewrite it without a picker. A failure is reported to Rust,
            // which shows it on the status line.
            val src = File(f[3])
            val ticket = f[1].toLong()
            val uri = Uri.parse(f[2])
            try {
                val out = contentResolver.openOutputStream(uri, "wt") ?: throw java.io.FileNotFoundException(f[2])
                out.use { o -> src.inputStream().use { it.copyTo(o) } }
                nativeSaveDone(ticket, 1, f[2], displayName(uri))
            } catch (e: Exception) {
                nativeSaveDone(ticket, -1, e.message ?: e.toString(), "")
            } finally {
                src.delete()
            }
            nextFile()
            return
        }
        filesBusy = true
        if (f[0] == "save") {
            // The system's Create dialog names the new document; the cache file holds its bytes.
            saveTicket = f[1].toLong()
            saveSource = f[3]
            val name = f[2]
            val mime = if (name.endsWith(".xlsx", ignoreCase = true)) XLSX_MIME else android.webkit.MimeTypeMap.getSingleton()
                .getMimeTypeFromExtension(name.substringAfterLast('.', "").lowercase()) ?: "application/octet-stream"
            createDoc.launch(Pair(name, mime))
        } else {
            openDoc.launch(arrayOf(
                XLSX_MIME,
                "application/vnd.ms-excel.sheet.macroEnabled.12",
                "application/vnd.ms-excel",
                "application/vnd.ms-excel.sheet.binary.macroEnabled.12",
                "application/vnd.oasis.opendocument.spreadsheet",
                "text/csv",
                "text/tab-separated-values",
                "text/plain",
            ))
        }
    }

    /** Why a document could not be opened, in words the status line can show. */
    private fun readFailure(name: String, e: Exception): String = when (e) {
        is java.io.FileNotFoundException -> "$name is no longer there. It was moved, renamed or deleted."
        is SecurityException -> "GridCraft can no longer open $name. Open it again from File > Open."
        else -> "Couldn't open $name: ${e.message ?: e.toString()}"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val content = findViewById<View>(android.R.id.content)
        ViewCompat.setOnApplyWindowInsetsListener(content) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout() or WindowInsetsCompat.Type.ime())
            v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            // The native side gets touch positions in window pixels, not relative to the padded surface.
            nativeSetInsets(bars.left, bars.top)
            imeShownNow = insets.isVisible(WindowInsetsCompat.Type.ime())
            if (keepImeHidden && imeShownNow) {
                (getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).hideSoftInputFromWindow(v.windowToken, 0)
            }
            WindowInsetsCompat.CONSUMED
        }
        imeView = ImeView()
        (content as FrameLayout).addView(imeView, FrameLayout.LayoutParams(1, 1))
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                // Back closes the innermost open dialog, sheet page, sheet, menu or Backstage first.
                if (nativeCanGoBack()) send(4, "", KeyEvent.KEYCODE_ESCAPE, 0) else leave()
            }
        })
        clipboard.addPrimaryClipChangedListener(clipListener)
        reportNight(resources.configuration)
        handler.post(keyboardPoll)
        // A restored instance (process death) must not reopen the document it was launched with.
        if (savedInstanceState == null) handleIntent(intent)
    }

    // singleTask: a document opened from another app while GridCraft runs arrives here.
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIntent(intent)
    }

    /** Open with (VIEW, EDIT) and Share (SEND) from the manifest's filters: a document another app sent. */
    private fun handleIntent(intent: Intent) {
        val uri = when (intent.action) {
            Intent.ACTION_VIEW, Intent.ACTION_EDIT -> intent.data
            Intent.ACTION_SEND -> streamExtra(intent)
            else -> null
        } ?: return
        openIncoming(uri, intent.type?.substringBefore(';')?.trim() ?: "")
    }

    @Suppress("DEPRECATION") // minSdk 29: getParcelableExtra(String) is the form that works on every supported API.
    private fun streamExtra(intent: Intent): Uri? = intent.getParcelableExtra(Intent.EXTRA_STREAM)

    /**
     * Reads a document another app handed over and opens it like a picked one, so the unsaved-changes
     * prompt and Save all apply. VIEW grants are often temporary: keeping access is tried, not required.
     */
    private fun openIncoming(uri: Uri, mime: String) {
        val name = displayName(uri)
        if (!nativeAccepts(name, mime)) {
            nativeNotice("Couldn't open $name: GridCraft opens Excel, OpenDocument spreadsheet, CSV and text files.")
            return
        }
        if (uri.scheme == "content") keepAccess(uri)
        try {
            val dest = File(cacheDir, "open-${System.nanoTime()}.bin")
            contentResolver.openInputStream(uri)?.use { input -> dest.outputStream().use { input.copyTo(it) } }
                ?: throw java.io.FileNotFoundException(uri.toString())
            nativeOpened(name, dest.path, uri.toString())
        } catch (e: Exception) {
            nativeNotice(readFailure(name, e))
        }
    }

    // GameActivity passes key events to the native side before the back dispatcher sees them.
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (event.keyCode == KeyEvent.KEYCODE_BACK) {
            if (event.action == KeyEvent.ACTION_UP) {
                if (nativeCanGoBack()) send(4, "", KeyEvent.KEYCODE_ESCAPE, 0) else leave()
            }
            return true
        }
        return super.dispatchKeyEvent(event)
    }

    /**
     * Back with nothing open: go to the background instead of finishing. The process (and the
     * open document) stays; eframe can run only once per process, so a finished activity would
     * leave the next launch without a running UI loop.
     */
    private fun leave() {
        moveTaskToBack(true)
    }

    private val clipboard by lazy { getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager }
    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener { readClip() }

    /** Hand the system clip's text to Rust, which Paste reads. Android only lets the focused app read it. */
    private fun readClip() {
        val text = clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(this)?.toString() ?: return
        nativeClip(text)
    }

    // configChanges includes uiMode, so a dark mode switch arrives here instead of recreating the activity.
    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        reportNight(newConfig)
        // Rotation makes Android show the keyboard again for the focused canvas, even after Back hid it.
        // If the keyboard was hidden before the rotation, keep it hidden for a short while (insets listener).
        if (!imeShownNow) {
            keepImeHidden = true
            handler.postDelayed({ keepImeHidden = false }, 1500)
        }
    }

    private fun reportNight(c: Configuration) {
        val night = c.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
        // The system-bar padding shows the window background, which the Light theme keeps light:
        // match it to the header colour and flip the bar icons so they stay readable.
        window.decorView.setBackgroundColor(if (night) Color.rgb(18, 19, 23) else Color.rgb(250, 250, 250))
        WindowCompat.getInsetsController(window, window.decorView).run {
            isAppearanceLightStatusBars = !night
            isAppearanceLightNavigationBars = !night
        }
        nativeSetNight(night)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus) return
        readClip()
        // Returning from Recents: Android restores the soft keyboard for the focused imeView before the poll runs.
        // Hide it here when the poll would not want it, and drop the focus that restores it.
        if (!(nativeWantsKeyboard() && (!resumeHold || nativeCanvasEditing()))) {
            keyboardShown = false
            imeView.clearFocus()
            hideKeyboard()
        }
    }

    override fun onResume() {
        super.onResume()
        resumeHold = true
    }

    // A tap is the user asking for the keyboard again (a settings field, the canvas): end the resume hold.
    override fun dispatchTouchEvent(ev: MotionEvent): Boolean {
        if (ev.actionMasked == MotionEvent.ACTION_DOWN) resumeHold = false
        return super.dispatchTouchEvent(ev)
    }

    // Android may kill a backgrounded process without warning: copy unsaved work now (Rust, recovery).
    override fun onPause() {
        super.onPause()
        nativeFlushRecovery()
    }

    override fun onStop() {
        super.onStop()
        nativeFlushRecovery()
    }

    override fun onDestroy() {
        // The UI loop ends with the activity. Let the last recovery flush run, then end the process; a new
        // activity in this process is restarted in a fresh one (onCreate). The kill runs on its own thread:
        // super.onDestroy() can wait on the native loop, and the main thread is what would post it.
        Thread {
            Thread.sleep(300)
            android.os.Process.killProcess(android.os.Process.myPid())
        }.start()
        clipboard.removePrimaryClipChangedListener(clipListener)
        handler.removeCallbacks(keyboardPoll)
        super.onDestroy()
    }


    private fun showKeyboard() {
        pullContext()
        imeView.requestFocus()
        val imm = getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        imm.restartInput(imeView)
        imm.showSoftInput(imeView, 0)
    }

    private fun hideKeyboard() {
        val imm = getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        imm.hideSoftInputFromWindow(imeView.windowToken, 0)
    }

    /** Rust side (android.rs): kind 0 commit, 1 composing text, 2 finish, 3 delete surrounding, 4 key. */
    private external fun nativeIme(kind: Int, text: String, a: Int, b: Int)

    /** Left and top padding in pixels that touch positions must be shifted by. */
    private external fun nativeSetInsets(left: Int, top: Int)

    /** Next file request from Rust, or an empty string. */
    private external fun nativeTakeRequest(): String

    /** A save ended: status 1 saved ([msg] is the document URI, [display] its name), 0 cancelled, else failed with [msg]. */
    private external fun nativeSaveDone(ticket: Long, status: Int, msg: String, display: String)

    /** A file operation failed: say [msg] on the status line. */
    private external fun nativeNotice(msg: String)

    /** The user picked the document at [uri]; its bytes are in the cache file at [path]. */
    private external fun nativeOpened(name: String, path: String, uri: String)

    /** Next copy for the system clipboard, prefixed with "c", or an empty string. */
    private external fun nativeTakeClip(): String

    /** Whether the system is in night mode. */
    private external fun nativeSetNight(night: Boolean)

    /** The system clipboard's text. */
    private external fun nativeClip(text: String)

    /** The activity is being backgrounded: write a recovery copy of unsaved work. */
    private external fun nativeFlushRecovery()

    /** Whether egui wants the soft keyboard. */
    private external fun nativeWantsKeyboard(): Boolean

    /** Whether the document canvas is being edited (no dialog, sheet or Backstage open). */
    private external fun nativeCanvasEditing(): Boolean

    /** Whether File › Close asked to end the app. */
    private external fun nativeQuitRequested(): Boolean

    /** The text around the caret for the input method: ack, caret, composing start and end, text. */
    private external fun nativeContext(): String

    /** Whether Android Back has a sheet, menu or Backstage to close. */
    private external fun nativeCanGoBack(): Boolean

    /** Whether GridCraft opens a document with this name and MIME type (files.rs accepts_document). */
    private external fun nativeAccepts(name: String, mime: String): Boolean

    private inner class ImeView : View(this@MainActivity) {
        init {
            isFocusable = true
            isFocusableInTouchMode = true
        }

        override fun onCheckIsTextEditor(): Boolean = true

        override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
            outAttrs.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
            outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN or EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_ACTION_NONE
            // The keyboard reads the text and caret when it attaches: give it Rust's current context.
            pullContext()
            outAttrs.initialSelStart = ctxCaret
            outAttrs.initialSelEnd = ctxCaret
            return Connection(this)
        }
    }

    /**
     * The keyboard's view of the text. Each edit changes the context at once (so the next read sees
     * it) and goes to Rust, which applies it to the document; Rust's context then replaces this one.
     */
    private inner class Connection(view: View) : BaseInputConnection(view, true) {
        override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
            val t = text?.toString() ?: ""
            replacedRange().let { edit(it.first, it.second, t, composing = false) }
            send(0, t, 0, 0)
            return true
        }

        override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean {
            val t = text?.toString() ?: ""
            replacedRange().let { edit(it.first, it.second, t, composing = true) }
            send(1, t, 0, 0)
            return true
        }

        override fun setComposingRegion(start: Int, end: Int): Boolean {
            val lo = minOf(start, end).coerceIn(0, ctxText.length)
            val hi = maxOf(start, end).coerceIn(0, ctxText.length)
            if (lo < hi) {
                ctxComp = lo
                ctxCompEnd = hi
                send(5, "", lo, hi)
            }
            return true
        }

        override fun finishComposingText(): Boolean {
            ctxComp = -1
            ctxCompEnd = -1
            send(2, "", 0, 0)
            return true
        }

        override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
            val (b, a) = eraseAround(beforeLength, afterLength, codePoints = false)
            send(3, "", b, a)
            return true
        }

        override fun deleteSurroundingTextInCodePoints(beforeLength: Int, afterLength: Int): Boolean {
            val (b, a) = eraseAround(beforeLength, afterLength, codePoints = true)
            send(3, "", b, a)
            return true
        }

        override fun sendKeyEvent(event: KeyEvent): Boolean {
            if (event.action == KeyEvent.ACTION_DOWN) {
                if (event.keyCode == KeyEvent.KEYCODE_DEL) eraseAround(1, 0, codePoints = true)
                // A printable key the keyboard sends as a key event (the first key after Enter did this)
                // carries its character as a commit; a bare key code would be dropped by Rust.
                val c = event.unicodeChar
                if (c >= 32 && event.keyCode != KeyEvent.KEYCODE_FORWARD_DEL) {
                    val t = String(Character.toChars(c))
                    replacedRange().let { edit(it.first, it.second, t, composing = false) }
                    send(0, t, 0, 0)
                } else {
                    send(4, "", event.keyCode, 0)
                }
            }
            return true
        }

        override fun performEditorAction(actionCode: Int): Boolean {
            send(4, "", KeyEvent.KEYCODE_ENTER, 0)
            return true
        }

        // The context's text before and after the caret, `n` UTF-16 units (a surrogate pair is kept whole).
        override fun getTextBeforeCursor(n: Int, flags: Int): CharSequence {
            if (n <= 0) return ""
            val end = ctxCaret.coerceIn(0, ctxText.length)
            var start = (end - n).coerceAtLeast(0)
            if (start in 1 until end && Character.isLowSurrogate(ctxText[start])) start++
            return ctxText.substring(start, end)
        }

        override fun getTextAfterCursor(n: Int, flags: Int): CharSequence {
            if (n <= 0) return ""
            val start = ctxCaret.coerceIn(0, ctxText.length)
            var end = (start + n).coerceAtMost(ctxText.length)
            if (end in start + 1 until ctxText.length && Character.isHighSurrogate(ctxText[end - 1])) end--
            return ctxText.substring(start, end)
        }

        // Selections are not reported: the caret is the only position the context knows.
        override fun getSelectedText(flags: Int): CharSequence = ""

        override fun getCursorCapsMode(reqModes: Int): Int = TextUtils.getCapsMode(ctxText, ctxCaret.coerceIn(0, ctxText.length), reqModes)

        override fun getExtractedText(request: ExtractedTextRequest?, flags: Int): ExtractedText? = ExtractedText().apply {
            text = ctxText
            startOffset = 0
            selectionStart = ctxCaret
            selectionEnd = ctxCaret
        }

        // Rust moves the caret only by keys and touches; an input method's request is not applied.
        override fun setSelection(start: Int, end: Int): Boolean = true
    }
}
