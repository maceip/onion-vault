package dev.onionvault

import android.view.Surface

/**
 * The JNI bridge to the native engine.
 *
 * Every function is implemented in `crates/ov-core/src/lib.rs`; the names of
 * the methods are part of the ABI, the `Java_dev_onionvault_OvNative_*`
 * symbols of the native library.
 */
object OvNative {

    /** Start the engine: Tor, the key manager and the off-screen browser. */
    external fun nativeInit(
        filesDir: String,
        cacheDir: String,
        strict: Boolean,
        requireAuth: Boolean,
    ): Boolean

    /** Stop the engine. */
    external fun nativeShutdown()

    /** Attach the surface of the [OvSurfaceView] to the renderer. */
    external fun nativeCreateSurface(surface: Surface, width: Int, height: Int)

    /** Detach the surface. */
    external fun nativeDestroySurface()

    /** The surface was resized. */
    external fun nativeResize(width: Int, height: Int)

    /**
     * Run one iteration of the CEF message loop.
     *
     * CEF runs with an external message pump on Android: the platform does
     * not have a windowing system that CEF could use, so the native UI drives
     * the loop from the [android.view.Choreographer].
     */
    external fun nativeDoMessageLoopWork()

    /** Load a URL. */
    external fun nativeLoadUrl(url: String)

    /** Forward a touch event. */
    external fun nativeTouch(action: Int, x: Float, y: Float, id: Int, modifiers: Int)

    /** Forward a mouse button event. */
    external fun nativeMouseButton(
        x: Float,
        y: Float,
        button: Int,
        mouseUp: Int,
        modifiers: Int,
        clickCount: Int,
    )

    /** Forward a mouse move event. */
    external fun nativeMouseMove(x: Float, y: Float, modifiers: Int)

    /** Forward a scroll event. */
    external fun nativeMouseWheel(
        x: Float,
        y: Float,
        deltaX: Float,
        deltaY: Float,
        modifiers: Int,
    )

    /** Forward a key event (0 = down, 1 = up, 2 = character). */
    external fun nativeKey(
        action: Int,
        keyCode: Int,
        character: Int,
        modifiers: Int,
        isSystemKey: Int,
    )

    /** Commit the text of the soft keyboard. */
    external fun nativeImeCommitText(text: String)

    /** Write a decrypted plaintext into a field of the page. */
    external fun nativeInsertPlaintext(nodeId: String, text: String)

    /** Encrypt the content of a field. */
    external fun nativeEncryptField(nodeId: String, recipients: Array<String>)

    /** Insert the armored public key of the local user into a field. */
    external fun nativeInsertPublicKey(nodeId: String)

    /** Decrypt the armored message of a node. */
    external fun nativeDecryptNode(nodeId: String, armored: String)

    /** Import an armored public key that was found in the page. */
    external fun nativeImportKey(armored: String)

    /** Generate a new key pair and return its fingerprint. */
    external fun nativeGenerateKey(userId: String): String?

    /** Export the armored public key of a fingerprint. */
    external fun nativeExportPublicKey(fingerprint: String): String?

    /** The number of snapshots of the back stack. */
    external fun nativeSnapshotCount(): Int

    /** The metadata of a snapshot of the back stack (JSON encoded). */
    external fun nativeSnapshotInfo(index: Int): String?

    /** The pixels of the frame of a snapshot, in the R, G, B, A order. */
    external fun nativeSnapshotFrame(index: Int): ByteArray?

    /** The markup of a snapshot of the back stack. */
    external fun nativeSnapshotMarkup(index: Int): ByteArray?

    /** Go back in the history of the browser. */
    external fun nativeGoBack(): Boolean

    /**
     * Run a CEF subprocess.
     *
     * The subprocesses are started by [CefService]; the command line is
     * passed as a list of switches.
     */
    external fun nativeRunSubprocess(processType: String, switches: Array<String>): Int
}
