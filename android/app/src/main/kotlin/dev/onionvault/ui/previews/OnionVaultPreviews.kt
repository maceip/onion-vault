package dev.onionvault.ui.previews

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.onionvault.R
import dev.onionvault.ui.BrowserScreen
import dev.onionvault.ui.BrowserUiState
import dev.onionvault.ui.CircuitHop
import dev.onionvault.ui.CircuitState
import dev.onionvault.ui.KeyInfo
import dev.onionvault.ui.LegalDocument
import dev.onionvault.ui.NormalizedBounds
import dev.onionvault.ui.PgpFieldState
import dev.onionvault.ui.PgpOverlayState
import dev.onionvault.ui.PgpPlaintextState
import dev.onionvault.ui.TorBootstrapState

/**
 * The previews of Trap.
 *
 * Every preview renders the screen from a pure state, without a browser and
 * without the native library: the same code paths are used by the
 * application, by the previews of Android Studio and by the screenshot tests
 * of the continuous integration.
 */
private object Previews {
    /** The color of the background of the hidden service page. */
    val Page = Color(0xFF15181F)

    /** The color of the text of the hidden service page. */
    val Text = Color(0xFFF2EEE6)

    /** The color of the field of the hidden service page. */
    val Field = Color(0xFF1E222C)
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
                    userId = "Trap <7f3c91a2@example.onion>",
                    hasSecret = true,
                ),
            ),
        ),
        showKeyManager = true,
        surface = { MockHiddenServicePage() },
    )
}

/**
 * The fifth preview: the licences that travel with the application.
 *
 * The LGPL-3.0 of the engine asks for the recipient of the binary to be able
 * to read the licence of the library; the texts are therefore shown by the
 * application itself.
 */
@Preview(name = "5. Legal Notices of the Bundled Engine", heightDp = 640)
@Composable
fun LegalNoticesPreview() {
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
            legalDocuments = listOf(
                LegalDocument(
                    name = "LGPL-3.0.txt",
                    title = "GNU Lesser General Public License 3.0",
                    sizeBytes = 42_098,
                ),
                LegalDocument(
                    name = "GPL-3.0.txt",
                    title = "GNU General Public License 3.0",
                    sizeBytes = 34_674,
                ),
                LegalDocument(
                    name = "LGPL-2.1.txt",
                    title = "GNU Lesser General Public License 2.1",
                    sizeBytes = 26_001,
                ),
                LegalDocument(
                    name = "CEFRIUM-NOTICE.txt",
                    title = "The notice of the CEF engine",
                    sizeBytes = 2_273_727,
                ),
            ),
        ),
        showLegalNotices = true,
        surface = { MockHiddenServicePage() },
    )
}

/**
 * A mock of the start page of the browser.
 *
 * The application starts on `about:blank`; the preview shows the chrome of
 * the browser with the mark of the brand and the address of a hidden service.
 */
@Composable
private fun MockStartPage() {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Previews.Page)
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Spacer(Modifier.height(36.dp))
        Image(
            painter = painterResource(R.drawable.ov_trap_mark),
            contentDescription = "Trap",
            modifier = Modifier.size(96.dp),
        )
        Spacer(Modifier.height(16.dp))
        Text(
            text = "Trap",
            color = Previews.Text,
            fontSize = 22.sp,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(6.dp))
        Text(
            text = "Every request goes through Tor. JavaScript is switched off.",
            color = Color(0xFF8A8175),
            fontSize = 13.sp,
        )
    }
}

/**
 * A mock of the page of tor.taxi.
 *
 * The page is plain HTML without a single line of JavaScript, which is why it
 * is rendered by the browser of the application exactly as it is written.
 */
@Composable
private fun MockTorTaxiPage() {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(0xFF000000))
            .padding(16.dp),
    ) {
        Text(
            text = "tor.taxi - your ride to the darknet",
            color = Color(0xFFFFD400),
            fontSize = 18.sp,
            fontFamily = FontFamily.Monospace,
        )
        Spacer(Modifier.height(12.dp))
        Text(
            text = "Click here to visit our onion address!",
            color = Color(0xFFFFFFFF),
            fontSize = 13.sp,
            fontFamily = FontFamily.Monospace,
        )
        Spacer(Modifier.height(8.dp))
        Text(
            text = "Links in red are experiencing prolonged downtime.",
            color = Color(0xFFFF2222),
            fontSize = 13.sp,
            fontFamily = FontFamily.Monospace,
        )
        Spacer(Modifier.height(16.dp))
        Text(
            text = "Markets",
            color = Color(0xFFFFD400),
            fontSize = 15.sp,
            fontFamily = FontFamily.Monospace,
        )
        Spacer(Modifier.height(8.dp))
        Text(
            text = "Dark Matter - 2 3 +",
            color = Color(0xFFFF2222),
            fontSize = 13.sp,
            fontFamily = FontFamily.Monospace,
        )
        Spacer(Modifier.height(6.dp))
        Text(
            text = "DrugHub",
            color = Color(0xFFFFFFFF),
            fontSize = 13.sp,
            fontFamily = FontFamily.Monospace,
        )
        Spacer(Modifier.height(16.dp))
        Text(
            text = "Forums",
            color = Color(0xFFFFD400),
            fontSize = 15.sp,
            fontFamily = FontFamily.Monospace,
        )
        Spacer(Modifier.height(8.dp))
        Text(
            text = "Dread",
            color = Color(0xFFFFFFFF),
            fontSize = 13.sp,
            fontFamily = FontFamily.Monospace,
        )
    }
}

/**
 * The state of the chrome of the browser, for the preview and for the tests.
 *
 * The client is ready, so the badge of the transport shows that the tunnel is
 * up.
 */
fun chromeState(): BrowserUiState = BrowserUiState(
    url = "about:blank",
    title = "Trap",
    tor = TorBootstrapState(
        percent = 100,
        stage = "ready",
        ready = true,
        isolationToken = "iso-3f7c91a2b4e08d6a",
    ),
)

/**
 * The state of the browser after it has loaded tor.taxi over Tor.
 *
 * The title is the one of the page, the transport is ready and the circuit of
 * the page is the one of the hidden service.
 */
fun torTaxiState(): BrowserUiState = BrowserUiState(
    url = "https://tor.taxi",
    title = "tor.taxi - your ride to the darknet",
    tor = TorBootstrapState(
        percent = 100,
        stage = "ready",
        ready = true,
        isolationToken = "iso-3f7c91a2b4e08d6a",
    ),
)

/**
 * The sixth preview: the chrome of the browser.
 *
 * The top bar with the wordmark of the brand, the dock with the tiles of the
 * features, the address pill with the shield of the leak protection and the
 * row of the navigation, above the start page of the browser.
 */
@Preview(name = "6. The Browser Chrome of Trap", heightDp = 640)
@Composable
fun BrowserChromePreview() {
    BrowserScreen(
        state = chromeState(),
        surface = { MockStartPage() },
    )
}

/**
 * The seventh preview: the browser after it has loaded tor.taxi.
 *
 * The address pill carries the address of the page, the badge of the top bar
 * shows that the transport is up, and the dock shows that the browser only
 * reaches hidden services.
 */
@Preview(name = "7. tor.taxi Loaded over Tor", heightDp = 640)
@Composable
fun TorTaxiPreview() {
    BrowserScreen(
        state = torTaxiState(),
        surface = { MockTorTaxiPage() },
    )
}

/**
 * The eighth preview: the menu of the browser.
 *
 * The sheet is the menu of the design board: the entries that have a real
 * function behind them — the new tab, the tabs, the keyring, the traffic
 * policy, the zoom and the licences.
 */
@Preview(name = "8. The Menu of Trap", heightDp = 640)
@Composable
fun BrowserMenuPreview() {
    BrowserScreen(
        state = chromeState(),
        showMenu = true,
        surface = { MockStartPage() },
    )
}
