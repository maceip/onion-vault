package dev.onionvault

import android.graphics.Bitmap
import android.graphics.Canvas
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import dev.onionvault.ui.previews.LegalNoticesPreview
import dev.onionvault.ui.previews.PgpActionChipsPreview
import dev.onionvault.ui.previews.PgpDecryptionModalPreview
import dev.onionvault.ui.previews.PredictiveBackPreview
import dev.onionvault.ui.previews.TorBootstrapPreview
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
