package dev.onionvault.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.onionvault.R

/**
 * The controls of Trap.
 *
 * Every control is painted with the colours of the artwork: the teal of the
 * badge is the primary colour, the amber of the onion is the secondary
 * colour, and the surfaces are a warm dark so that both of them carry.
 */

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

/** The icon button that carries the brand mark of Trap. */
@Composable
fun OvBrandButton(
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = 38.dp,
) {
    // The mark is a rounded square, like the icon of the launcher; a circle
    // would cut the corners of the artwork away.
    val shape = RoundedCornerShape(percent = 28)
    Box(
        modifier = modifier
            .size(size)
            .clip(shape)
            .background(OnionPalette.SurfaceHigh)
            .border(1.dp, OnionPalette.Hairline, shape)
            .clickable(onClick = onClick)
            .semantics { this.contentDescription = contentDescription },
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            painter = painterResource(R.drawable.ov_trap_mark),
            contentDescription = null,
            tint = Color.Unspecified,
            modifier = Modifier.size(size * 0.86f),
        )
    }
}

/**
 * The primary button of the browser: the amber of the onion.
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
 * the badge.
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
            Icon(
                painter = icon,
                contentDescription = null,
                tint = if (enabled) content else OnionPalette.TextMuted,
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
 * teal decrypts and the green of the leaves handles the keys.
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
        ChipKind.Key -> OnionPalette.Leaf
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
            Icon(
                painter = icon,
                contentDescription = null,
                tint = accent,
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
 * The bar is painted with the gradient of the badge, so that it looks like
 * the artwork fills up while the client bootstraps.
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
                        listOf(OnionPalette.Teal, OnionPalette.Cyan),
                    ),
                ),
        )
    }
}
