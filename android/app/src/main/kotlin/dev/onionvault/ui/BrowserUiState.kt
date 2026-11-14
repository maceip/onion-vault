package dev.onionvault.ui

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.unit.IntOffset

/**
 * The state of the browser screen.
 *
 * The state is a pure value: it never holds a CEF or a JNI pointer, so that
 * every screen can be rendered in a `@Preview` and in the screenshot tests
 * of the continuous integration.
 */
@Immutable
data class BrowserUiState(
    /** The address of the page that is shown. */
    val url: String = "",
    /** The title of the page that is shown. */
    val title: String = "",
    /** The progress of the Tor bootstrap. */
    val tor: TorBootstrapState = TorBootstrapState(),
    /** The PGP overlays of the page. */
    val pgp: PgpOverlayState = PgpOverlayState(),
    /** The message of the status line. */
    val status: String = "",
    /** `true` when the browser can go back. */
    val canGoBack: Boolean = false,
    /** The page that is shown behind the live page during the back gesture. */
    val backPreview: BackPreviewState? = null,
    /** `true` when the last request was blocked by the leak protection. */
    val blocked: Boolean = false,
    /** The keys of the keyring. */
    val keys: List<KeyInfo> = emptyList(),
    /** The licence documents that travel with the application. */
    val legalDocuments: List<LegalDocument> = emptyList(),
    /** `true` when the browser refuses every clearnet request. */
    val hiddenServiceOnly: Boolean = true,
    /** The zoom level of the page; `0.0` is the default size. */
    val zoomLevel: Float = 0f,
    /** The number of the tabs of the browser. */
    val tabCount: Int = 1,
)

/**
 * A licence document that is packaged into the application.
 *
 * @property name the file name of the document in the assets.
 * @property title the name that is shown to the user.
 * @property sizeBytes the size of the document in bytes.
 */
@Immutable
data class LegalDocument(
    val name: String,
    val title: String,
    val sizeBytes: Long,
)

/**
 * A key of the keyring.
 *
 * @property fingerprint the fingerprint of the certificate.
 * @property userId the user id of the certificate.
 * @property hasSecret `true` when a secret key is stored for it.
 */
@Immutable
data class KeyInfo(
    val fingerprint: String,
    val userId: String,
    val hasSecret: Boolean,
)

/**
 * The state of the Tor client.
 *
 * @property percent the progress of the bootstrap in percent.
 * @property stage the stage of the bootstrap.
 * @property ready `true` when the client can carry traffic.
 * @property circuit the circuit that the traffic uses.
 * @property isolationToken the token of the circuit of the current page.
 */
@Immutable
data class TorBootstrapState(
    val percent: Int = 0,
    val stage: String = "",
    val ready: Boolean = false,
    val circuit: CircuitState? = null,
    val isolationToken: String? = null,
)

/**
 * The circuit that carries the traffic of the browser.
 *
 * @property hops the hops of the circuit, from the guard to the rendezvous.
 */
@Immutable
data class CircuitState(
    val hops: List<CircuitHop> = emptyList(),
)

/**
 * One hop of a Tor circuit.
 *
 * @property role the role of the hop.
 * @property latencyMillis the round trip time to the hop.
 */
@Immutable
data class CircuitHop(
    val role: String,
    val latencyMillis: Int,
)

/**
 * The PGP overlays of the page.
 *
 * @property fields the detected fields of the page.
 * @property plaintext the decrypted message that is shown in the sheet.
 */
@Immutable
data class PgpOverlayState(
    val fields: List<PgpFieldState> = emptyList(),
    val plaintext: PgpPlaintextState? = null,
    val error: String? = null,
)

/**
 * A field of the page that carries PGP data.
 *
 * @property nodeId the synthetic id of the node.
 * @property kind the role of the node (`textarea`, `text_input`, ...).
 * @property label the label of the action chip.
 * @property bounds the bounds of the node in the normalized page space.
 * @property preview a short preview of the content of the node.
 * @property armored the complete armored text of a PGP block.
 */
@Immutable
data class PgpFieldState(
    val nodeId: String,
    val kind: String,
    val label: String,
    val bounds: NormalizedBounds,
    val preview: String = "",
    val armored: String? = null,
)

/**
 * The bounds of a node in the normalized page space.
 *
 * The values are between zero and one, so that the overlay follows the
 * layout of the page in every preview and on every screen.
 */
@Immutable
data class NormalizedBounds(
    val left: Float,
    val top: Float,
    val right: Float,
    val bottom: Float,
) {
    /** The width of the bounds. */
    val width: Float get() = right - left

    /** The height of the bounds. */
    val height: Float get() = bottom - top
}

/**
 * The decrypted message that is shown in the native sheet.
 *
 * @property nodeId the node the message came from.
 * @property title the title of the sheet.
 * @property body the plaintext of the message.
 * @property signatureVerified `true` when the signature was verified.
 * @property signerFingerprint the fingerprint of the signer.
 */
@Immutable
data class PgpPlaintextState(
    val nodeId: String,
    val title: String,
    val body: String,
    val signatureVerified: Boolean,
    val signerFingerprint: String? = null,
)

/**
 * The page that is shown behind the live page during the back gesture.
 *
 * @property url the address of the page.
 * @property title the title of the page.
 * @property scrollOffset the scroll offset of the page.
 * @property fields the PGP fields of the page.
 * @property frameSnapshot the last frame of the page.
 */
@Immutable
data class BackPreviewState(
    val url: String,
    val title: String,
    val scrollOffset: IntOffset = IntOffset.Zero,
    val fields: List<PgpFieldState> = emptyList(),
    val frameSnapshot: ImageBitmap? = null,
)
