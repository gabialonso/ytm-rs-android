package io.github.usuallygabo.ytmrs;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.os.Handler;
import android.os.Looper;
import android.webkit.CookieManager;
import android.webkit.WebStorage;

import java.lang.ref.WeakReference;

/**
 * The JNI bridge. Rust calls the static methods below from its own threads (see
 * src/android/mod.rs); the native methods are implemented in Rust.
 */
public final class Bridge {
    private static Context app;
    private static WeakReference<Activity> activity = new WeakReference<>(null);
    private static final Handler main = new Handler(Looper.getMainLooper());
    private static TextInput textInput;
    private static boolean visible;

    private Bridge() {}

    static void attach(Activity a) {
        app = a.getApplicationContext();
        activity = new WeakReference<>(a);
        Player.get(app);
    }

    static void detach(Activity a) {
        if (activity.get() == a) activity = new WeakReference<>(null);
    }

    /** The app's activity while it's on screen, else null. */
    static Activity visibleActivity() {
        return visible ? activity.get() : null;
    }

    static void setVisible(boolean v) {
        visible = v;
        onVisible(v);
    }

    static void setTextInput(TextInput t) {
        textInput = t;
    }

    // ------------------------------------------------------------ Rust → Java

    /** Show Google's sign-in page; the result comes back through onLoginResult. */
    public static void startLogin(String url) {
        main.post(() -> {
            Activity a = activity.get();
            Context c = a != null ? a : app;
            Intent i = new Intent(c, LoginActivity.class).putExtra(LoginActivity.EXTRA_URL, url);
            if (a == null) i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            c.startActivity(i);
        });
    }

    /** Forget the WebView's session (sign-out, and right after a sign-in). */
    public static void clearCookies() {
        main.post(() -> {
            CookieManager.getInstance().removeAllCookies(null);
            CookieManager.getInstance().flush();
            WebStorage.getInstance().deleteAllData();
        });
    }

    /** The song for the lock screen and notification. {@code cover}: a URL or a file path. */
    public static void setTrack(String title, String artist, String album, String cover, long durationMs) {
        main.post(() -> Player.get(app).setTrack(title, artist, album, cover, durationMs));
    }

    /**
     * {@code playing}: playing or about to (buffering); {@code moving}: sound is coming out,
     * so the position advances from {@code positionMs}.
     */
    public static void setState(boolean playing, boolean moving, long positionMs) {
        main.post(() -> Player.get(app).setState(playing, moving, positionMs));
    }

    /** Run the lyrics API's challenge page; the token comes back through onTurnstileResult. */
    public static void startTurnstile(String url) {
        main.post(() -> Turnstile.start(app, url));
    }

    /** Put text on the clipboard (Android says so itself). */
    public static void copyText(String text) {
        main.post(() -> app.getSystemService(android.content.ClipboardManager.class)
                .setPrimaryClip(android.content.ClipData.newPlainText("ytm-rs", text)));
    }

    /** Show the keyboard (an egui text field has focus) or hide it. */
    public static void setKeyboard(boolean show) {
        main.post(() -> {
            if (textInput != null) textInput.show(show);
        });
    }

    // ------------------------------------------------------------ Java → Rust

    static final int LOGIN_OK = 0, LOGIN_CANCELLED = 1, LOGIN_FAILED = 2;

    static native void onLoginResult(int status, String value);

    static final int CMD_PLAY = 0, CMD_PAUSE = 1, CMD_TOGGLE = 2, CMD_NEXT = 3, CMD_PREVIOUS = 4,
            CMD_STOP = 5, CMD_SEEK = 6;

    static native void onMediaCommand(int cmd, long arg);

    static native void onVisible(boolean visible);

    /** The system's Back, in the app; false when there's nowhere back to go. */
    static native boolean onBack();

    /** Stop playback before the process is killed (swiped from Recents). */
    static native void onClosing();

    static final int KEY_BACKSPACE = 0, KEY_ENTER = 1, KEY_DELETE = 2;

    static native void onTurnstileResult(boolean ok, String value);

    static native void onText(String text);

    static native void onKeyboardHidden();

    static native void onKey(int key);

    /** The safe area's margins, in pixels. */
    static native void onInsets(int left, int top, int right, int bottom);
}
