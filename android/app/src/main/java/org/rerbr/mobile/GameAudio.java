package org.rerbr.mobile;

import android.content.Context;
import android.media.AudioAttributes;
import android.media.AudioFocusRequest;
import android.media.AudioFormat;
import android.media.AudioManager;
import android.media.AudioTrack;
import android.util.Log;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Owns Android playback. Rust supplies interleaved stereo PCM at the game's 44.1 kHz. */
final class GameAudio {
    private static native void render(ByteBuffer pcm);
    private static final int RATE = 44100;
    private static final int BLOCK_BYTES = 512 * 2 * Float.BYTES;
    private final AudioAttributes attributes = new AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_GAME).setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build();
    private final AudioManager manager;
    private final AudioFocusRequest request;
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private volatile int generation;
    private boolean resumed, playing;

    GameAudio(Context context) {
        manager = context.getSystemService(AudioManager.class);
        request = new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                .setAudioAttributes(attributes).setWillPauseWhenDucked(true)
                .setOnAudioFocusChangeListener(change -> setPlaying(resumed && change == AudioManager.AUDIOFOCUS_GAIN))
                .build();
    }

    void resume() {
        resumed = true;
        setPlaying(manager.requestAudioFocus(request) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED);
    }

    void pause() {
        resumed = false;
        setPlaying(false);
        manager.abandonAudioFocusRequest(request);
    }

    void close() {
        pause();
        worker.shutdown();
    }

    private void setPlaying(boolean enable) {
        if (enable == playing) return;
        playing = enable;
        int current = ++generation;
        if (enable) worker.execute(() -> play(current));
    }

    private void play(int current) {
        android.os.Process.setThreadPriority(android.os.Process.THREAD_PRIORITY_AUDIO);
        ByteBuffer pcm = ByteBuffer.allocateDirect(BLOCK_BYTES).order(ByteOrder.nativeOrder());
        AudioFormat format = new AudioFormat.Builder().setSampleRate(RATE)
                .setEncoding(AudioFormat.ENCODING_PCM_FLOAT).setChannelMask(AudioFormat.CHANNEL_OUT_STEREO).build();
        int min = AudioTrack.getMinBufferSize(RATE, AudioFormat.CHANNEL_OUT_STEREO, AudioFormat.ENCODING_PCM_FLOAT);
        while (generation == current) {
            AudioTrack track = null;
            try {
                track = new AudioTrack.Builder().setAudioAttributes(attributes).setAudioFormat(format)
                        .setBufferSizeInBytes(Math.max(min, BLOCK_BYTES * 2)).setTransferMode(AudioTrack.MODE_STREAM).build();
                track.play();
                boolean disconnected = false;
                while (generation == current && !disconnected) {
                    pcm.clear();
                    render(pcm);
                    while (pcm.hasRemaining() && generation == current) {
                        int written = track.write(pcm, pcm.remaining(), AudioTrack.WRITE_BLOCKING);
                        if (written == AudioTrack.ERROR_DEAD_OBJECT) { disconnected = true; break; }
                        if (written <= 0) throw new IllegalStateException("AudioTrack.write: " + written);
                    }
                }
            } catch (RuntimeException error) {
                Log.e("RBR", "AudioTrack playback failed", error);
                return;
            } finally {
                if (track != null) track.release();
            }
        }
    }
}
