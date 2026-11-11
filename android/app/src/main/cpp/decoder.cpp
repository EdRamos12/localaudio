#include <jni.h>
#include <opus.h>

extern "C" JNIEXPORT jlong JNICALL
Java_net_localaudio_player_OpusDecoder_nativeCreate(JNIEnv *, jobject, jint rate, jint channels) {
    int error = OPUS_OK;
    OpusDecoder *decoder = opus_decoder_create(rate, channels, &error);
    if (error != OPUS_OK || decoder == nullptr) {
        return 0;
    }
    return reinterpret_cast<jlong>(decoder);
}

extern "C" JNIEXPORT jint JNICALL
Java_net_localaudio_player_OpusDecoder_nativeDecode(
        JNIEnv *env, jobject, jlong handle, jbyteArray packet, jshortArray pcm) {
    auto *decoder = reinterpret_cast<OpusDecoder *>(handle);
    if (decoder == nullptr || packet == nullptr || pcm == nullptr) {
        return -1;
    }
    jbyte *input = env->GetByteArrayElements(packet, nullptr);
    jshort *output = env->GetShortArrayElements(pcm, nullptr);
    if (input == nullptr || output == nullptr) {
        if (input != nullptr) env->ReleaseByteArrayElements(packet, input, JNI_ABORT);
        if (output != nullptr) env->ReleaseShortArrayElements(pcm, output, JNI_ABORT);
        return -1;
    }
    const int input_len = env->GetArrayLength(packet);
    const int output_len = env->GetArrayLength(pcm);
    const int frame_size = output_len / 2;
    const int decoded = opus_decode(
            decoder,
            reinterpret_cast<const unsigned char *>(input),
            input_len,
            output,
            frame_size,
            0);
    env->ReleaseByteArrayElements(packet, input, JNI_ABORT);
    env->ReleaseShortArrayElements(pcm, output, 0);
    return decoded;
}

extern "C" JNIEXPORT void JNICALL
Java_net_localaudio_player_OpusDecoder_nativeDestroy(JNIEnv *, jobject, jlong handle) {
    auto *decoder = reinterpret_cast<OpusDecoder *>(handle);
    if (decoder != nullptr) {
        opus_decoder_destroy(decoder);
    }
}
