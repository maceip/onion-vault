package dev.onionvault.ui

import android.graphics.Bitmap
import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.unit.IntOffset
import dev.onionvault.OvNative
import org.json.JSONObject

/**
 * One entry of the back stack of Trap.
 *
 * The entry keeps everything that is needed to show the page again without
 * waiting for the hidden service: the rendered frame, the state of the DOM
 * and the PGP fields.
 *
 * @property url the address of the page.
 * @property title the title of the page.
 * @property scrollOffset the scroll offset of the page.
 * @property frameSnapshot the last frame of the page.
 * @property domHtmlSnapshot the markup of the document.
 * @property pgpFieldOverlays the PGP fields of the page.
 */
@Immutable
data class OnionBackStackEntry(
    val url: String,
    val title: String,
    val scrollOffset: IntOffset,
    val frameSnapshot: ImageBitmap?,
    val domHtmlSnapshot: ByteArray,
    val pgpFieldOverlays: List<PgpFieldState>,
) {
    /** The entries are compared by their content, not by their identity. */
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is OnionBackStackEntry) return false
        return url == other.url &&
            title == other.title &&
            scrollOffset == other.scrollOffset &&
            domHtmlSnapshot.contentEquals(other.domHtmlSnapshot) &&
            pgpFieldOverlays == other.pgpFieldOverlays
    }

    /** The hash code follows the equality. */
    override fun hashCode(): Int {
        var result = url.hashCode()
        result = 31 * result + title.hashCode()
        result = 31 * result + scrollOffset.hashCode()
        result = 31 * result + domHtmlSnapshot.contentHashCode()
        result = 31 * result + pgpFieldOverlays.hashCode()
        return result
    }
}

/**
 * The back stack of Trap.
 *
 * A hidden service is slow: a back navigation over a six hop circuit can
 * take seconds, and a page that was reached through a form POST cannot be
 * restored by the network stack at all.  The stack therefore keeps the
 * rendered frame and the state of every page that was visited, and the
 * predictive back gesture shows the frame of the previous page while the
 * finger is on the screen.
 *
 * The stack is bounded: the oldest page falls out of it.
 */
class OnionBackStackManager(
    /** The maximum number of entries of the stack. */
    val maxDepth: Int = MAX_DEPTH,
    /** The source of the snapshots; the tests use a fake one. */
    private val source: SnapshotSource = JniSnapshotSource,
) {

    /** The entries of the stack, the newest first. */
    private val stack = mutableListOf<OnionBackStackEntry>()

    /** The entries of the stack, the newest first. */
    val entries: List<OnionBackStackEntry>
        get() = synchronized(this) { stack.toList() }

    /** `true` when the browser can go back. */
    val canGoBack: Boolean
        get() = synchronized(this) { stack.size > 1 }

    /** The page that is shown behind the live page during the gesture. */
    val previous: OnionBackStackEntry?
        get() = synchronized(this) { stack.getOrNull(1) }

    /** The page that is currently shown. */
    val current: OnionBackStackEntry?
        get() = synchronized(this) { stack.firstOrNull() }

    /**
     * Read the stack from the engine.
     *
     * The engine keeps the snapshots; the UI pulls them, because a frame is
     * too big for a callback.
     */
    fun refresh() {
        val snapshots = source.snapshots()
        synchronized(this) {
            stack.clear()
            stack.addAll(snapshots.take(maxDepth))
        }
    }

    /** The page that is shown behind the live page during the gesture. */
    fun previewState(): BackPreviewState? {
        val entry = previous ?: return null
        return BackPreviewState(
            url = entry.url,
            title = entry.title,
            scrollOffset = entry.scrollOffset,
            fields = entry.pgpFieldOverlays,
            frameSnapshot = entry.frameSnapshot,
        )
    }

    companion object {
        /** The maximum number of entries of the stack. */
        const val MAX_DEPTH: Int = 12
    }
}

/** The source of the snapshots of the back stack. */
interface SnapshotSource {
    /** The snapshots of the engine, the newest first. */
    fun snapshots(): List<OnionBackStackEntry>
}

/** The snapshots of the engine, read through the JNI bridge. */
object JniSnapshotSource : SnapshotSource {
    override fun snapshots(): List<OnionBackStackEntry> {
        val count = OvNative.nativeSnapshotCount()
        if (count <= 0) {
            return emptyList()
        }
        val entries = ArrayList<OnionBackStackEntry>(count)
        for (index in 0 until count) {
            val info = OvNative.nativeSnapshotInfo(index) ?: continue
            val json = JSONObject(info)
            val frame = OvNative.nativeSnapshotFrame(index)?.let { bytes ->
                decodeFrame(bytes, json.optInt("frame_width"), json.optInt("frame_height"))
            }
            val markup = OvNative.nativeSnapshotMarkup(index) ?: ByteArray(0)
            entries.add(
                OnionBackStackEntry(
                    url = json.optString("url", ""),
                    title = json.optString("title", ""),
                    scrollOffset = IntOffset(
                        json.optDouble("scroll_x", 0.0).toInt(),
                        json.optDouble("scroll_y", 0.0).toInt(),
                    ),
                    frameSnapshot = frame,
                    domHtmlSnapshot = markup,
                    pgpFieldOverlays = parseFields(
                        json.optJSONArray("fields"),
                        json.optInt("viewport_width", 0),
                        json.optInt("viewport_height", 0),
                    ),
                ),
            )
        }
        return entries
    }

    /** Turn the pixels of a snapshot into a bitmap. */
    private fun decodeFrame(pixels: ByteArray, width: Int, height: Int): ImageBitmap? {
        if (width <= 0 || height <= 0 || pixels.size < width * height * 4) {
            return null
        }
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        bitmap.copyPixelsFromBuffer(java.nio.ByteBuffer.wrap(pixels))
        return bitmap.asImageBitmap()
    }
}

/**
 * Read the PGP fields of a page.
 *
 * The engine reports the fields of the page with their bounds in device
 * pixels; they are normalized here, so that the overlay follows the layout
 * of the page in every preview and on every screen.
 *
 * @param fields the fields of the DOM scan report.
 * @param viewportWidth the width of the page in pixels.
 * @param viewportHeight the height of the page in pixels.
 */
internal fun parseFields(
    fields: org.json.JSONArray?,
    viewportWidth: Int,
    viewportHeight: Int,
): List<PgpFieldState> {
    if (fields == null) {
        return emptyList()
    }
    val width = viewportWidth.coerceAtLeast(1).toFloat()
    val height = viewportHeight.coerceAtLeast(1).toFloat()
    val out = ArrayList<PgpFieldState>(fields.length())
    for (index in 0 until fields.length()) {
        val field = fields.optJSONObject(index) ?: continue
        val rect = field.optJSONObject("rect") ?: continue
        val kind = field.optString("kind", "")
        val left = (rect.optInt("x", 0).toFloat() / width).coerceIn(0f, 1f)
        val top = (rect.optInt("y", 0).toFloat() / height).coerceIn(0f, 1f)
        val right = ((rect.optInt("x", 0) + rect.optInt("width", 0)).toFloat() / width)
            .coerceIn(0f, 1f)
        val bottom = ((rect.optInt("y", 0) + rect.optInt("height", 0)).toFloat() / height)
            .coerceIn(0f, 1f)
        out.add(
            PgpFieldState(
                nodeId = field.optString("node_id", ""),
                kind = kind,
                label = labelFor(kind),
                bounds = NormalizedBounds(
                    left = left,
                    top = top,
                    right = maxOf(right, left),
                    bottom = maxOf(bottom, top),
                ),
                preview = field.optString("value_preview", ""),
                armored = field.optString("armored").takeIf { it.isNotEmpty() },
            ),
        )
    }
    return out
}

/** The label of the action chip of a field. */
internal fun labelFor(kind: String): String = when (kind) {
    "pgp_message" -> "Decrypt Block"
    "pgp_public_key" -> "Insert My PubKey"
    "textarea" -> "Encrypt Field"
    "text_input" -> "Encrypt Field"
    else -> "Field"
}
