package net.localaudio.player

import android.content.Context
import android.media.AudioAttributes
import android.net.wifi.WifiManager
import android.os.Build
import android.media.AudioFormat
import android.media.AudioTrack
import android.os.Handler
import android.os.Looper
import android.util.Log
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetSocketAddress
import java.net.Socket
import java.util.TreeMap
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.sqrt

enum class Phase { Idle, Connecting, Buffering, Playing, Error }

data class PlayStats(
    val phase: Phase,
    val message: String = "",
    val packets: Long = 0,
    val lost: Long = 0,
    val rms: Int = 0,
    val delayMs: Int = 0,
    val remoteVolume: Int? = null,
) {
    val active: Boolean
        get() = phase == Phase.Connecting || phase == Phase.Buffering || phase == Phase.Playing
}

class StreamClient(context: Context, private val onStats: (PlayStats) -> Unit) {
    private val wifiLock = (context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager)
        .createWifiLock(
            if (Build.VERSION.SDK_INT >= 29) {
                WifiManager.WIFI_MODE_FULL_LOW_LATENCY
            } else {
                @Suppress("DEPRECATION")
                WifiManager.WIFI_MODE_FULL_HIGH_PERF
            },
            "localaudio:stream",
        )
        .apply { setReferenceCounted(false) }

    @Volatile var volumePercent: Int = 70
    @Volatile var bufferMs: Int = 60
    @Volatile private var remoteVolume: Int? = null

    private val running = AtomicBoolean(false)
    private val main = Handler(Looper.getMainLooper())
    private var thread: Thread? = null
    private var socket: Socket? = null
    private var udp: DatagramSocket? = null

    fun start(host: String, tcpPort: Int, passphrase: String) {
        stop()
        running.set(true)
        thread = Thread({
            try {
                session(host, tcpPort, passphrase)
            } catch (error: Exception) {
                if (running.getAndSet(false)) {
                    Log.e(TAG, "session failed", error)
                    publish(PlayStats(Phase.Error, friendlyError(error)))
                }
            }
        }, "localaudio-play").also { it.start() }
    }

    fun stop() {
        running.set(false)
        runCatching { socket?.close() }
        runCatching { udp?.close() }
        thread?.join(1500)
        thread = null
        publish(PlayStats(Phase.Idle))
    }

    private fun session(host: String, tcpPort: Int, passphrase: String) {
        publish(PlayStats(Phase.Connecting))
        val tcp = Socket()
        socket = tcp
        tcp.tcpNoDelay = true
        tcp.connect(InetSocketAddress(host, tcpPort), 4000)
        tcp.soTimeout = 8000
        val token = passphrase.ifBlank { "-" }
        tcp.getOutputStream().apply {
            write("HELLO phone $token\n".toByteArray())
            flush()
        }
        val welcome = readLine(tcp)
        if (welcome == "DENY") {
            running.set(false)
            publish(PlayStats(Phase.Error, "The computer asked for a different passphrase."))
            tcp.close()
            return
        }
        if (!welcome.startsWith("WELCOME")) {
            running.set(false)
            publish(PlayStats(Phase.Error, "That address isn't running LocalAudio."))
            tcp.close()
            return
        }

        val audio = DatagramSocket(null)
        audio.reuseAddress = true
        audio.receiveBufferSize = 256 * 1024
        audio.bind(InetSocketAddress(0))
        audio.soTimeout = 500
        udp = audio
        tcp.getOutputStream().apply {
            write("LISTEN ${audio.localPort}\n".toByteArray())
            flush()
        }
        Log.i(TAG, "listening udp ${audio.localPort} for $host")
        publish(PlayStats(Phase.Buffering))
        wifiLock.acquire()

        val decoder = OpusDecoder(48_000, 2)
        val buffer = JitterBuffer()
        val track = buildTrack()
        val packets = java.util.concurrent.atomic.AtomicLong()
        val lost = java.util.concurrent.atomic.AtomicLong()
        val rms = java.util.concurrent.atomic.AtomicInteger()
        val delayMs = java.util.concurrent.atomic.AtomicInteger()
        val player = Thread({
            android.os.Process.setThreadPriority(android.os.Process.THREAD_PRIORITY_URGENT_AUDIO)
            playLoop(track, buffer, lost, rms, delayMs)
        }, "localaudio-audio")
        player.start()

        val reader = Thread({ readControl(tcp) }, "localaudio-control")
        reader.start()

        val packet = DatagramPacket(ByteArray(4096), 4096)
        val pcm = ShortArray(960 * 2)
        var lastLog = System.nanoTime()
        var lastPublish = System.nanoTime()
        var lastPing = System.nanoTime()
        try {
            while (running.get()) {
                if (System.nanoTime() - lastPing > 5_000_000_000L) {
                    lastPing = System.nanoTime()
                    runCatching {
                        tcp.getOutputStream().apply {
                            write("PING\n".toByteArray())
                            flush()
                        }
                    }
                }
                try {
                    audio.receive(packet)
                } catch (_: java.net.SocketTimeoutException) {
                    publish(
                        PlayStats(
                            Phase.Buffering,
                            "Connected, waiting for sound from the computer",
                            packets = packets.get(),
                            lost = lost.get(),
                            remoteVolume = remoteVolume,
                        ),
                    )
                    continue
                }
                val parsed = parsePacket(packet.data, packet.length) ?: continue
                val samples = try {
                    decoder.decode(parsed.payload, pcm)
                } catch (error: Exception) {
                    Log.w(TAG, "decode: ${error.message}")
                    continue
                }
                buffer.push(parsed.sequence, pcm.copyOf(samples * 2))
                val count = packets.incrementAndGet()
                val now = System.nanoTime()
                if (now - lastPublish > 100_000_000L) {
                    lastPublish = now
                    val playing = track.playState == AudioTrack.PLAYSTATE_PLAYING
                    val stats = PlayStats(
                        phase = if (playing) Phase.Playing else Phase.Buffering,
                        packets = count,
                        lost = lost.get(),
                        rms = rms.get(),
                        delayMs = delayMs.get(),
                        remoteVolume = remoteVolume,
                    )
                    publish(stats)
                    if (now - lastLog > 1_000_000_000L) {
                        lastLog = now
                        Log.i(
                            TAG,
                            "stats packets=${stats.packets} lost=${stats.lost} rms=${stats.rms} delayMs=${stats.delayMs} queued=${buffer.depth()} underruns=${track.underrunCount} playing=$playing",
                        )
                    }
                }
            }
        } finally {
            running.set(false)
            if (wifiLock.isHeld) wifiLock.release()
            player.join(1000)
            reader.join(500)
            runCatching { track.pause() }
            track.release()
            decoder.close()
            runCatching { audio.close() }
            runCatching { tcp.close() }
        }
    }

    private fun readControl(tcp: Socket) {
        try {
            tcp.soTimeout = 0
            while (running.get()) {
                val line = readLine(tcp)
                if (line.startsWith("VOLUME ")) {
                    val value = line.removePrefix("VOLUME ").trim().toIntOrNull()?.coerceIn(0, 100)
                    if (value != null) {
                        volumePercent = value
                        remoteVolume = value
                    }
                }
            }
        } catch (_: Exception) {
            if (running.getAndSet(false)) {
                publish(PlayStats(Phase.Error, "The computer stopped streaming."))
            }
        }
    }

    private fun playLoop(
        track: AudioTrack,
        buffer: JitterBuffer,
        lost: java.util.concurrent.atomic.AtomicLong,
        rms: java.util.concurrent.atomic.AtomicInteger,
        delayMs: java.util.concurrent.atomic.AtomicInteger,
    ) {
        var primed = false
        var started = false
        var appliedMs = -1
        var writtenFrames = 0L
        var extraMs = 0
        var steadySince = System.nanoTime()
        var lastWrite = System.nanoTime()

        fun play(frame: ShortArray) {
            rms.set(rmsOf(frame))
            track.setVolume(volumePercent / 100f)
            writePcm(track, frame)
            writtenFrames += frame.size / 2
            lastWrite = System.nanoTime()
        }

        while (running.get()) {
            if (extraMs > 0 && System.nanoTime() - steadySince > 15_000_000_000L) {
                extraMs -= 20
                steadySince = System.nanoTime()
            }
            val targetMs = (bufferMs.coerceIn(20, 200) + extraMs).coerceAtMost(MAX_BUFFER_MS)
            val targetFrames = targetMs / 20
            if (targetMs != appliedMs) {
                val granted = track.setBufferSizeInFrames(targetMs * 48 + 960)
                Log.i(TAG, "buffer ${targetMs}ms -> ${granted / 48}ms")
                appliedMs = targetMs
            }

            if (!primed) {
                if (buffer.depth() < targetFrames) {
                    sleepBriefly()
                    continue
                }
                lost.addAndGet(buffer.trimTo(targetFrames).toLong())
                var filled = 0
                while (filled < targetFrames) {
                    when (val next = buffer.pop(targetFrames)) {
                        is JitterBuffer.Pop.Frame -> {
                            play(next.pcm)
                            filled++
                        }
                        is JitterBuffer.Pop.Gap -> lost.addAndGet(next.skipped)
                        null -> break
                    }
                }
                track.play()
                if (!started) Log.i(TAG, "AudioTrack started")
                started = true
                primed = true
                continue
            }

            if (buffer.depth() > EXCESS_FRAMES) {
                lost.addAndGet(buffer.trimTo(REORDER_FRAMES).toLong())
            }
            when (val next = buffer.pop(REORDER_FRAMES)) {
                is JitterBuffer.Pop.Frame -> play(next.pcm)
                is JitterBuffer.Pop.Gap -> lost.addAndGet(next.skipped)
                null -> {
                    val starvedMs = (System.nanoTime() - lastWrite) / 1_000_000L
                    if (starvedMs > targetMs + 20) {
                        extraMs = (extraMs + 20).coerceAtMost(MAX_BUFFER_MS - 20)
                        steadySince = System.nanoTime()
                        Log.i(TAG, "ran dry after ${starvedMs}ms, cushion +${extraMs}ms")
                        track.pause()
                        primed = false
                        continue
                    }
                    sleepBriefly()
                }
            }
            val head = track.playbackHeadPosition.toLong() and 0xffffffffL
            val pending = (writtenFrames - head).coerceAtLeast(0)
            delayMs.set((pending / 48 + buffer.depth() * 20).toInt())
        }
    }

    private fun writePcm(track: AudioTrack, pcm: ShortArray) {
        val bytes = ByteArray(pcm.size * 2)
        var index = 0
        for (sample in pcm) {
            val value = sample.toInt()
            bytes[index++] = (value and 0xff).toByte()
            bytes[index++] = ((value shr 8) and 0xff).toByte()
        }
        val wrote = track.write(bytes, 0, bytes.size, AudioTrack.WRITE_BLOCKING)
        if (wrote < 0) Log.w(TAG, "AudioTrack write $wrote")
    }

    private fun publish(stats: PlayStats) {
        main.post { onStats(stats) }
    }

    private fun sleepBriefly() {
        try {
            Thread.sleep(2)
        } catch (_: InterruptedException) {
            Thread.currentThread().interrupt()
        }
    }

    private fun readLine(socket: Socket): String {
        val buffer = StringBuilder()
        val input = socket.getInputStream()
        while (true) {
            val value = input.read()
            if (value < 0) error("Connection closed")
            if (value == '\n'.code) break
            if (value != '\r'.code) buffer.append(value.toChar())
        }
        return buffer.toString()
    }

    companion object {
        const val TAG = "LocalAudio"
        private const val REORDER_FRAMES = 2
        private const val EXCESS_FRAMES = 5
        private const val MAX_BUFFER_MS = 220

        private fun friendlyError(error: Exception): String = when (error) {
            is java.net.ConnectException -> "Couldn't reach the computer. Is LocalAudio open there?"
            is java.net.SocketTimeoutException -> "The computer didn't answer in time."
            is java.net.UnknownHostException -> "That address doesn't exist on this network."
            is java.net.NoRouteToHostException -> "This phone can't reach that address."
            else -> error.message ?: "Connection failed."
        }

        private fun buildTrack(): AudioTrack {
            val min = AudioTrack.getMinBufferSize(
                48_000,
                AudioFormat.CHANNEL_OUT_STEREO,
                AudioFormat.ENCODING_PCM_16BIT,
            )
            val size = maxOf(min, 48 * 4 * 240)
            return try {
                track(size, lowLatency = true)
            } catch (error: UnsupportedOperationException) {
                Log.w(TAG, "low latency unavailable: ${error.message}")
                track(size, lowLatency = false)
            }
        }

        private fun track(size: Int, lowLatency: Boolean): AudioTrack {
            val builder = AudioTrack.Builder()
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                        .build(),
                )
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                        .setSampleRate(48_000)
                        .setChannelMask(AudioFormat.CHANNEL_OUT_STEREO)
                        .build(),
                )
                .setBufferSizeInBytes(size)
                .setTransferMode(AudioTrack.MODE_STREAM)
            if (lowLatency) {
                builder.setPerformanceMode(AudioTrack.PERFORMANCE_MODE_LOW_LATENCY)
            }
            return builder.build()
        }

        private fun rmsOf(pcm: ShortArray): Int {
            if (pcm.isEmpty()) return 0
            var energy = 0.0
            for (sample in pcm) energy += sample * sample.toDouble()
            return sqrt(energy / pcm.size).toInt()
        }

        private fun parsePacket(data: ByteArray, length: Int): AudioPacket? {
            if (length < 14) return null
            if (data[0] != 'L'.code.toByte() || data[1] != 'A'.code.toByte() ||
                data[2] != 'U'.code.toByte() || data[3] != '1'.code.toByte()
            ) {
                return null
            }
            val sequence = le32(data, 4)
            val payloadLen = le16(data, 12)
            if (payloadLen <= 0 || 14 + payloadLen > length) return null
            return AudioPacket(sequence, data.copyOfRange(14, 14 + payloadLen))
        }

        private fun le32(data: ByteArray, offset: Int): Long {
            return (data[offset].toLong() and 0xff) or
                ((data[offset + 1].toLong() and 0xff) shl 8) or
                ((data[offset + 2].toLong() and 0xff) shl 16) or
                ((data[offset + 3].toLong() and 0xff) shl 24)
        }

        private fun le16(data: ByteArray, offset: Int): Int {
            return (data[offset].toInt() and 0xff) or ((data[offset + 1].toInt() and 0xff) shl 8)
        }
    }
}

private data class AudioPacket(val sequence: Long, val payload: ByteArray)

private class JitterBuffer {
    private val frames = TreeMap<Long, ShortArray>()
    private var next: Long? = null

    @Synchronized
    fun push(sequence: Long, pcm: ShortArray) {
        val expected = next
        if (expected == null) {
            next = sequence
        } else if (sequence < expected) {
            return
        }
        frames[sequence] = pcm
        while (frames.size > 40) frames.pollFirstEntry()
    }

    @Synchronized
    fun depth(): Int = frames.size
    @Synchronized
    fun trimTo(maxPackets: Int): Int {
        var dropped = 0
        while (frames.size > maxPackets) {
            frames.pollFirstEntry()
            dropped++
        }
        if (dropped > 0) next = frames.firstKey()
        return dropped
    }

    @Synchronized
    fun pop(targetPackets: Int): Pop? {
        val expected = next ?: return null
        val ready = frames.remove(expected)
        if (ready != null) {
            next = expected + 1
            return Pop.Frame(ready)
        }
        val first = frames.firstEntry() ?: return null
        if (frames.size >= targetPackets && first.key > expected) {
            val skipped = first.key - expected
            next = first.key
            return Pop.Gap(skipped)
        }
        return null
    }

    sealed interface Pop {
        data class Frame(val pcm: ShortArray) : Pop
        data class Gap(val skipped: Long) : Pop
    }
}
