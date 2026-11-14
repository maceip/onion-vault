package dev.onionvault.ui

import androidx.compose.ui.graphics.Color

/**
 * The colours of Trap.
 *
 * They are the seven swatches of the brand palette of the design board
 * ("TRAP Neon Graffiti Browser Board"): the neon teal carries the browser,
 * the pink and the mint are the accents of the controls, the amber and the
 * cream are the warmth of the highlights and the purple is the shadow of the
 * mural.  Every surface is a near black, so that the sliced neon glyphs of
 * the board blend into the chrome instead of sitting on a light plate.
 */
object OnionPalette {

    /** The neon teal of the primary colour of the board. */
    val Teal = Color(0xFF00E4C1)

    /** The deep teal below the primary colour. */
    val TealDeep = Color(0xFF00A88F)

    /** The mint of the highlights of the board. */
    val Mint = Color(0xFF67EEE1)

    /** The pink of the graffiti of the board. */
    val Pink = Color(0xFFFD96C3)

    /** The deep pink of the shadow of the graffiti. */
    val PinkDeep = Color(0xFFB85C8B)

    /** The amber of the leaves of the board. */
    val Amber = Color(0xFFE8933E)

    /** The cream of the highlights of the board. */
    val Cream = Color(0xFFFBD4A9)

    /** The purple of the mural of the board. */
    val Purple = Color(0xFF8F55A6)

    /** The deep purple of the shadow of the mural. */
    val PurpleDeep = Color(0xFF5A3568)

    /** The near black of the background of the board. */
    val Surface = Color(0xFF15181F)

    /** The background of a bar. */
    val SurfaceRaised = Color(0xFF1E222C)

    /** The background of a card, a chip or a button. */
    val SurfaceHigh = Color(0xFF2A2F3C)

    /** The line between two surfaces. */
    val Hairline = Color(0xFF3A4050)

    /** The colour of the text. */
    val TextPrimary = Color(0xFFF2EEE6)

    /** The colour of a text that is less important. */
    val TextMuted = Color(0xFFA8A2B8)

    /** The colour of a text on the amber of a button. */
    val OnAmber = Color(0xFF2A1608)

    /** The colour of a text on the teal of a button. */
    val OnTeal = Color(0xFF04241F)

    /** The colour that says that something works. */
    val Good = Color(0xFF00E4C1)

    /** The colour that says that something needs attention. */
    val Warn = Color(0xFFE8933E)

    /** The colour of a bad news. */
    val Bad = Color(0xFFFD96C3)
}
