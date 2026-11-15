package org.byteveda.carrova.platform

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.os.PersistableBundle
import androidx.core.content.FileProvider
import java.io.File

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
    fun read(context: Context): Clip? {
        val clip = manager(context).primaryClip ?: return null
        if (clip.itemCount == 0) return null
        val description = clip.description
        val item = clip.getItemAt(0)
        val bytes = item.text?.toString()?.toByteArray(Charsets.UTF_8)
            ?: item.uri?.let { uri ->
                context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
            }
        @Suppress("DEPRECATION")
        val sensitive = description.extras?.get(EXTRA_IS_SENSITIVE)?.toString()
        val mimes = Array(description.mimeTypeCount) { description.getMimeType(it) }
        return Clip(mimes, sensitive, bytes)
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
