package io.github.usuallygabo.ytmrs;

import android.app.NativeActivity;
import android.app.NotificationManager;
import android.content.Intent;
import android.graphics.Insets;
import android.os.Bundle;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.window.OnBackInvokedDispatcher;

/**
 * The egui app. NativeActivity loads libytm_rs.so and runs its {@code android_main} on a
 * thread of its own; this subclass tells Rust when the app is on screen and keeps the
 * player alive across Back.
 */
public class MainActivity extends NativeActivity {
    static {
        // Loaded here too (not only by NativeActivity's dlopen) so the JVM finds the
        // Bridge's native methods.
        System.loadLibrary("ytm_rs");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        Bridge.attach(this);
        super.onCreate(savedInstanceState);
        // Back leaves the app as Home does: finishing the activity would end the player.
        TextInput text = new TextInput(this);
        addContentView(text, new ViewGroup.LayoutParams(1, 1));
        Bridge.setTextInput(text);
        // The safe area: system bars, the camera cutout, and the keyboard when it's up.
        getWindow().getDecorView().setOnApplyWindowInsetsListener((v, insets) -> {
            Insets i = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout() | WindowInsets.Type.ime());
            Bridge.onInsets(i.left, i.top, i.right, i.bottom);
            return v.onApplyWindowInsets(insets);
        });
        getOnBackInvokedDispatcher().registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT, () -> moveTaskToBack(true));
    }

    @Override
    protected void onStart() {
        super.onStart();
        Bridge.onVisible(true);
    }

    @Override
    protected void onStop() {
        Bridge.onVisible(false);
        super.onStop();
    }

    @Override
    protected void onDestroy() {
        Bridge.detach(this);
        super.onDestroy();
        // The app was closed (swiped away from Recents). The Rust side can't start over in
        // this process (winit makes one event loop per process), so end it, player and all.
        Player.get(this).release();
        stopService(new Intent(this, PlaybackService.class));
        getSystemService(NotificationManager.class).cancelAll();
        android.os.Process.killProcess(android.os.Process.myPid());
    }
}
