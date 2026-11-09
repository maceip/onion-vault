package dev.onionvault

import android.content.Context
import android.util.AttributeSet
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputConnectionWrapper
import android.view.SurfaceView

/**
 * The surface that shows the off-screen rendered page.
 *
 * CEF paints into a memory buffer; the native code copies that buffer into
 * the native window of this surface.  The view also forwards the touch and
 * the key events to CEF and bridges the soft keyboard.
 */
class OvSurfaceView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : SurfaceView(context, attrs) {

    /** The modifiers of the last event, in the CEF encoding. */
    private var modifiers: Int = 0

    init {
        isFocusable = true
        isFocusableInTouchMode = true
        holder.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {
                attachSurface()
            }

            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                attachSurface()
                OvNative.nativeResize(width, height)
            }

            override fun surfaceDestroyed(holder: SurfaceHolder) {
                OvNative.nativeDestroySurface()
            }
        })
    }

    /** Attach the surface to the native renderer. */
    private fun attachSurface() {
        if (!holder.surface.isValid) {
            return
        }
        OvNative.nativeCreateSurface(holder.surface, width, height)
    }

    /** The CEF modifiers of an Android meta state. */
    private fun modifiersOf(metaState: Int): Int {
        var result = 0
        if (metaState and KeyEvent.META_SHIFT_ON != 0) result = result or 1
        if (metaState and KeyEvent.META_CTRL_ON != 0) result = result or 2
        if (metaState and KeyEvent.META_ALT_ON != 0) result = result or 4
        if (metaState and KeyEvent.META_META_ON != 0) result = result or 8
        return result
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        modifiers = modifiersOf(event.metaState)
        val action = event.actionMasked

        when (action) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                val index = event.actionIndex
                OvNative.nativeTouch(
                    action,
                    event.getX(index),
                    event.getY(index),
                    event.getPointerId(index),
                    modifiers,
                )
            }

            MotionEvent.ACTION_MOVE -> {
                // CEF's touch event carries a single point; the primary
                // pointer is the one that scrolls the page.
                OvNative.nativeTouch(
                    action,
                    event.x,
                    event.y,
                    event.getPointerId(0),
                    modifiers,
                )
            }

            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                val index = event.actionIndex
                OvNative.nativeTouch(
                    action,
                    event.getX(index),
                    event.getY(index),
                    event.getPointerId(index),
                    modifiers,
                )
            }

            MotionEvent.ACTION_CANCEL -> {
                OvNative.nativeTouch(action, event.x, event.y, 0, modifiers)
            }

            else -> return super.onTouchEvent(event)
        }
        return true
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        modifiers = modifiersOf(event.metaState)
        OvNative.nativeKey(0, keyCode, event.unicodeChar, modifiers, 0)
        return true
    }

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean {
        modifiers = modifiersOf(event.metaState)
        OvNative.nativeKey(1, keyCode, event.unicodeChar, modifiers, 0)
        return true
    }

    override fun onKeyMultiple(keyCode: Int, repeatCount: Int, event: KeyEvent): Boolean {
        modifiers = modifiersOf(event.metaState)
        repeat(repeatCount) {
            OvNative.nativeKey(2, keyCode, event.unicodeChar, modifiers, 0)
        }
        return true
    }

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection? {
        val connection = super.onCreateInputConnection(outAttrs) ?: return null
        // The page is rendered by CEF, so the text of the soft keyboard has
        // to be forwarded to the browser instead of the focused view.
        return CefInputConnection(connection)
    }

    /** Forwards the text of the soft keyboard to CEF. */
    private class CefInputConnection(target: InputConnection) :
        InputConnectionWrapper(target, true) {

        override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
            OvNative.nativeImeCommitText(text?.toString() ?: "")
            return true
        }

        override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean {
            OvNative.nativeImeCommitText(text?.toString() ?: "")
            return true
        }

        override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
            OvNative.nativeKey(1, KeyEvent.KEYCODE_DEL, 0, 0, 0)
            return true
        }
    }
}
