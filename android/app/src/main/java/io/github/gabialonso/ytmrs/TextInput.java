package io.github.gabialonso.ytmrs;

import android.content.Context;
import android.text.InputType;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;

/**
 * An invisible view that takes the keyboard while an egui text field has focus, and
 * passes what's typed to Rust (src/android/text.rs). NativeActivity's own view isn't a
 * text editor, so keyboards would send it only plain keys.
 */
final class TextInput extends View {
    private final InputMethodManager imm;

    TextInput(Context c) {
        super(c);
        imm = c.getSystemService(InputMethodManager.class);
        setFocusable(true);
        setFocusableInTouchMode(true);
    }

    void show(boolean on) {
        if (on) {
            requestFocus();
            imm.restartInput(this);
            imm.showSoftInput(this, 0);
        } else {
            imm.hideSoftInputFromWindow(getWindowToken(), 0);
            clearFocus();
        }
    }

    @Override
    public boolean onCheckIsTextEditor() {
        return true;
    }

    @Override
    public InputConnection onCreateInputConnection(EditorInfo out) {
        // Plain text without suggestions: each letter is committed as typed (no
        // composing text, which egui's fields have no place for).
        out.inputType = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
                | InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD;
        out.imeOptions = EditorInfo.IME_ACTION_DONE | EditorInfo.IME_FLAG_NO_FULLSCREEN | EditorInfo.IME_FLAG_NO_EXTRACT_UI;
        return new BaseInputConnection(this, false) {
            @Override
            public boolean commitText(CharSequence text, int newCursorPosition) {
                if (text.length() > 0) Bridge.onText(text.toString());
                return true;
            }

            @Override
            public boolean setComposingText(CharSequence text, int newCursorPosition) {
                // A keyboard that composes anyway: take the text as typed.
                return commitText(text, newCursorPosition);
            }

            @Override
            public boolean deleteSurroundingText(int before, int after) {
                for (int i = 0; i < before; i++) Bridge.onKey(Bridge.KEY_BACKSPACE);
                for (int i = 0; i < after; i++) Bridge.onKey(Bridge.KEY_DELETE);
                return true;
            }

            @Override
            public boolean sendKeyEvent(KeyEvent e) {
                if (e.getAction() == KeyEvent.ACTION_DOWN) {
                    switch (e.getKeyCode()) {
                        case KeyEvent.KEYCODE_DEL: Bridge.onKey(Bridge.KEY_BACKSPACE); break;
                        case KeyEvent.KEYCODE_FORWARD_DEL: Bridge.onKey(Bridge.KEY_DELETE); break;
                        case KeyEvent.KEYCODE_ENTER: Bridge.onKey(Bridge.KEY_ENTER); break;
                        default:
                            int ch = e.getUnicodeChar(e.getMetaState());
                            if (ch > 0) Bridge.onText(new String(Character.toChars(ch)));
                    }
                }
                return true;
            }

            @Override
            public boolean performEditorAction(int action) {
                Bridge.onKey(Bridge.KEY_ENTER);
                return true;
            }
        };
    }
}
