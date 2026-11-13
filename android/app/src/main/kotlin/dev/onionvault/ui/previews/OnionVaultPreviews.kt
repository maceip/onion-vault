package dev.onionvault.ui.previews

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.onionvault.ui.BrowserScreen
import dev.onionvault.ui.BrowserUiState
import dev.onionvault.ui.CircuitHop
import dev.onionvault.ui.CircuitState
import dev.onionvault.ui.KeyInfo
import dev.onionvault.ui.NormalizedBounds
import dev.onionvault.ui.PgpFieldState
import dev.onionvault.ui.PgpOverlayState
import dev.onionvault.ui.PgpPlaintextState
import dev.onionvault.ui.TorBootstrapState

/**
 * The previews of OnionVault.
 *
 * Every preview renders the screen from a pure state, without a browser and
 * without the native library: the same code paths are used by the
 * application, by the previews of Android Studio and by the screenshot tests
 * of the continuous integration.
 */
private object Previews {
    /** The color of the background of the hidden service page. */
    val Page = Color(0xFF12100E)

    /** The color of the text of the hidden service page. */
    val Text = Color(0xFFD9D2C6)

    /** The color of the field of the hidden service page. */
    val Field = Color(0xFF1B1815)
}

/**
 * A mock of the hidden service page.
 *
 * The off-screen rendered page is injected into the screen, so the preview
 * shows a stand-in of a no-JavaScript form of a hidden service.
 */
@Composable
private fun MockHiddenServicePage() {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Previews.Page)
            .padding(16.dp),
    ) {
        Text(
            text = "SecureDrop",
            color = Previews.Text,
            fontSize = 20.sp,
            fontFamily = FontFamily.Monospace,
        )
        Spacer(Modifier.height(12.dp))
        Text(
            text = "Send a message to the journalist:",
            color = Previews.Text,
            fontSize = 13.sp,
        )
        Spacer(Modifier.height(8.dp))
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(90.dp)
                .border(1.dp, Color(0xFF1E8C9B), RoundedCornerShape(4.dp))
                .background(Previews.Field)
                .padding(8.dp),
        ) {
            Text(
                text = "This message will be encrypted with the recipient's key.",
                color = Color(0xFF8A8175),
                fontSize = 12.sp,
            )
        }
        Spacer(Modifier.height(12.dp))
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(70.dp)
                .border(1.dp, Color(0xFF9A6224), RoundedCornerShape(4.dp))
                .background(Previews.Field)
                .padding(8.dp),
        ) {
            Text(
                text = "-----BEGIN PGP MESSAGE-----\n\nhQEMA...\n-----END PGP MESSAGE-----",
                color = Color(0xFF8A8175),
                fontSize = 10.sp,
                fontFamily = FontFamily.Monospace,
            )
        }
    }
}

/**
 * The first preview: the state of the Tor client.
 *
 * The bootstrap is at 65 percent, the circuit of the hidden service is
 * shown with the round trip time of every hop, and the token of the circuit
 * proves that the page is isolated from the other hidden services.
 */
@Preview(name = "1. Tor Bootstrap & Circuit Isolation HUD", heightDp = 640)
@Composable
fun TorBootstrapPreview() {
    BrowserScreen(
        state = BrowserUiState(
            url = "http://sdshqo3t6ilcq6cyudujqi3l5n6dqsmx6x3rv3moymqm3vhch6ad.onion",
            title = "SecureDrop",
            tor = TorBootstrapState(
                percent = 65,
                stage = "building the circuit",
                ready = false,
                circuit = CircuitState(
                    hops = listOf(
                        CircuitHop("Guard", 412),
                        CircuitHop("Middle", 287),
                        CircuitHop("Exit", 1_204),
                        CircuitHop("Rendezvous", 1_876),
                    ),
                ),
                isolationToken = "iso-3f7c91a2b4e08d6a",
            ),
        ),
        surface = { MockHiddenServicePage() },
    )
}

/**
 * The second preview: the form of a hidden service with the action chips.
 *
 * The chips are anchored on the fields that the render process detected; the
 * document itself is a mock, because the preview must not need a browser.
 */
@Preview(name = "2. Highlighted No-JS Form with PGP Action Chips", heightDp = 640)
@Composable
fun PgpActionChipsPreview() {
    BrowserScreen(
        state = BrowserUiState(
            url = "http://sdshqo3t6ilcq6cyudujqi3l5n6dqsmx6x3rv3moymqm3vhch6ad.onion",
            title = "SecureDrop",
            tor = TorBootstrapState(
                percent = 100,
                stage = "ready",
                ready = true,
                circuit = CircuitState(
                    hops = listOf(
                        CircuitHop("Guard", 398),
                        CircuitHop("Middle", 301),
                        CircuitHop("Exit", 1_098),
                        CircuitHop("Rendezvous", 1_744),
                    ),
                ),
                isolationToken = "iso-3f7c91a2b4e08d6a",
            ),
            pgp = PgpOverlayState(
                fields = listOf(
                    PgpFieldState(
                        nodeId = "ov-00000000000000a1",
                        kind = "textarea",
                        label = "Encrypt Field",
                        bounds = NormalizedBounds(0.05f, 0.22f, 0.95f, 0.36f),
                        preview = "This message will be encrypted…",
                    ),
                    PgpFieldState(
                        nodeId = "ov-00000000000000b2",
                        kind = "pgp_message",
                        label = "Decrypt Block",
                        bounds = NormalizedBounds(0.05f, 0.44f, 0.95f, 0.56f),
                        preview = "-----BEGIN PGP MESSAGE-----",
                    ),
                    PgpFieldState(
                        nodeId = "ov-00000000000000c3",
                        kind = "pgp_public_key",
                        label = "Insert My PubKey",
                        bounds = NormalizedBounds(0.05f, 0.64f, 0.95f, 0.74f),
                        preview = "-----BEGIN PGP PUBLIC KEY BLOCK-----",
                    ),
                ),
            ),
        ),
        surface = { MockHiddenServicePage() },
    )
}

/**
 * The third preview: the predictive back gesture in the middle of the
 * swipe.
 *
 * The live page is scaled down, the frame of the previous page is visible
 * behind it, and the progress of the gesture is at 65 percent.
 */
@Preview(name = "3. Predictive Back Mid-Swipe Gesture (Progress = 0.65f)", heightDp = 640)
@Composable
fun PredictiveBackPreview() {
    BrowserScreen(
        state = BrowserUiState(
            url = "http://sdshqo3t6ilcq6cyudujqi3l5n6dqsmx6x3rv3moymqm3vhch6ad.onion",
            title = "SecureDrop",
            canGoBack = true,
            backPreview = dev.onionvault.ui.BackPreviewState(
                url = "http://2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid.onion",
                title = "The inbox",
                fields = listOf(
                    PgpFieldState(
                        nodeId = "ov-00000000000000d4",
                        kind = "pgp_message",
                        label = "Decrypt Block",
                        bounds = NormalizedBounds(0.05f, 0.30f, 0.95f, 0.42f),
                    ),
                ),
            ),
            tor = TorBootstrapState(
                percent = 100,
                stage = "ready",
                ready = true,
                isolationToken = "iso-3f7c91a2b4e08d6a",
            ),
        ),
        previewProgress = 0.65f,
        surface = { MockHiddenServicePage() },
    )
}

/**
 * The fourth preview: the key manager and the sheet with the decrypted
 * message.
 *
 * The plaintext is shown in the native UI and not in the page; the badge
 * shows that the signature of the sender was verified.
 */
@Preview(name = "4. Hardware-Backed PGP Key Manager & Decryption Modal", heightDp = 640)
@Composable
fun PgpDecryptionModalPreview() {
    BrowserScreen(
        state = BrowserUiState(
            url = "http://sdshqo3t6ilcq6cyudujqi3l5n6dqsmx6x3rv3moymqm3vhch6ad.onion",
            title = "SecureDrop",
            tor = TorBootstrapState(
                percent = 100,
                stage = "ready",
                ready = true,
                isolationToken = "iso-3f7c91a2b4e08d6a",
            ),
            pgp = PgpOverlayState(
                fields = listOf(
                    PgpFieldState(
                        nodeId = "ov-00000000000000b2",
                        kind = "pgp_message",
                        label = "Decrypt Block",
                        bounds = NormalizedBounds(0.05f, 0.44f, 0.95f, 0.56f),
                    ),
                ),
                plaintext = PgpPlaintextState(
                    nodeId = "ov-00000000000000b2",
                    title = "Decrypted message",
                    body = "The meeting is at 14:00. Bring the documents.\n\n-- Anna",
                    signatureVerified = true,
                    signerFingerprint = "6F1A 2C93 9B4E 77D1 0E58 5A62 3F8B 91C4 7D0A 11E2",
                ),
            ),
            keys = listOf(
                KeyInfo(
                    fingerprint = "6F1A2C939B4E77D10E585A623F8B91C47D0A11E2",
                    userId = "Anna <anna@protonmail.com>",
                    hasSecret = false,
                ),
                KeyInfo(
                    fingerprint = "A3D9071F5C2E8B40D16F7394C8E5A2B6F04D71C93",
                    userId = "OnionVault <7f3c91a2@example.onion>",
                    hasSecret = true,
                ),
            ),
        ),
        showKeyManager = true,
        surface = { MockHiddenServicePage() },
    )
}
