package dev.onionvault.ui

import androidx.compose.ui.graphics.Color

/**
 * The colours of OnionVault.
 *
 * They are taken from the artwork of the application: the teal of the badge
 * is the primary colour, the amber of the skin of the onion is the secondary
 * colour, the green of the leaves is the accent.  The surfaces are a warm
 * dark, so that the amber and the teal of the controls carry.
 */
object OnionPalette {

    /** The teal at the top left of the badge. */
    val Teal = Color(0xFF1DE5C2)

    /** The teal in the middle of the badge. */
    val TealDeep = Color(0xFF16C2CF)

    /** The cyan at the bottom right of the badge. */
    val Cyan = Color(0xFF10A1D8)

    /** The dark cyan of the shadow under the onion. */
    val CyanDeep = Color(0xFF0B7FB4)

    /** The amber of the skin of the onion. */
    val Amber = Color(0xFFF5A93C)

    /** The light amber of the highlight of the onion. */
    val AmberLight = Color(0xFFFFD98A)

    /** The dark amber of the shadow of the onion. */
    val AmberDeep = Color(0xFFD9781F)

    /** The green of the leaves. */
    val Leaf = Color(0xFFA9E45F)

    /** The blue of the second leaf. */
    val LeafCool = Color(0xFF45C2E8)

    /** The dark brown of the outline of the artwork. */
    val Outline = Color(0xFF3A1608)

    /** The background of the application. */
    val Surface = Color(0xFF14110E)

    /** The background of a bar. */
    val SurfaceRaised = Color(0xFF1E1913)

    /** The background of a card, a chip or a button. */
    val SurfaceHigh = Color(0xFF2A2219)

    /** The line between two surfaces. */
    val Hairline = Color(0xFF3B2F24)

    /** The colour of the text. */
    val TextPrimary = Color(0xFFF7F1E8)

    /** The colour of a text that is less important. */
    val TextMuted = Color(0xFFB3A794)

    /** The colour of a text on the amber of a button. */
    val OnAmber = Color(0xFF3A1608)

    /** The colour of a text on the teal of a button. */
    val OnTeal = Color(0xFF07302C)

    /** The colour that says that something works. */
    val Good = Color(0xFF8FD44E)

    /** The colour that says that something needs attention. */
    val Warn = Color(0xFFFFB74D)

    /** The colour of a bad news. */
    val Bad = Color(0xFFE8705A)
}
