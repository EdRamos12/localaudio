package net.localaudio.player

class OpusDecoder(sampleRate: Int, channels: Int) : AutoCloseable {
    private var handle: Long = nativeCreate(sampleRate, channels)

    init {
        check(handle != 0L) { "libopus decoder failed to start" }
    }

    fun decode(packet: ByteArray, pcm: ShortArray): Int {
        val samples = nativeDecode(handle, packet, pcm)
        check(samples > 0) { "opus decode failed ($samples)" }
        return samples
    }

    override fun close() {
        if (handle != 0L) {
            nativeDestroy(handle)
            handle = 0L
        }
    }

    private external fun nativeCreate(rate: Int, channels: Int): Long
    private external fun nativeDecode(handle: Long, packet: ByteArray, pcm: ShortArray): Int
    private external fun nativeDestroy(handle: Long)

    companion object {
        init {
            System.loadLibrary("localaudiojni")
        }
    }
}
