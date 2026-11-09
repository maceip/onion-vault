package dev.onionvault.ui

import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.activity.BackEventCompat
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The screen of the browser.
 *
 * The screen is driven by a pure [BrowserUiState]: the surface of the
 * off-screen rendered page is injected, so that the screen can be rendered
 * in a preview without a browser.
 *
 * The predictive back gesture shows the frame of the previous page while the
 * finger is on the screen.  The gesture does not wait for the hidden
 * service: the frame was rendered before and is kept in the back stack.
 *
 * @param state the state of the browser.
 * @param modifier the modifier of the screen.
 * @param onNavigate the user asked for another address.
 * @param onFieldAction the user tapped a PGP action chip.
 * @param onInsertPlaintext the user wants the plaintext in the page.
 * @param onReloadLive the user wants the page to be fetched again.
 * @param onBack the user committed the back gesture.
 * @param surface the off-screen rendered page.
 * @param previewProgress the progress of the back gesture; the previews use
 * it to show the gesture in the middle of the swipe.
 */
@Composable
fun BrowserScreen(
    state: BrowserUiState,
    modifier: Modifier = Modifier,
    onNavigate: (String) -> Unit = {},
    onFieldAction: (PgpFieldState) -> Unit = {},
    onInsertPlaintext: (String) -> Unit = {},
    onReloadLive: () -> Unit = {},
    onBack: () -> Unit = {},
    surface: @Composable () -> Unit = {},
    previewProgress: Float = 0f,
) {
    var address by remember(state.url) { mutableStateOf(state.url) }
    // The preview of the gesture starts in the state that the parameter
    // describes; the gesture itself drives the values.
    val scale = remember { Animatable(1f - previewProgress * 0.12f) }
    val shift = remember { Animatable(previewProgress * 96f) }
    var progress by remember { mutableFloatStateOf(previewProgress) }
    var edge by remember { mutableStateOf(BackEventCompat.EDGE_LEFT) }
    val scope = rememberCoroutineScope()

    PredictiveBackHandler(enabled = state.canGoBack) { backFlow ->
        try {
            backFlow.collect { event ->
                edge = event.swipeEdge
                progress = event.progress
                // The live page shrinks and moves away, the cached page of the
                // back stack becomes visible behind it.
                scope.launch {
                    scale.snapTo(1f - event.progress * 0.12f)
                    shift.snapTo(event.progress * 96f * if (event.swipeEdge == BackEventCompat.EDGE_RIGHT) -1f else 1f)
                }
            }
            // The gesture was committed.
            onBack()
        } finally {
            // The gesture was cancelled (or the back was committed): the
            // live page returns to its place.
            withContext(NonCancellable) {
                scale.animateTo(1f, animationSpec = tween(180))
                shift.animateTo(0f, animationSpec = tween(180))
            }
            progress = 0f
        }
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Color(0xFF0B0E11)),
    ) {
        BrowserTopBar(
            address = address,
            onAddressChange = { address = it },
            onNavigate = { onNavigate(address) },
            onReloadLive = onReloadLive,
            showReload = state.canGoBack,
        )

        Box(modifier = Modifier.weight(1f)) {
            // The cached frame of the previous page.
            BackPreviewLayer(state = state, progress = progress)

            // The live page.
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .graphicsLayer {
                        scaleX = scale.value
                        scaleY = scale.value
                        translationX = shift.value
                    },
            ) {
                surface()
            }

            // The action chips of the PGP fields.
            PgpFieldOverlay(state = state, onFieldAction = onFieldAction)
        }

        TorStatusBar(state = state)

        state.pgp.plaintext?.let { plaintext ->
            PlaintextSheet(
                plaintext = plaintext,
                onInsert = { onInsertPlaintext(plaintext.nodeId) },
            )
        }
    }
}

/**
 * The top bar of the browser.
 *
 * @param address the address that is typed.
 * @param onAddressChange the user changed the address.
 * @param onNavigate the user wants to visit the address.
 * @param onReloadLive the user wants the page to be fetched again.
 * @param showReload `true` when the chip is shown.
 */
@Composable
private fun BrowserTopBar(
    address: String,
    onAddressChange: (String) -> Unit,
    onNavigate: () -> Unit,
    onReloadLive: () -> Unit,
    showReload: Boolean,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(Color(0xFF101418))
            .padding(horizontal = 8.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OutlinedTextField(
            value = address,
            onValueChange = onAddressChange,
            modifier = Modifier.weight(1f),
            singleLine = true,
            placeholder = { Text("onion address", color = Color.Gray) },
        )
        Spacer(Modifier.width(8.dp))
        Button(onClick = onNavigate) { Text("Go") }
        if (showReload) {
            Spacer(Modifier.width(8.dp))
            Button(onClick = onReloadLive) { Text("Reload Live over Tor") }
        }
    }
}

/**
 * The layer that shows the cached frame of the previous page.
 *
 * The scrim lightens while the gesture goes on, so that the page that is
 * about to come back is readable.
 */
@Composable
private fun BackPreviewLayer(state: BrowserUiState, progress: Float) {
    val preview = state.backPreview ?: return
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color(0xFF151A20)),
        contentAlignment = Alignment.Center,
    ) {
        val frame = preview.frameSnapshot
        if (frame != null) {
            // The frame that was rendered before the navigation; it is the
            // page that comes back, without waiting for the circuit.
            Image(
                bitmap = frame,
                contentDescription = preview.title.ifEmpty { preview.url },
                contentScale = ContentScale.FillBounds,
                modifier = Modifier.fillMaxSize(),
            )
        } else {
            Text(
                text = preview.title.ifEmpty { preview.url },
                color = Color(0xFF6C7885),
                fontSize = 14.sp,
                modifier = Modifier.padding(16.dp),
            )
        }

        // The scrim lightens while the gesture goes on.
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Color.Black.copy(alpha = 0.45f * (1f - progress))),
        )
    }
}

/**
 * The overlay with the action chips of the PGP fields.
 *
 * The chips are anchored on the bounds of the fields, which the render
 * process reports in the normalized space of the page.
 */
@Composable
private fun PgpFieldOverlay(
    state: BrowserUiState,
    onFieldAction: (PgpFieldState) -> Unit,
) {
    val fields = state.pgp.fields
    if (fields.isEmpty()) {
        return
    }
    BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
        val width = maxWidth
        val height = maxHeight
        fields.forEach { field ->
            val bounds = field.bounds
            // The bounds are relative to the page; the overlay is placed with
            // the same proportions, so that it follows the layout.
            Box(
                modifier = Modifier
                    .offset(
                        x = width * bounds.left,
                        y = height * bounds.top,
                    )
                    .width(width * bounds.width)
                    .padding(2.dp),
            ) {
                ActionChip(field = field, onClick = { onFieldAction(field) })
            }
        }
    }
}

/**
 * The chip of a PGP field.
 *
 * @param field the field the chip belongs to.
 * @param onClick the user tapped the chip.
 */
@Composable
private fun ActionChip(field: PgpFieldState, onClick: () -> Unit) {
    Button(
        onClick = onClick,
        modifier = Modifier
            .border(1.dp, Color(0xFF00E5FF), RoundedCornerShape(6.dp))
            .clip(RoundedCornerShape(6.dp)),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(
            horizontal = 8.dp,
            vertical = 2.dp,
        ),
    ) {
        Text(
            text = field.label,
            fontSize = 11.sp,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/**
 * The status line with the state of the Tor client.
 *
 * @param state the state of the browser.
 */
@Composable
private fun TorStatusBar(state: BrowserUiState) {
    val tor = state.tor
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(Color(0xFF101418))
            .padding(horizontal = 10.dp, vertical = 6.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = if (tor.ready) "Tor ready" else "Tor: ${tor.percent}% ${tor.stage}",
                color = if (tor.ready) Color(0xFF00FF66) else Color.LightGray,
                fontSize = 12.sp,
                modifier = Modifier.weight(1f),
            )
            tor.isolationToken?.let {
                Text(
                    text = it,
                    color = Color(0xFF00E5FF),
                    fontSize = 11.sp,
                    fontFamily = FontFamily.Monospace,
                )
            }
        }
        if (!tor.ready) {
            Spacer(Modifier.height(4.dp))
            LinearProgressIndicator(
                progress = { tor.percent / 100f },
                modifier = Modifier.fillMaxWidth(),
            )
        }
        tor.circuit?.let { circuit ->
            Spacer(Modifier.height(4.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                circuit.hops.forEach { hop ->
                    HopBadge(hop)
                }
            }
        }
    }
}

/**
 * The badge of a hop of the circuit.
 *
 * @param hop the hop of the circuit.
 */
@Composable
private fun HopBadge(hop: CircuitHop) {
    Surface(
        color = Color(0xFF1B2229),
        shape = RoundedCornerShape(4.dp),
    ) {
        Text(
            text = "${hop.role} ${hop.latencyMillis}ms",
            color = Color(0xFF9AA7B4),
            fontSize = 10.sp,
            fontFamily = FontFamily.Monospace,
            modifier = Modifier.padding(horizontal = 5.dp, vertical = 2.dp),
        )
    }
}

/**
 * The sheet with the decrypted message.
 *
 * The plaintext is shown in the native UI and not in the page: a hostile
 * hidden service could read it back through CSS.
 *
 * @param plaintext the decrypted message.
 * @param onInsert the user wants the plaintext in the page.
 */
@Composable
private fun PlaintextSheet(plaintext: PgpPlaintextState, onInsert: () -> Unit) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(Color(0xE6101418))
            .padding(12.dp),
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = plaintext.title,
                    color = Color.White,
                    fontSize = 16.sp,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.weight(1f),
                )
                SignatureBadge(plaintext)
            }
            Spacer(Modifier.height(6.dp))
            Box(
                modifier = Modifier
                    .height(120.dp)
                    .verticalScroll(rememberScrollState()),
            ) {
                Text(
                    text = plaintext.body,
                    color = Color.LightGray,
                    fontSize = 14.sp,
                    fontFamily = FontFamily.Monospace,
                )
            }
            Spacer(Modifier.height(6.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
            ) {
                Button(onClick = onInsert) { Text("Insert") }
            }
        }
    }
}

/**
 * The badge that shows the state of the signature.
 *
 * @param plaintext the decrypted message.
 */
@Composable
private fun SignatureBadge(plaintext: PgpPlaintextState) {
    val verified = plaintext.signatureVerified
    Surface(
        color = Color(0xFF1B2229),
        shape = RoundedCornerShape(4.dp),
    ) {
        Text(
            text = if (verified) {
                "Good Signature from fingerprint ${plaintext.signerFingerprint.orEmpty()}"
            } else {
                "Signature not verified"
            },
            color = if (verified) Color(0xFF00FF66) else Color(0xFFFFB300),
            fontSize = 10.sp,
            fontFamily = FontFamily.Monospace,
            modifier = Modifier.padding(horizontal = 6.dp, vertical = 3.dp),
        )
    }
}
