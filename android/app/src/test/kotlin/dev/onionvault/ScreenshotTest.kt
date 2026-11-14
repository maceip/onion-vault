package dev.onionvault

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import dev.onionvault.ui.BrowserScreen
import dev.onionvault.ui.previews.BrowserChromePreview
import dev.onionvault.ui.previews.LegalNoticesPreview
import dev.onionvault.ui.previews.PgpActionChipsPreview
import dev.onionvault.ui.previews.PgpDecryptionModalPreview
import dev.onionvault.ui.previews.PredictiveBackPreview
import dev.onionvault.ui.previews.TorBootstrapPreview
import dev.onionvault.ui.previews.TorTaxiPreview
import dev.onionvault.ui.previews.torTaxiState
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.io.File

/**
 * The screenshot harness of the previews.
 *
 * Every preview is rendered with the real Compose runtime on the JVM and
 * written as a PNG, so that the design can be inspected without a device.
 * The images land in `build/screenshots/`.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ScreenshotTest {

    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun the_tor_bootstrap_is_rendered() {
        capture("1-tor-bootstrap") { TorBootstrapPreview() }
    }

    @Test
    fun the_pgp_chips_are_rendered() {
        capture("2-pgp-chips") { PgpActionChipsPreview() }
    }

    @Test
    fun the_predictive_back_is_rendered() {
        capture("3-predictive-back") { PredictiveBackPreview() }
    }

    @Test
    fun the_key_manager_is_rendered() {
        capture("4-key-manager") { PgpDecryptionModalPreview() }
    }

    @Test
    fun the_legal_notices_are_rendered() {
        capture("5-legal-notices") { LegalNoticesPreview() }
    }

    @Test
    fun the_browser_chrome_is_rendered() {
        capture("6-browser-chrome") { BrowserChromePreview() }
    }

    @Test
    fun tor_taxi_is_rendered() {
        // The page that was fetched over Tor is injected when the harness
        // points at one; the continuous integration renders the mock page of
        // the preview.
        val page = fetchedPage()
        if (page == null) {
            capture("7-tor-taxi") { TorTaxiPreview() }
        } else {
            capture("7-tor-taxi") {
                BrowserScreen(
                    state = torTaxiState(),
                    surface = {
                        Image(
                            bitmap = page,
                            contentDescription = "tor.taxi",
                            contentScale = ContentScale.FillBounds,
                            modifier = Modifier.fillMaxSize(),
                        )
                    },
                )
            }
        }
    }

    /**
     * The page that was fetched over Tor, when the harness points at one.
     *
     * The environment variable `TRAP_PAGE_IMAGE` holds the path of a PNG
     * of a page that was fetched through the Tor client of the application;
     * without it the preview renders its own mock page.
     */
    private fun fetchedPage(): ImageBitmap? {
        val path = System.getenv("TRAP_PAGE_IMAGE") ?: return null
        val file = File(path)
        if (!file.exists()) return null
        val bitmap = BitmapFactory.decodeFile(file.absolutePath) ?: return null
        return bitmap.asImageBitmap()
    }

    /** Render a preview and write it into `build/screenshots/`. */
    private fun capture(name: String, content: @Composable () -> Unit) {
        compose.setContent {
            Box(modifier = Modifier.fillMaxSize()) {
                content()
            }
        }
        compose.waitForIdle()

        val view = compose.activity.window.decorView
        view.measure(
            android.view.View.MeasureSpec.makeMeasureSpec(view.width, android.view.View.MeasureSpec.EXACTLY),
            android.view.View.MeasureSpec.makeMeasureSpec(view.height, android.view.View.MeasureSpec.EXACTLY),
        )
        val bitmap = Bitmap.createBitmap(
            view.width.coerceAtLeast(1),
            view.height.coerceAtLeast(1),
            Bitmap.Config.ARGB_8888,
        )
        val canvas = Canvas(bitmap)
        view.draw(canvas)

        // A dialog lives in its own window; it is drawn over the screen.
        val dialog = org.robolectric.shadows.ShadowDialog.getLatestDialog()
        if (dialog != null && dialog.isShowing) {
            val decor = dialog.window?.decorView
            if (decor != null && decor.width > 0 && decor.height > 0) {
                val overlay = Bitmap.createBitmap(decor.width, decor.height, Bitmap.Config.ARGB_8888)
                decor.draw(Canvas(overlay))
                val attributes = dialog.window?.attributes
                canvas.drawBitmap(
                    overlay,
                    (attributes?.x ?: 0).toFloat(),
                    (attributes?.y ?: 0).toFloat(),
                    null,
                )
            }
        }

        val directory = File("build/screenshots").apply { mkdirs() }
        File(directory, "$name.png").outputStream().use { stream ->
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, stream)
        }
    }
}
