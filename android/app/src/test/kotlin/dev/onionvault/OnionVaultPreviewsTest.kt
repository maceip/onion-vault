package dev.onionvault

import androidx.compose.ui.test.isRoot
import androidx.compose.ui.test.junit4.createComposeRule
import dev.onionvault.ui.previews.BrowserChromePreview
import dev.onionvault.ui.previews.BrowserMenuPreview
import dev.onionvault.ui.previews.LegalNoticesPreview
import dev.onionvault.ui.previews.PgpActionChipsPreview
import dev.onionvault.ui.previews.PgpDecryptionModalPreview
import dev.onionvault.ui.previews.PredictiveBackPreview
import dev.onionvault.ui.previews.TorBootstrapPreview
import dev.onionvault.ui.previews.TorTaxiPreview
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The screenshot test of the previews.
 *
 * Every preview of Trap is rendered on the JVM, so that the continuous
 * integration proves that the screens inflate and that the layout does not
 * break - without an emulator, without a device and without the native
 * browser.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp-xhdpi")
class OnionVaultPreviewsTest {

    /** The rule that renders the composition. */
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun the_tor_bootstrap_preview_renders() {
        compose.setContent { TorBootstrapPreview() }
        compose.onAllNodes(isRoot())[0].assertExists()
    }

    @Test
    fun the_pgp_action_chips_preview_renders() {
        compose.setContent { PgpActionChipsPreview() }
        compose.onAllNodes(isRoot())[0].assertExists()
    }

    @Test
    fun the_predictive_back_preview_renders() {
        compose.setContent { PredictiveBackPreview() }
        compose.onAllNodes(isRoot())[0].assertExists()
    }

    @Test
    fun the_decryption_modal_preview_renders() {
        compose.setContent { PgpDecryptionModalPreview() }
        compose.onAllNodes(isRoot())[0].assertExists()
    }

    @Test
    fun the_legal_notices_preview_renders() {
        compose.setContent { LegalNoticesPreview() }
        compose.onAllNodes(isRoot())[0].assertExists()
    }

    @Test
    fun the_browser_chrome_preview_renders() {
        compose.setContent { BrowserChromePreview() }
        compose.onAllNodes(isRoot())[0].assertExists()
    }

    @Test
    fun the_tor_taxi_preview_renders() {
        compose.setContent { TorTaxiPreview() }
        compose.onAllNodes(isRoot())[0].assertExists()
    }

    @Test
    fun the_browser_menu_preview_renders() {
        compose.setContent { BrowserMenuPreview() }
        compose.onAllNodes(isRoot())[0].assertExists()
    }
}
