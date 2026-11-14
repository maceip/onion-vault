package dev.onionvault.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.onionvault.R

/**
 * The controls of Trap.
 *
 * The iconography is the neon system of the design board: every glyph is cut
 * out of the board itself, with its own colour, and it is drawn on a plate
 * whose border carries the teal of the brand.  The controls are never the
 * stock icons of the material library: the board is the style of the
 * application.
 */

/**
 * A button of the chrome with a neon glyph.
 *
 * The glyph keeps its own colour, so it is drawn as an image and not as a
 * tinted icon.
 *
 * @param icon the glyph of the button.
 * @param contentDescription the description of the button.
 * @param onClick the user tapped the button.
 * @param modifier the modifier of the button.
 * @param enabled `false` dims the button and stops the taps.
 * @param accent the colour of the border of the plate.
 * @param plate the size of the plate of the button.
 * @param glyph the size of the glyph.
 */
@Composable
fun OvNeonButton(
    icon: Painter,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    accent: Color = OnionPalette.Teal,
    plate: Dp = 38.dp,
    glyph: Dp = 24.dp,
) {
    val shape = CircleShape
    Box(
        modifier = modifier
            .size(plate)
            .clip(shape)
            .background(OnionPalette.SurfaceHigh.copy(alpha = 0.55f))
            .border(1.dp, if (enabled) accent.copy(alpha = 0.75f) else OnionPalette.Hairline, shape)
            .clickable(enabled = enabled, onClick = onClick)
            .semantics { this.contentDescription = contentDescription },
        contentAlignment = Alignment.Center,
    ) {
        Image(
            painter = icon,
            contentDescription = null,
            contentScale = ContentScale.Fit,
            modifier = Modifier
                .size(glyph)
                .alpha(if (enabled) 1f else 0.35f),
        )
    }
}

/**
 * A button of the chrome with a neon glyph and a label under it.
 *
 * The tile is one of the tiles of the dock of the design board: a square
 * plate with the border of the brand and the name of the feature below it.
 *
 * @param icon the glyph of the tile.
 * @param label the name of the feature.
 * @param onClick the user tapped the tile.
 * @param modifier the modifier of the tile.
 * @param active `true` when the feature is on.
 */
@Composable
fun OvNeonTile(
    icon: Painter,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    active: Boolean = false,
) {
    val accent = if (active) OnionPalette.Mint else OnionPalette.Teal
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .clickable(onClick = onClick)
            .semantics { contentDescription = label }
            .padding(vertical = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier = Modifier
                .size(46.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(
                    if (active) {
                        OnionPalette.Teal.copy(alpha = 0.16f)
                    } else {
                        OnionPalette.SurfaceHigh.copy(alpha = 0.55f)
                    },
                )
                .border(1.dp, accent.copy(alpha = if (active) 1f else 0.7f), RoundedCornerShape(12.dp)),
            contentAlignment = Alignment.Center,
        ) {
            Image(
                painter = icon,
                contentDescription = null,
                contentScale = ContentScale.Fit,
                modifier = Modifier.size(28.dp),
            )
        }
        Spacer(Modifier.height(3.dp))
        Text(
            text = label,
            color = if (active) OnionPalette.Mint else OnionPalette.TextMuted,
            fontSize = 9.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/**
 * A row of the menu of the browser.
 *
 * @param icon the glyph of the row.
 * @param label the name of the action.
 * @param onClick the user tapped the row.
 * @param modifier the modifier of the row.
 * @param enabled `false` dims the row and stops the taps.
 * @param trailing the text that is shown at the end of the row.
 */
@Composable
fun OvMenuRow(
    icon: Painter,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    trailing: String? = null,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Image(
            painter = icon,
            contentDescription = null,
            contentScale = ContentScale.Fit,
            modifier = Modifier
                .size(22.dp)
                .alpha(if (enabled) 1f else 0.4f),
        )
        Spacer(Modifier.width(12.dp))
        Text(
            text = label,
            color = if (enabled) OnionPalette.TextPrimary else OnionPalette.TextMuted,
            fontSize = 14.sp,
            fontWeight = FontWeight.Medium,
            modifier = Modifier.weight(1f),
        )
        if (trailing != null) {
            Text(
                text = trailing,
                color = OnionPalette.Teal,
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold,
            )
        }
    }
}

/**
 * The brand mark of Trap: the wordmark of the board and the two onions.
 *
 * @param modifier the modifier of the mark.
 * @param wordmark the size of the wordmark.
 */
@Composable
fun OvBrandMark(
    modifier: Modifier = Modifier,
    wordmark: Dp = 88.dp,
) {
    Row(
        modifier = modifier,
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Image(
            painter = painterResource(R.drawable.ic_neon_trap_logo),
            contentDescription = "Trap",
            contentScale = ContentScale.Fit,
            modifier = Modifier
                .width(wordmark)
                .height(wordmark * 0.5f),
        )
        Image(
            painter = painterResource(R.drawable.ic_neon_onion_twins),
            contentDescription = null,
            contentScale = ContentScale.Fit,
            modifier = Modifier
                .width(wordmark * 0.5f)
                .height(wordmark * 0.35f),
        )
    }
}

/**
 * The plate of the address of the browser.
 *
 * The plate is the pill of the design board: a rounded field with the border
 * of the brand, the shield of the leak protection on the left and the button
 * that clears the address on the right.
 *
 * @param content the content of the field.
 * @param modifier the modifier of the plate.
 */
@Composable
fun OvAddressPlate(
    modifier: Modifier = Modifier,
    border: Color = OnionPalette.TealDeep,
    content: @Composable () -> Unit,
) {
    val shape = RoundedCornerShape(22.dp)
    Box(
        modifier = modifier
            .clip(shape)
            .background(OnionPalette.SurfaceHigh.copy(alpha = 0.6f))
            .border(1.dp, border, shape),
        contentAlignment = Alignment.CenterStart,
    ) {
        content()
    }
}

/** The icon button of the browser: a round plate with a thin outline. */
@Composable
fun OvIconButton(
    icon: Painter,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tint: Color = OnionPalette.Teal,
    enabled: Boolean = true,
    size: Dp = 38.dp,
) {
    val shape = CircleShape
    Box(
        modifier = modifier
            .size(size)
            .clip(shape)
            .background(if (enabled) OnionPalette.SurfaceHigh else OnionPalette.SurfaceRaised)
            .border(1.dp, if (enabled) OnionPalette.Hairline else OnionPalette.SurfaceRaised, shape)
            .clickable(enabled = enabled, onClick = onClick)
            .semantics { this.contentDescription = contentDescription },
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            painter = icon,
            contentDescription = null,
            tint = if (enabled) tint else OnionPalette.TextMuted.copy(alpha = 0.4f),
            modifier = Modifier.size(size * 0.55f),
        )
    }
}

/**
 * The primary button of the browser: the amber of the board.
 *
 * @param icon the icon of the button; it may be `null`.
 */
@Composable
fun OvPrimaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: Painter? = null,
    enabled: Boolean = true,
) {
    OvTextButton(
        text = text,
        onClick = onClick,
        modifier = modifier,
        icon = icon,
        enabled = enabled,
        container = OnionPalette.Amber,
        content = OnionPalette.OnAmber,
        border = null,
    )
}

/**
 * The secondary button of the browser: an outlined plate with the teal of
 * the brand.
 *
 * @param icon the icon of the button; it may be `null`.
 */
@Composable
fun OvSecondaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: Painter? = null,
    enabled: Boolean = true,
) {
    OvTextButton(
        text = text,
        onClick = onClick,
        modifier = modifier,
        icon = icon,
        enabled = enabled,
        container = OnionPalette.SurfaceHigh,
        content = OnionPalette.Teal,
        border = BorderStroke(1.dp, OnionPalette.TealDeep),
    )
}

/** The button that carries a text and, optionally, an icon. */
@Composable
private fun OvTextButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier,
    icon: Painter?,
    enabled: Boolean,
    container: Color,
    content: Color,
    border: BorderStroke?,
) {
    val shape = RoundedCornerShape(10.dp)
    Row(
        modifier = modifier
            .clip(shape)
            .background(if (enabled) container else OnionPalette.SurfaceRaised)
            .then(if (border != null) Modifier.border(border, shape) else Modifier)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.Center,
    ) {
        if (icon != null) {
            Image(
                painter = icon,
                contentDescription = null,
                contentScale = ContentScale.Fit,
                modifier = Modifier.size(17.dp),
            )
            Spacer(Modifier.width(6.dp))
        }
        Text(
            text = text,
            color = if (enabled) content else OnionPalette.TextMuted,
            fontSize = 13.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
        )
    }
}

/**
 * The chip of a PGP action.
 *
 * The colour of the chip tells what the action does: the amber encrypts, the
 * teal decrypts and the mint of the board handles the keys.
 */
@Composable
fun OvActionChip(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: Painter? = null,
    kind: ChipKind = ChipKind.Decrypt,
) {
    val accent = when (kind) {
        ChipKind.Encrypt -> OnionPalette.Amber
        ChipKind.Decrypt -> OnionPalette.Teal
        ChipKind.Key -> OnionPalette.Mint
    }
    val shape = RoundedCornerShape(8.dp)
    Row(
        modifier = modifier
            .clip(shape)
            .background(OnionPalette.SurfaceHigh)
            .border(1.dp, accent, shape)
            .clickable(onClick = onClick)
            .padding(horizontal = 8.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Image(
                painter = icon,
                contentDescription = null,
                contentScale = ContentScale.Fit,
                modifier = Modifier.size(13.dp),
            )
            Spacer(Modifier.width(4.dp))
        }
        Text(
            text = text,
            color = accent,
            fontSize = 11.sp,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
        )
    }
}

/** The kinds of the action chips. */
enum class ChipKind { Encrypt, Decrypt, Key }

/** The plate that holds a label, used for the state of the transport. */
@Composable
fun OvBadge(
    text: String,
    modifier: Modifier = Modifier,
    accent: Color = OnionPalette.TextMuted,
    container: Color = OnionPalette.SurfaceHigh,
) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(5.dp))
            .background(container)
            .padding(horizontal = 6.dp, vertical = 2.dp),
    ) {
        Text(
            text = text,
            color = accent,
            fontSize = 10.sp,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
        )
    }
}

/**
 * The progress bar of the bootstrap.
 *
 * The bar is painted with the gradient of the brand, so that it looks like
 * the neon of the board fills up while the client bootstraps.
 */
@Composable
fun OvProgressBar(
    progress: Float,
    modifier: Modifier = Modifier,
    height: Dp = 4.dp,
) {
    val fraction = progress.coerceIn(0f, 1f)
    Box(
        modifier = modifier
            .height(height)
            .clip(RoundedCornerShape(height / 2))
            .background(OnionPalette.SurfaceHigh),
    ) {
        Box(
            modifier = Modifier
                .height(height)
                .fillMaxWidth(fraction)
                .clip(RoundedCornerShape(height / 2))
                .background(
                    Brush.horizontalGradient(
                        listOf(OnionPalette.Teal, OnionPalette.Mint),
                    ),
                ),
        )
    }
}

/**
 * The tab badge of the top bar.
 *
 * @param count the number of the tabs of the browser.
 */
@Composable
fun OvTabBadge(count: Int, modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(8.dp))
            .background(OnionPalette.Teal.copy(alpha = 0.14f))
            .border(1.dp, OnionPalette.Teal.copy(alpha = 0.8f), RoundedCornerShape(8.dp))
            .padding(horizontal = 7.dp, vertical = 3.dp),
    ) {
        Text(
            text = count.toString(),
            color = OnionPalette.Mint,
            fontSize = 11.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = FontFamily.Monospace,
        )
    }
}
