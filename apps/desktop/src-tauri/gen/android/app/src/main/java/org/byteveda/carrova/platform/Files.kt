package org.byteveda.carrova.platform

import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.os.ParcelFileDescriptor
import android.provider.MediaStore
import androidx.core.content.FileProvider
import java.io.File

/** Files glue for `carrova-platform` (android/files.rs). No decisions here. */
object Files {
    @JvmStatic
    fun filesDir(context: Context): String = context.filesDir.absolutePath

    @JvmStatic
    @Suppress("DEPRECATION")
    fun downloadsDir(context: Context): String =
        Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS).absolutePath

    /** A pending Downloads entry (hidden until [publish]); returns its URI. */
    @JvmStatic
    fun create(context: Context, name: String, mime: String): String {
        val values = ContentValues().apply {
            put(MediaStore.Downloads.DISPLAY_NAME, name)
            put(MediaStore.Downloads.MIME_TYPE, mime)
            put(MediaStore.Downloads.IS_PENDING, 1)
        }
        val uri = context.contentResolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
            ?: throw IllegalStateException("MediaStore insert failed")
        return uri.toString()
    }

    /** A read-write descriptor of [create]'s entry, handed over to Rust. */
    @JvmStatic
    fun open(context: Context, uri: String): Int {
        val pfd = context.contentResolver.openFileDescriptor(Uri.parse(uri), "rw")
            ?: throw IllegalStateException("no descriptor")
        return pfd.detachFd()
    }

    /** Make the entry visible; returns its final display name. */
    @JvmStatic
    fun publish(context: Context, uri: String): String {
        val entry = Uri.parse(uri)
        val values = ContentValues().apply { put(MediaStore.Downloads.IS_PENDING, 0) }
        context.contentResolver.update(entry, values, null, null)
        context.contentResolver.query(entry, arrayOf(MediaStore.Downloads.DISPLAY_NAME), null, null, null)
            ?.use { if (it.moveToFirst()) return it.getString(0) }
        throw IllegalStateException("entry gone")
    }

    @JvmStatic
    fun discard(context: Context, uri: String) {
        context.contentResolver.delete(Uri.parse(uri), null, null)
    }

    @JvmStatic
    fun openUrl(context: Context, url: String) {
        context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }

    @JvmStatic
    fun openPath(context: Context, path: String) {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", File(path))
        val intent = Intent(Intent.ACTION_VIEW)
            .setDataAndType(uri, context.contentResolver.getType(uri))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_GRANT_READ_URI_PERMISSION)
        context.startActivity(intent)
    }
}
