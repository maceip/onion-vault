package dev.onionvault

import android.util.Log

/**
 * The callbacks of the native engine.
 *
 * The Rust side calls these static methods through JNI; they are dispatched
 * to the activity that is currently in the foreground.
 */
object OvBridge {

    private const val TAG = "OvBridge"

    /** The UI that receives the callbacks. */
    interface Listener {
        /** The render process found new DOM fields (JSON encoded report). */
        fun onDomScan(json: String)

        /** The browser navigated to a new document (JSON encoded). */
        fun onNavigation(json: String)

        /** A decrypted message has to be shown in the native sheet. */
        fun showPlaintext(nodeId: String, title: String, body: String, verified: Boolean)

        /**
         * The engine changed a field of the page (JSON encoded result).
         *
         * The result tells whether the page carries the new text; a field
         * that still holds the old text would send it.
         */
        fun onMutationResult(json: String)

        /** An error has to be shown to the user. */
        fun showError(message: String)

        /** The progress of the Tor bootstrap changed. */
        fun onTorProgress(percent: Int, stage: String)

        /**
         * A page was left and its snapshot is complete.
         *
         * The UI pulls the snapshot from the engine; the callback only tells
         * it that the back stack changed.
         */
        fun onPageSnapshot(url: String)
    }

    @Volatile
    private var listener: Listener? = null

    /** Register the UI that receives the callbacks. */
    fun setListener(value: Listener?) {
        listener = value
    }

    @JvmStatic
    fun onDomScan(json: String) {
        dispatch { it.onDomScan(json) }
    }

    @JvmStatic
    fun onNavigation(json: String) {
        dispatch { it.onNavigation(json) }
    }

    @JvmStatic
    fun showPlaintext(nodeId: String, title: String, body: String, verified: Boolean) {
        dispatch { it.showPlaintext(nodeId, title, body, verified) }
    }

    @JvmStatic
    fun onMutationResult(json: String) {
        dispatch { it.onMutationResult(json) }
    }

    @JvmStatic
    fun showError(message: String) {
        Log.w(TAG, message)
        dispatch { it.showError(message) }
    }

    @JvmStatic
    fun onTorProgress(percent: Int, stage: String) {
        dispatch { it.onTorProgress(percent, stage) }
    }

    @JvmStatic
    fun onPageSnapshot(url: String) {
        dispatch { it.onPageSnapshot(url) }
    }

    /** Run the callback on the UI thread. */
    private fun dispatch(action: (Listener) -> Unit) {
        val target = listener ?: return
        android.os.Handler(android.os.Looper.getMainLooper()).post {
            try {
                action(target)
            } catch (error: Throwable) {
                Log.e(TAG, "the native callback failed", error)
            }
        }
    }
}
