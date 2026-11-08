package io.github.usuallygabo.ytmrs;

import android.app.Activity;
import android.graphics.Insets;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.view.WindowInsets;
import android.webkit.CookieManager;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.FrameLayout;
import android.window.OnBackInvokedDispatcher;

/**
 * Google's own sign-in page in a WebView. Once the flow lands on music.youtube.com with a
 * session (the SAPISID cookie), the youtube.com Cookie header goes back to Rust, as the
 * desktop sign-in window does (src/login.rs).
 */
public class LoginActivity extends Activity {
    static final String EXTRA_URL = "url";
    private static final String HOME = "https://music.youtube.com/";
    private static final String SESSION_COOKIE = "SAPISID";

    private final Handler main = new Handler(Looper.getMainLooper());
    private WebView web;
    private boolean reported;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        FrameLayout root = new FrameLayout(this);
        web = new WebView(this);
        root.addView(web);
        // Edge to edge: keep the page clear of the status bar, navigation bar and keyboard.
        root.setOnApplyWindowInsetsListener((v, insets) -> {
            Insets i = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.ime());
            v.setPadding(i.left, i.top, i.right, i.bottom);
            return WindowInsets.CONSUMED;
        });
        setContentView(root);

        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        CookieManager cookies = CookieManager.getInstance();
        cookies.setAcceptCookie(true);
        cookies.setAcceptThirdPartyCookies(web, true);
        web.setWebViewClient(new WebViewClient() {
            @Override
            public void onPageFinished(WebView view, String url) {
                if (url != null && url.startsWith(HOME)) check(0);
            }
        });
        getOnBackInvokedDispatcher().registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT, () -> {
            if (web.canGoBack()) web.goBack();
            else finish();
        });
        String url = getIntent().getStringExtra(EXTRA_URL);
        web.loadUrl(url != null ? url : HOME);
    }

    /** Landing on the site signed out also gets here: only a session has SAPISID. The
     *  cookie can be written a moment after the page loads, so look a few times. */
    private void check(int tries) {
        if (reported) return;
        String header = CookieManager.getInstance().getCookie(HOME);
        if (header != null && hasCookie(header, SESSION_COOKIE)) {
            report(Bridge.LOGIN_OK, header);
        } else if (tries < 20) {
            main.postDelayed(() -> check(tries + 1), 500);
        }
    }

    private static boolean hasCookie(String header, String name) {
        for (String part : header.split(";")) {
            if (part.trim().startsWith(name + "=")) return true;
        }
        return false;
    }

    private void report(int status, String value) {
        if (reported) return;
        reported = true;
        Bridge.onLoginResult(status, value);
        finish();
    }

    @Override
    protected void onDestroy() {
        report(Bridge.LOGIN_CANCELLED, "");
        main.removeCallbacksAndMessages(null);
        web.destroy();
        super.onDestroy();
    }
}
