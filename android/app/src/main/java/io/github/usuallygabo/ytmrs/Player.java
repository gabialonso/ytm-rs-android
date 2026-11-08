package io.github.usuallygabo.ytmrs;

import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.media.AudioAttributes;
import android.media.AudioFocusRequest;
import android.media.AudioManager;
import android.media.MediaMetadata;
import android.media.session.MediaSession;
import android.media.session.PlaybackState;
import android.net.wifi.WifiManager;
import android.os.Handler;
import android.os.Looper;
import android.os.PowerManager;
import android.os.SystemClock;
import android.util.Log;

import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * What the system knows of the player: the MediaSession (lock screen, notification,
 * headset and Bluetooth buttons), audio focus, and the locks that keep streaming going
 * with the screen off. The player itself is in Rust; this follows what Rust says and
 * passes the buttons back. Main thread only.
 */
final class Player {
    private static final String TAG = "ytm-rs";
    private static Player instance;

    static Player get(Context c) {
        if (instance == null) instance = new Player(c.getApplicationContext());
        return instance;
    }

    final Context ctx;
    final MediaSession session;
    private final AudioManager audio;
    private final AudioFocusRequest focus;
    private final PowerManager.WakeLock cpu;
    private final WifiManager.WifiLock wifi;
    private final Handler main = new Handler(Looper.getMainLooper());
    private final ExecutorService covers = Executors.newSingleThreadExecutor();

    String title = "", artist = "", album = "";
    long durationMs = -1;
    Bitmap art;
    private String coverSource;
    boolean hasTrack, playing;
    private boolean moving, hasFocus, resumeOnGain, noisyListening;
    private long positionMs;

    private Player(Context c) {
        ctx = c;
        audio = c.getSystemService(AudioManager.class);
        session = new MediaSession(c, "ytm-rs");
        session.setCallback(new MediaSession.Callback() {
            @Override public void onPlay() { Bridge.onMediaCommand(Bridge.CMD_PLAY, 0); }
            @Override public void onPause() { Bridge.onMediaCommand(Bridge.CMD_PAUSE, 0); }
            @Override public void onSkipToNext() { Bridge.onMediaCommand(Bridge.CMD_NEXT, 0); }
            @Override public void onSkipToPrevious() { Bridge.onMediaCommand(Bridge.CMD_PREVIOUS, 0); }
            @Override public void onStop() { Bridge.onMediaCommand(Bridge.CMD_PAUSE, 0); }
            @Override public void onSeekTo(long pos) { Bridge.onMediaCommand(Bridge.CMD_SEEK, pos); }
        });
        Intent open = new Intent(c, MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP);
        session.setSessionActivity(PendingIntent.getActivity(c, 0, open, PendingIntent.FLAG_IMMUTABLE));

        AudioAttributes attrs = new AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_MEDIA)
                .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                .build();
        focus = new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                .setAudioAttributes(attrs)
                .setOnAudioFocusChangeListener(this::onFocusChange, main)
                .build();

        cpu = c.getSystemService(PowerManager.class).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "ytm-rs:playback");
        cpu.setReferenceCounted(false);
        wifi = c.getSystemService(WifiManager.class).createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "ytm-rs:playback");
        wifi.setReferenceCounted(false);
        publishState();
    }

    void setTrack(String title, String artist, String album, String cover, long durationMs) {
        this.title = title;
        this.artist = artist;
        this.album = album;
        this.durationMs = durationMs;
        hasTrack = true;
        // The same song's art again (the file copy arriving after the URL, say) keeps
        // what's shown until the new picture is in.
        if (!cover.equals(coverSource)) {
            coverSource = cover;
            if (cover.isEmpty()) art = null;
            else loadCover(cover);
        }
        publishMetadata();
        session.setActive(true);
        PlaybackService.sync(ctx);
    }

    void setState(boolean playing, boolean moving, long positionMs) {
        boolean changed = playing != this.playing;
        this.playing = playing;
        this.moving = moving;
        this.positionMs = positionMs;
        publishState();
        if (changed) {
            if (playing) {
                resumeOnGain = false;
                if (!hasFocus) hasFocus = audio.requestAudioFocus(focus) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED;
                listenForNoisy(true);
                cpu.acquire();
                wifi.acquire();
            } else {
                // Paused for a call (say): keep the focus to hear when it's ours again.
                if (!resumeOnGain && hasFocus) {
                    audio.abandonAudioFocusRequest(focus);
                    hasFocus = false;
                }
                listenForNoisy(false);
                if (cpu.isHeld()) cpu.release();
                if (wifi.isHeld()) wifi.release();
            }
            PlaybackService.sync(ctx);
        }
    }

    private void onFocusChange(int change) {
        switch (change) {
            case AudioManager.AUDIOFOCUS_GAIN:
                if (resumeOnGain && !playing) Bridge.onMediaCommand(Bridge.CMD_PLAY, 0);
                resumeOnGain = false;
                break;
            case AudioManager.AUDIOFOCUS_LOSS_TRANSIENT:
                // A call, a navigation prompt: pause, and play on when it's over.
                if (playing) {
                    resumeOnGain = true;
                    Bridge.onMediaCommand(Bridge.CMD_PAUSE, 0);
                }
                break;
            case AudioManager.AUDIOFOCUS_LOSS:
                // Another player started.
                hasFocus = false;
                resumeOnGain = false;
                if (playing) Bridge.onMediaCommand(Bridge.CMD_PAUSE, 0);
                break;
            default:
                // Ducking: the system lowers the volume by itself.
                break;
        }
    }

    /** Headphones unplugged or Bluetooth gone: pause instead of playing out loud. */
    private final BroadcastReceiver noisy = new BroadcastReceiver() {
        @Override public void onReceive(Context c, Intent i) {
            if (playing) Bridge.onMediaCommand(Bridge.CMD_PAUSE, 0);
        }
    };

    private void listenForNoisy(boolean on) {
        if (on == noisyListening) return;
        noisyListening = on;
        if (on) ctx.registerReceiver(noisy, new IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY), Context.RECEIVER_NOT_EXPORTED);
        else ctx.unregisterReceiver(noisy);
    }

    private void publishMetadata() {
        MediaMetadata.Builder m = new MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, title)
                .putString(MediaMetadata.METADATA_KEY_ARTIST, artist)
                .putString(MediaMetadata.METADATA_KEY_ALBUM, album);
        if (durationMs > 0) m.putLong(MediaMetadata.METADATA_KEY_DURATION, durationMs);
        if (art != null) m.putBitmap(MediaMetadata.METADATA_KEY_ART, art);
        session.setMetadata(m.build());
    }

    private void publishState() {
        int state = !playing ? PlaybackState.STATE_PAUSED : moving ? PlaybackState.STATE_PLAYING : PlaybackState.STATE_BUFFERING;
        long actions = PlaybackState.ACTION_PLAY | PlaybackState.ACTION_PAUSE | PlaybackState.ACTION_PLAY_PAUSE
                | PlaybackState.ACTION_SKIP_TO_NEXT | PlaybackState.ACTION_SKIP_TO_PREVIOUS
                | PlaybackState.ACTION_SEEK_TO | PlaybackState.ACTION_STOP;
        session.setPlaybackState(new PlaybackState.Builder()
                .setActions(actions)
                .setState(state, positionMs, moving ? 1f : 0f, SystemClock.elapsedRealtime())
                .build());
    }

    private void loadCover(String source) {
        covers.execute(() -> {
            Bitmap b = null;
            try {
                if (source.startsWith("http://") || source.startsWith("https://")) {
                    HttpURLConnection conn = (HttpURLConnection) new URL(source).openConnection();
                    conn.setConnectTimeout(10_000);
                    conn.setReadTimeout(10_000);
                    try (InputStream in = conn.getInputStream()) {
                        b = BitmapFactory.decodeStream(in);
                    } finally {
                        conn.disconnect();
                    }
                } else {
                    b = BitmapFactory.decodeFile(source.startsWith("file://") ? source.substring(7) : source);
                }
            } catch (Exception e) {
                Log.w(TAG, "cover art: " + e);
            }
            Bitmap loaded = b;
            main.post(() -> {
                // Another song's art may have been asked for meanwhile.
                if (!source.equals(coverSource) || loaded == null) return;
                art = loaded;
                publishMetadata();
                PlaybackService.sync(ctx);
            });
        });
    }

    /** The app is closing: the system forgets the player. */
    void release() {
        listenForNoisy(false);
        if (hasFocus) audio.abandonAudioFocusRequest(focus);
        if (cpu.isHeld()) cpu.release();
        if (wifi.isHeld()) wifi.release();
        session.release();
    }
}
