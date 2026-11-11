package net.localaudio.player

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.PowerManager
import androidx.core.app.ServiceCompat

// Keeps the process foreground while a stream is active so Android doesn't freeze it or cut its network.
// shortcut: StreamClient still lives in PlayerScreen, so a system-destroyed activity ends the stream; move it here if that bites.
class PlaybackService : Service() {
    private var wakeLock: PowerManager.WakeLock? = null

    override fun onBind(intent: Intent?) = null

    override fun onCreate() {
        super.onCreate()
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, "Playback", NotificationManager.IMPORTANCE_LOW),
        )
        // Launcher intent brings the existing task forward instead of stacking a second player.
        val open = PendingIntent.getActivity(
            this, 0, packageManager.getLaunchIntentForPackage(packageName), PendingIntent.FLAG_IMMUTABLE,
        )
        val notification = Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle("LocalAudio")
            .setContentText("Playing sound from your computer")
            .setContentIntent(open)
            .setOngoing(true)
            .build()
        ServiceCompat.startForeground(
            this, 1, notification,
            if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK else 0,
        )
        // Covers the buffering gaps when AudioTrack is paused and holds no wake lock of its own.
        wakeLock = getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "localaudio:playback")
            .apply { acquire() }
    }

    override fun onDestroy() {
        wakeLock?.release()
        super.onDestroy()
    }

    companion object {
        private const val CHANNEL = "playback"
    }
}
