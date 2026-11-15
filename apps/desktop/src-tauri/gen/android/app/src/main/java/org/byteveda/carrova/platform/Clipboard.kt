package org.byteveda.carrova.platform

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.os.PersistableBundle
import androidx.core.content.FileProvider
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.InputStream

/** Clipboard glue for `carrova-platform` (android/clipboard.rs). No decisions here. */
object Clipboard {
    /** `ClipDescription.EXTRA_IS_SENSITIVE` (named in API 33, honoured as a string before). */
    private const val EXTRA_IS_SENSITIVE = "android.content.extra.IS_SENSITIVE"

    /** The primary clip's first item, as Rust reads it. */
    class Clip(
        @JvmField val mimes: Array<String>,
        @JvmField val sensitive: String?,
        @JvmField val bytes: ByteArray?,
    )

    @JvmStatic
    fun read(context: Context, maxBytes: Long): Clip? {
        val clip = manager(context).primaryClip ?: return null
        if (clip.itemCount == 0) return null
        val description = clip.description
        val item = clip.getItemAt(0)
        val mimes = Array(description.mimeTypeCount) { description.getMimeType(it) }
        // Text (also a URI clip's link, never its target) or a PNG capped at one byte past maxBytes.
        val bytes = if (mimes.any { it.startsWith("text/") }) {
            (item.text?.toString() ?: item.uri?.toString())?.toByteArray(Charsets.UTF_8)
        } else if (mimes.contains("image/png")) {
            item.uri?.let { uri ->
                context.contentResolver.openInputStream(uri)?.use { readUpTo(it, maxBytes + 1) }
            }
        } else {
            null
        }
        @Suppress("DEPRECATION")
        val sensitive = description.extras?.get(EXTRA_IS_SENSITIVE)?.toString()
        return Clip(mimes, sensitive, bytes)
    }

    /** `InputStream.readNBytes(int)` needs API 33; minSdk is 29. */
    private fun readUpTo(stream: InputStream, limit: Long): ByteArray {
        val out = ByteArrayOutputStream()
        val buf = ByteArray(8192)
        var left = limit
        while (left > 0) {
            val n = stream.read(buf, 0, minOf(buf.size.toLong(), left).toInt())
            if (n < 0) break
            out.write(buf, 0, n)
            left -= n
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun write(context: Context, mime: String, bytes: ByteArray, sensitive: Boolean) {
        val clip = if (mime.startsWith("text/")) {
            ClipData.newPlainText("", String(bytes, Charsets.UTF_8))
        } else {
            val file = File(context.cacheDir, "clipboard.png").apply { writeBytes(bytes) }
            val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
            ClipData.newUri(context.contentResolver, "", uri)
        }
        if (sensitive) {
            clip.description.extras = PersistableBundle().apply {
                putBoolean(EXTRA_IS_SENSITIVE, true)
            }
        }
        manager(context).setPrimaryClip(clip)
    }

    private fun manager(context: Context) =
        context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
}
