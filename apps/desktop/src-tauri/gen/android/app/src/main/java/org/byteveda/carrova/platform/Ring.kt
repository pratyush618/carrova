package org.byteveda.carrova.platform

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.MediaPlayer
import android.media.RingtoneManager
import android.os.Handler
import android.os.Looper

/** Ring glue for `carrova-platform` (android/ring.rs): the alarm stream at full volume. */
object Ring {
    private val main = Handler(Looper.getMainLooper())
    private var player: MediaPlayer? = null
    /** Alarm volume before the ring, restored when it ends. */
    private var volumeBefore: Int? = null

    @JvmStatic
    @Synchronized
    fun start(context: Context, seconds: Int) {
        stop(context)
        val audio = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        volumeBefore = audio.getStreamVolume(AudioManager.STREAM_ALARM)
        audio.setStreamVolume(AudioManager.STREAM_ALARM, audio.getStreamMaxVolume(AudioManager.STREAM_ALARM), 0)
        val tone = RingtoneManager.getDefaultUri(RingtoneManager.TYPE_ALARM)
            ?: RingtoneManager.getDefaultUri(RingtoneManager.TYPE_RINGTONE)
        player = MediaPlayer().apply {
            setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_ALARM)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                    .build()
            )
            setDataSource(context, tone)
            isLooping = true
            prepare()
            start()
        }
        main.postDelayed({ stop(context) }, seconds * 1000L)
    }

    @JvmStatic
    @Synchronized
    fun stop(context: Context) {
        main.removeCallbacksAndMessages(null)
        player?.run {
            stop()
            release()
        }
        player = null
        volumeBefore?.let {
            val audio = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
            audio.setStreamVolume(AudioManager.STREAM_ALARM, it, 0)
        }
        volumeBefore = null
    }
}
