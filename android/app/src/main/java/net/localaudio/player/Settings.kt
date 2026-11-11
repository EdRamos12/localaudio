package net.localaudio.player

import android.content.Context

class Settings(context: Context) {
    private val prefs = context.getSharedPreferences("localaudio", Context.MODE_PRIVATE)

    var bufferMs: Int
        get() = prefs.getInt("buffer_ms", 60)
        set(value) = prefs.edit().putInt("buffer_ms", value).apply()

    var volume: Int
        get() = prefs.getInt("volume", 100)
        set(value) = prefs.edit().putInt("volume", value).apply()

    var passphrase: String
        get() = prefs.getString("passphrase", "").orEmpty()
        set(value) = prefs.edit().putString("passphrase", value).apply()

    var lastHost: String
        get() = prefs.getString("last_host", "").orEmpty()
        set(value) = prefs.edit().putString("last_host", value).apply()

    var lastPort: Int
        get() = prefs.getInt("last_port", 47321)
        set(value) = prefs.edit().putInt("last_port", value).apply()

    var lastName: String
        get() = prefs.getString("last_name", "").orEmpty()
        set(value) = prefs.edit().putString("last_name", value).apply()
}
