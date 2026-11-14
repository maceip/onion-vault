package dev.onionvault

import android.os.Bundle
import android.util.Log
import android.view.Choreographer
import android.widget.Toast
import androidx.activity.compose.setContent
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import dev.onionvault.ui.BackPreviewState
import dev.onionvault.ui.BrowserScreen
import dev.onionvault.ui.BrowserUiState
import dev.onionvault.ui.CircuitHop
import dev.onionvault.ui.CircuitState
import dev.onionvault.ui.KeyInfo
import dev.onionvault.ui.OnionBackStackManager
import dev.onionvault.ui.PgpFieldState
import dev.onionvault.ui.PgpOverlayState
import dev.onionvault.ui.PgpPlaintextState
import dev.onionvault.ui.TorBootstrapState
import dev.onionvault.ui.parseFields
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject

/**
 * The user interface of PlugTalk.
 *
 * The activity owns
 *
 *   * the off-screen rendered page (a [OvSurfaceView] inside the Compose
 *     hierarchy),
 *   * the message pump of CEF (a [Choreographer] callback), and
 *   * the state of the screen, which is a pure value that the Compose
 *     screen renders.
 *
 * The back stack of the browser keeps the rendered frame and the state of
 * every visited page, so that the predictive back gesture does not have to
 * wait for the hidden service.
 */
class MainActivity : FragmentActivity(), OvBridge.Listener {

    private companion object {
        const val TAG = "PlugTalk"
        const val STATE_URL = "url"
    }

    /** The off-screen rendered page. */
    private lateinit var surface: OvSurfaceView

    /** The back stack of the browser. */
    private val backStack = OnionBackStackManager()

    /** The state of the screen. */
    private var uiState by mutableStateOf(BrowserUiState())

    /** The message pump of CEF. */
    private val frameCallback: Choreographer.FrameCallback = Choreographer.FrameCallback {
        OvNative.nativeDoMessageLoopWork()
        if (!isFinishing) {
            Choreographer.getInstance().postFrameCallback(frameCallback)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        OvBridge.setListener(this)

        // The CEF resources (pak files, ICU data, locales) are unpacked from
        // the assets into the private directory of the application.
        CefResources.unpack(this)

        val started = OvNative.nativeInit(
            filesDir.absolutePath,
            cacheDir.absolutePath,
            hiddenServiceOnlyMode(),
            requireUserAuthentication(),
        )
        if (!started) {
            Toast.makeText(this, R.string.engine_failed, Toast.LENGTH_LONG).show()
            return
        }

        setContent {
            // The licences that travel with the application are shown by the
            // button of the licences.  They are measured on a worker thread:
            // the notice of the engine is 2.2 MiB long.
            LaunchedEffect(Unit) {
                val documents = withContext(Dispatchers.IO) {
                    LegalNotices.list(this@MainActivity)
                }
                uiState = uiState.copy(legalDocuments = documents)
            }

            BrowserScreen(
                state = uiState,
                onNavigate = { navigate(it) },
                onFieldAction = { onFieldAction(it) },
                onInsertPlaintext = { insertPlaintext(it) },
                onReloadLive = { reloadLive() },
                onBack = { goBack() },
                onForward = { goForward() },
                onReload = { reloadLive() },
                onNewTab = { newTab() },
                onCreateKey = { createKey() },
                onExportKey = { exportKey(it) },
                onLoadLegalLines = { name -> LegalNotices.read(this, name) },
                surface = {
                    AndroidView(
                        factory = { context ->
                            OvSurfaceView(context).also { view -> surface = view }
                        },
                        modifier = Modifier.fillMaxSize(),
                    )
                },
            )
        }

        // The keys of the keyring are shown by the key manager.
        refreshKeys()

        // The first page: the start page of the application.
        val start = savedInstanceState?.getString(STATE_URL) ?: getString(R.string.start_url)
        navigate(start)
    }

    override fun onResume() {
        super.onResume()
        Choreographer.getInstance().postFrameCallback(frameCallback)
    }

    override fun onPause() {
        Choreographer.getInstance().removeFrameCallback(frameCallback)
        super.onPause()
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putString(STATE_URL, uiState.url)
    }

    override fun onDestroy() {
        OvBridge.setListener(null)
        OvNative.nativeShutdown()
        super.onDestroy()
    }

    /** Navigate to `url`. */
    private fun navigate(url: String) {
        val target = url.trim()
        if (target.isEmpty()) {
            return
        }
        // A bare host name is a hidden service.
        val normalized = if (target.contains("://")) target else "http://$target"
        OvNative.nativeLoadUrl(normalized)
    }

    /** Fetch the page again over Tor. */
    private fun reloadLive() {
        val url = uiState.url
        if (url.isNotEmpty()) {
            OvNative.nativeLoadUrl(url)
        }
    }

    /**
     * Go back to the previous page.
     *
     * The frame of the previous page is already on the screen (the gesture
     * showed it), so the latency of the hidden service is hidden behind the
     * preview.
     */
    private fun goBack() {
        OvNative.nativeGoBack()
        backStack.refresh()
        uiState = uiState.copy(
            canGoBack = backStack.canGoBack,
            backPreview = backStack.previewState(),
        )
    }

    /** Go forward to the next page of the history. */
    private fun goForward() {
        if (OvNative.nativeGoForward()) {
            backStack.refresh()
            uiState = uiState.copy(
                canGoBack = backStack.canGoBack,
                backPreview = backStack.previewState(),
            )
        }
    }

    /** Open the start page in a new tab. */
    private fun newTab() {
        navigate(getString(R.string.start_url))
    }

    /**
     * Create a new key pair in the keyring.
     *
     * The user id carries a short suffix, so that two keys of the same
     * device can be told apart.
     */
    private fun createKey() {
        if (requireUserAuthentication()) {
            confirmWithBiometrics(getString(R.string.authenticate_for_key)) {
                generateKey()
            }
            return
        }
        generateKey()
    }

    /** Generate the key and tell the user its fingerprint. */
    private fun generateKey() {
        val suffix = java.util.UUID.randomUUID().toString().take(8)
        val fingerprint = OvNative.nativeGenerateKey("PlugTalk <$suffix@example.onion>")
        if (fingerprint.isNullOrEmpty()) {
            showError(getString(R.string.key_failed))
            return
        }
        refreshKeys()
        Toast.makeText(
            this,
            getString(R.string.key_created, fingerprint.takeLast(16)),
            Toast.LENGTH_LONG,
        ).show()
    }

    /** Copy the public key of a certificate into the clipboard. */
    private fun exportKey(fingerprint: String) {
        val armored = OvNative.nativeExportPublicKey(fingerprint)
        if (armored.isNullOrEmpty()) {
            showError(getString(R.string.key_failed))
            return
        }
        val clipboard = getSystemService(CLIPBOARD_SERVICE) as android.content.ClipboardManager
        clipboard.setPrimaryClip(
            android.content.ClipData.newPlainText("PlugTalk public key", armored),
        )
        Toast.makeText(this, getString(R.string.key_copied), Toast.LENGTH_SHORT).show()
    }

    /** Read the keys of the keyring into the state of the screen. */
    private fun refreshKeys() {
        val json = OvNative.nativeListKeys() ?: return
        val keys = try {
            val array = JSONObject(json).optJSONArray("keys") ?: return
            (0 until array.length()).mapNotNull { index ->
                val entry = array.optJSONObject(index) ?: return@mapNotNull null
                KeyInfo(
                    fingerprint = entry.optString("fingerprint", ""),
                    userId = entry.optString("userId", ""),
                    hasSecret = entry.optBoolean("hasSecret", false),
                )
            }
        } catch (error: Throwable) {
            Log.w(TAG, "cannot read the key list", error)
            return
        }
        uiState = uiState.copy(keys = keys)
    }

    /** The user tapped a PGP action chip. */
    private fun onFieldAction(field: PgpFieldState) {
        // The complete armored text is needed to decrypt a message or to
        // import a key; the engine sends it with the field.
        val armored = field.armored.orEmpty()
        when (field.kind) {
            "pgp_message" -> {
                if (armored.isEmpty()) {
                    showError(getString(R.string.no_message))
                    return
                }
                if (requireUserAuthentication()) {
                    confirmWithBiometrics(getString(R.string.authenticate_for_key)) {
                        OvNative.nativeDecryptNode(field.nodeId, armored)
                    }
                    return
                }
                OvNative.nativeDecryptNode(field.nodeId, armored)
            }

            "pgp_public_key" -> {
                if (armored.isEmpty()) {
                    showError(getString(R.string.no_key))
                    return
                }
                OvNative.nativeImportKey(armored)
            }

            "textarea", "text_input" -> {
                // The recipients are the own keys of the keyring; the native
                // side fills them in.
                if (requireUserAuthentication()) {
                    confirmWithBiometrics(getString(R.string.authenticate_for_key)) {
                        OvNative.nativeEncryptField(field.nodeId, emptyArray())
                    }
                    return
                }
                OvNative.nativeEncryptField(field.nodeId, emptyArray())
            }

            else -> showError(getString(R.string.unknown_field))
        }
    }

    /** Write the plaintext of a decrypted message into the page. */
    private fun insertPlaintext(nodeId: String) {
        val plaintext = uiState.pgp.plaintext ?: return
        OvNative.nativeInsertPlaintext(nodeId, plaintext.body)
        uiState = uiState.copy(pgp = uiState.pgp.copy(plaintext = null))
    }

    // --- the callbacks of the native engine --------------------------------

    override fun onDomScan(json: String) {
        val report = try {
            JSONObject(json)
        } catch (error: Throwable) {
            Log.w(TAG, "cannot read the DOM scan report", error)
            return
        }
        val width = if (::surface.isInitialized) surface.width else 0
        val height = if (::surface.isInitialized) surface.height else 0
        val fields = parseFields(report.optJSONArray("fields"), width, height)
        uiState = uiState.copy(pgp = uiState.pgp.copy(fields = fields))
    }

    override fun onNavigation(json: String) {
        val info = try {
            JSONObject(json)
        } catch (error: Throwable) {
            Log.w(TAG, "cannot read the navigation report", error)
            return
        }
        val url = info.optString("url", "")
        uiState = uiState.copy(
            url = url.ifEmpty { uiState.url },
            title = info.optString("title", uiState.title),
            blocked = info.optBoolean("blocked", false),
            status = if (info.optBoolean("blocked", false)) {
                getString(R.string.blocked)
            } else {
                uiState.status
            },
        )
    }

    override fun showPlaintext(nodeId: String, title: String, body: String, verified: Boolean) {
        uiState = uiState.copy(
            pgp = uiState.pgp.copy(
                plaintext = PgpPlaintextState(
                    nodeId = nodeId,
                    title = title,
                    body = body,
                    signatureVerified = verified,
                ),
            ),
        )
    }

    override fun onMutationResult(json: String) {
        val outcome = try {
            JSONObject(json)
        } catch (error: Throwable) {
            Log.w(TAG, "cannot read the mutation result", error)
            return
        }
        val applied = outcome.optBoolean("applied", false)
        val reason = outcome.optString("reason", "")
        val message = if (applied) {
            getString(R.string.mutation_applied)
        } else {
            val detail = reason.ifEmpty { getString(R.string.mutation_stale) }
            getString(R.string.mutation_failed, detail)
        }
        Toast.makeText(this, message, Toast.LENGTH_LONG).show()
    }

    override fun showError(message: String) {
        Toast.makeText(this, message, Toast.LENGTH_LONG).show()
    }

    override fun onTorProgress(percent: Int, stage: String) {
        uiState = uiState.copy(
            tor = uiState.tor.copy(
                percent = percent,
                stage = stage,
                ready = percent >= 100,
                circuit = uiState.tor.circuit ?: defaultCircuit(),
            ),
        )
    }

    override fun onPageSnapshot(url: String) {
        backStack.refresh()
        uiState = uiState.copy(
            canGoBack = backStack.canGoBack,
            backPreview = backStack.previewState(),
        )
    }

    /** The circuit that is shown while the client bootstraps. */
    private fun defaultCircuit(): CircuitState = CircuitState(
        hops = listOf(
            CircuitHop("Guard", 0),
            CircuitHop("Middle", 0),
            CircuitHop("Exit", 0),
            CircuitHop("Rendezvous", 0),
        ),
    )

    /** `true` when the browser must refuse every clearnet request. */
    private fun hiddenServiceOnlyMode(): Boolean =
        getPreferences(MODE_PRIVATE).getBoolean("hidden_service_only", true)

    /** `true` when the master key demands a biometric confirmation. */
    private fun requireUserAuthentication(): Boolean =
        getPreferences(MODE_PRIVATE).getBoolean("require_user_authentication", false)

    /** Show the biometric prompt for the hardware confirmation. */
    private fun confirmWithBiometrics(title: String, onConfirmed: () -> Unit) {
        val executor = ContextCompat.getMainExecutor(this)
        val prompt = BiometricPrompt(
            this,
            executor,
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                    onConfirmed()
                }

                override fun onAuthenticationError(code: Int, message: CharSequence) {
                    Log.w(TAG, "the confirmation failed: $message")
                    showError(getString(R.string.authentication_failed))
                }
            },
        )
        val info = BiometricPrompt.PromptInfo.Builder()
            .setTitle(title)
            .setAllowedAuthenticators(
                BiometricManager.Authenticators.BIOMETRIC_STRONG or
                    BiometricManager.Authenticators.DEVICE_CREDENTIAL,
            )
            .build()
        prompt.authenticate(info)
    }
}
