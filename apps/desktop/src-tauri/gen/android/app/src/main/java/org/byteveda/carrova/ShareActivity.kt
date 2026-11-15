package org.byteveda.carrova

import android.app.Activity
import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import java.io.File
import java.util.UUID
import org.byteveda.Carrova.MainActivity

/**
 * Share target: hands what another app shares to the app as links (tao delivers them to Rust as
 * `RunEvent::Opened`; the UI picks what to do). Shared files are copied into the app's cache
 * first and passed as `file:` links: Rust sends paths, and the read grant is this activity's.
 */
class ShareActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val streams = streams(intent)
        if (streams.isEmpty()) {
            val text = intent.getStringExtra(Intent.EXTRA_TEXT)
            if (text == null) finish() else forward(Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, text))
            return
        }
        // Copying can take a while: off the main thread.
        Thread {
            val copies = copies(streams)
            runOnUiThread { if (copies.isEmpty()) finish() else forward(filesIntent(copies)) }
        }.start()
    }

    private fun forward(intent: Intent) {
        startActivity(intent.setClass(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        finish()
    }

    /**
     * Shared content URIs another app owns. `file:` URIs and this app's own providers are
     * refused: this app could read its private files through them and send them away.
     */
    private fun streams(intent: Intent): List<Uri> {
        val all = when (intent.action) {
            Intent.ACTION_SEND -> listOfNotNull(intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM))
            Intent.ACTION_SEND_MULTIPLE -> intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM).orEmpty()
            else -> emptyList()
        }
        return all.filter { uri ->
            val provider = uri.authority?.let { packageManager.resolveContentProvider(it, 0) }
            uri.scheme == "content" && provider != null && provider.packageName != packageName
        }
    }

    /**
     * Each stream copied to `files/cache/shared/<share>/<n>/<name>` (unreadable ones are left
     * out). Rust admits `file:` links only inside that dir (its cache root + `shared`).
     */
    // shortcut: copies stay until Android clears the cache; delete them after sending if space matters.
    private fun copies(streams: List<Uri>): List<Uri> {
        val share = File(filesDir, "cache/shared/${UUID.randomUUID()}")
        return streams.mapIndexedNotNull { index, uri ->
            try {
                val dir = File(share, index.toString()).apply { mkdirs() }
                val file = File(dir, displayName(uri))
                val input = contentResolver.openInputStream(uri) ?: return@mapIndexedNotNull null
                input.use { source -> file.outputStream().use { source.copyTo(it) } }
                Uri.fromFile(file)
            } catch (e: Exception) {
                null
            }
        }
    }

    /** The stream's file name, without any path. */
    private fun displayName(uri: Uri): String {
        val name = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
            ?.use { if (it.moveToFirst()) it.getString(0) else null }
            ?.substringAfterLast('/')
        return if (name.isNullOrBlank() || name == "." || name == "..") "shared" else name
    }

    private fun filesIntent(files: List<Uri>): Intent {
        val clip = ClipData.newRawUri(null, files.first())
        files.drop(1).forEach { clip.addItem(ClipData.Item(it)) }
        return Intent(Intent.ACTION_SEND_MULTIPLE).apply { clipData = clip }
    }
}
