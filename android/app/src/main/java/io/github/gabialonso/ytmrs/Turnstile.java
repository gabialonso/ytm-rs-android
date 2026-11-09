package io.github.gabialonso.ytmrs;

import android.app.Activity;
import android.app.Dialog;
import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.JavascriptInterface;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

import org.json.JSONObject;

/**
 * The Better Lyrics API's Cloudflare Turnstile challenge (src/android/turnstile.rs): its
 * own page in a WebView, off screen; its widget reports the token through the same
 * message the desktop build listens for. If Cloudflare wants an interaction, the page
 * shows in a dialog while the app is on screen. Main thread only.
 */
final class Turnstile {
    private static final String TAG = "ytm-rs";
    /** How long to wait for an invisible pass before showing the page. */
    private static final long SHOW_AFTER_MS = 5_000;
    private static final long GIVE_UP_AFTER_MS = 120_000;
    /** The challenge page posts to window.parent, which at top level is the page itself. */
    private static final String BRIDGE = "(function(){if(window.__ytmBridge)return;window.__ytmBridge=true;"
            + "window.addEventListener('message',function(e){if(e.data&&typeof e.data.type==='string'"
            + "&&e.data.type.indexOf('turnstile-')===0){ytmIpc.postMessage(JSON.stringify(e.data));}});})();";

    private static final Handler main = new Handler(Looper.getMainLooper());
    private static WebView web;
    private static Dialog dialog;
    private static boolean done = true;

    private Turnstile() {}

    static void start(Context app, String url) {
        if (!done) finish(false, "started again");
        done = false;
        web = new WebView(app);
        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        web.addJavascriptInterface(new Object() {
            @JavascriptInterface
            public void postMessage(String msg) {
                main.post(() -> onMessage(msg));
            }
        }, "ytmIpc");
        web.setWebViewClient(new WebViewClient() {
            @Override
            public void onPageStarted(WebView view, String u, android.graphics.Bitmap favicon) {
                view.evaluateJavascript(BRIDGE, null);
            }

            @Override
            public void onPageFinished(WebView view, String u) {
                view.evaluateJavascript(BRIDGE, null);
            }
        });
        // Off screen it still needs a size for the widget to lay out.
        float d = app.getResources().getDisplayMetrics().density;
        int w = (int) (340 * d), h = (int) (180 * d);
        web.measure(View.MeasureSpec.makeMeasureSpec(w, View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(h, View.MeasureSpec.EXACTLY));
        web.layout(0, 0, w, h);
        web.loadUrl(url);
        main.postDelayed(Turnstile::showIfStuck, SHOW_AFTER_MS);
        main.postDelayed(() -> finish(false, "timed out waiting for verification"), GIVE_UP_AFTER_MS);
    }

    private static void onMessage(String msg) {
        if (done) return;
        try {
            JSONObject v = new JSONObject(msg);
            switch (v.optString("type")) {
                case "turnstile-token": finish(true, v.optString("token")); break;
                case "turnstile-error": finish(false, "challenge error " + v.opt("error")); break;
                case "turnstile-timeout": finish(false, "challenge timed out"); break;
                // Expired before use: ask the widget for a fresh one.
                case "turnstile-expired": web.evaluateJavascript("turnstile.reset()", null); break;
                default: break;
            }
        } catch (Exception e) {
            Log.w(TAG, "turnstile message: " + e);
        }
    }

    /** Cloudflare wants an interaction: let the user see the widget, if the app is up. */
    private static void showIfStuck() {
        Activity a = Bridge.visibleActivity();
        if (done || a == null) return;
        dialog = new Dialog(a);
        dialog.setTitle("Verifying lyrics access");
        dialog.setContentView(web, new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, web.getHeight()));
        dialog.setOnCancelListener(x -> finish(false, "closed before verification finished"));
        dialog.show();
    }

    private static void finish(boolean ok, String value) {
        if (done) return;
        done = true;
        main.removeCallbacksAndMessages(null);
        Bridge.onTurnstileResult(ok, value);
        if (dialog != null) {
            dialog.setOnCancelListener(null);
            dialog.dismiss();
            dialog = null;
        }
        if (web != null) {
            web.destroy();
            web = null;
        }
    }
}
