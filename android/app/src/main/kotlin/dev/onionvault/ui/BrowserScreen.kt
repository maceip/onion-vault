package dev.onionvault.ui

import androidx.activity.BackEventCompat
import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.onionvault.R
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
 * @param onForward the user tapped the forward button.
 * @param onReload the user tapped the reload button.
 * @param onNewTab the user tapped the new tab button.
 * @param onCreateKey the user wants a new PGP key.
 * @param onExportKey the user wants the public key of a certificate.
 * @param surface the off-screen rendered page.
 * @param previewProgress the progress of the back gesture; the previews use
 * it to show the gesture in the middle of the swipe.
 * @param showKeyManager the key manager is open; the previews use it to show
 * the manager of the keys.
 * @param showLegalNotices the list of the licences is open; the previews use
 * it to show the licences that travel with the application.
 * @param onLoadLegalLines the function that reads the text of a licence
 * document; it is called away from the thread of the interface.
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
    onForward: () -> Unit = {},
    onReload: () -> Unit = {},
    onNewTab: () -> Unit = {},
    onCreateKey: () -> Unit = {},
    onExportKey: (String) -> Unit = {},
    onLoadLegalLines: suspend (String) -> List<String> = { emptyList() },
    surface: @Composable () -> Unit = {},
    previewProgress: Float = 0f,
    showKeyManager: Boolean = false,
    showLegalNotices: Boolean = false,
) {
    var address by remember(state.url) { mutableStateOf(state.url) }
    // The preview of the gesture starts in the state that the parameter
    // describes; the gesture itself drives the values.
    val scale = remember { Animatable(1f - previewProgress * 0.12f) }
    val shift = remember { Animatable(previewProgress * 96f) }
    var progress by remember { mutableFloatStateOf(previewProgress) }
    var edge by remember { mutableStateOf(BackEventCompat.EDGE_LEFT) }
    var keysOpen by remember { mutableStateOf(showKeyManager) }
    var legalOpen by remember { mutableStateOf(showLegalNotices) }
    var legalDocument by remember { mutableStateOf<LegalDocument?>(null) }
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
                    shift.snapTo(
                        event.progress * 96f *
                            if (event.swipeEdge == BackEventCompat.EDGE_RIGHT) -1f else 1f,
                    )
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
            .background(OnionPalette.Surface),
    ) {
        BrandBar(
            state = state,
            onBrand = onNewTab,
            onKeys = { keysOpen = true },
            onLegal = { legalOpen = true },
        )

        NavigationBar(
            address = address,
            onAddressChange = { address = it },
            onNavigate = { onNavigate(address) },
            onBack = onBack,
            onForward = onForward,
            onReload = onReload,
            onNewTab = {
                address = ""
                onNewTab()
            },
            canGoBack = state.canGoBack,
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

    if (keysOpen) {
        KeyManagerDialog(
            keys = state.keys,
            onCreate = onCreateKey,
            onExport = onExportKey,
            onDismiss = { keysOpen = false },
        )
    }

    if (legalOpen) {
        LegalNoticesDialog(
            documents = state.legalDocuments,
            onOpenDocument = { document ->
                legalOpen = false
                legalDocument = document
            },
            onDismiss = { legalOpen = false },
        )
    }

    legalDocument?.let { document ->
        LegalDocumentDialog(
            document = document,
            loadLines = { onLoadLegalLines(document.name) },
            onDismiss = { legalDocument = null },
        )
    }
}

/**
 * The bar with the brand of PlugTalk.
 *
 * It carries the mark of the application, the title of the page, the state
 * of the transport, the button of the key manager and the button of the
 * licences.
 */
@Composable
private fun BrandBar(
    state: BrowserUiState,
    onBrand: () -> Unit,
    onKeys: () -> Unit,
    onLegal: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(OnionPalette.SurfaceRaised)
            .padding(start = 8.dp, end = 8.dp, top = 8.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OvBrandButton(
            contentDescription = "PlugTalk",
            onClick = onBrand,
            size = 34.dp,
        )
        Spacer(Modifier.width(10.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = state.title.ifEmpty { "PlugTalk" },
                color = OnionPalette.TextPrimary,
                fontSize = 15.sp,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = if (state.blocked) {
                    "the request was blocked"
                } else {
                    state.status.ifEmpty { state.url }
                },
                color = if (state.blocked) OnionPalette.Bad else OnionPalette.TextMuted,
                fontSize = 11.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Spacer(Modifier.width(8.dp))
        TorPill(state = state)
        Spacer(Modifier.width(6.dp))
        OvIconButton(
            icon = painterResource(R.drawable.ic_ov_keys),
            contentDescription = "The key manager",
            onClick = onKeys,
            tint = OnionPalette.Leaf,
        )
        Spacer(Modifier.width(4.dp))
        OvIconButton(
            icon = painterResource(R.drawable.ic_ov_legal),
            contentDescription = "The licences of PlugTalk",
            onClick = onLegal,
            tint = OnionPalette.AmberLight,
        )
    }
}

/** The small plate that shows the state of the transport. */
@Composable
private fun TorPill(state: BrowserUiState) {
    val tor = state.tor
    OvBadge(
        text = if (tor.ready) "TOR" else "${tor.percent}%",
        accent = if (tor.ready) OnionPalette.Good else OnionPalette.Amber,
        container = if (tor.ready) OnionPalette.Good.copy(alpha = 0.14f) else OnionPalette.Amber.copy(alpha = 0.14f),
    )
}

/**
 * The bar with the navigation and the address.
 *
 * @param canGoBack `true` when the back button is enabled.
 */
@Composable
private fun NavigationBar(
    address: String,
    onAddressChange: (String) -> Unit,
    onNavigate: () -> Unit,
    onBack: () -> Unit,
    onForward: () -> Unit,
    onReload: () -> Unit,
    onNewTab: () -> Unit,
    canGoBack: Boolean,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(OnionPalette.Surface)
            .padding(horizontal = 8.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OvIconButton(
            icon = painterResource(R.drawable.ic_ov_back),
            contentDescription = "Back",
            onClick = onBack,
            enabled = canGoBack,
        )
        Spacer(Modifier.width(6.dp))
        OvIconButton(
            icon = painterResource(R.drawable.ic_ov_forward),
            contentDescription = "Forward",
            onClick = onForward,
            tint = OnionPalette.Amber,
        )
        Spacer(Modifier.width(6.dp))
        OvIconButton(
            icon = painterResource(R.drawable.ic_ov_reload),
            contentDescription = "Reload",
            onClick = onReload,
            tint = OnionPalette.Amber,
        )
        Spacer(Modifier.width(6.dp))
        OvIconButton(
            icon = painterResource(R.drawable.ic_ov_new_tab),
            contentDescription = "New tab",
            onClick = onNewTab,
        )
        Spacer(Modifier.width(8.dp))
        OutlinedTextField(
            value = address,
            onValueChange = onAddressChange,
            modifier = Modifier.weight(1f),
            singleLine = true,
            shape = RoundedCornerShape(12.dp),
            placeholder = { Text("onion address", color = OnionPalette.TextMuted) },
            colors = OutlinedTextFieldDefaults.colors(
                focusedBorderColor = OnionPalette.Teal,
                unfocusedBorderColor = OnionPalette.Hairline,
                focusedContainerColor = OnionPalette.SurfaceRaised,
                unfocusedContainerColor = OnionPalette.SurfaceRaised,
                focusedTextColor = OnionPalette.TextPrimary,
                unfocusedTextColor = OnionPalette.TextPrimary,
                cursorColor = OnionPalette.Teal,
                focusedPlaceholderColor = OnionPalette.TextMuted,
                unfocusedPlaceholderColor = OnionPalette.TextMuted,
            ),
        )
        Spacer(Modifier.width(8.dp))
        OvPrimaryButton(text = "Go", onClick = onNavigate)
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
            .background(OnionPalette.SurfaceRaised),
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
                color = OnionPalette.TextMuted,
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
 * The colour of the chip tells what the action does: the amber encrypts, the
 * teal of the badge decrypts and the green of the leaves handles the keys.
 *
 * @param field the field the chip belongs to.
 * @param onClick the user tapped the chip.
 */
@Composable
private fun ActionChip(field: PgpFieldState, onClick: () -> Unit) {
    val kind = when (field.kind) {
        "pgp_message" -> ChipKind.Decrypt
        "pgp_public_key" -> ChipKind.Key
        else -> ChipKind.Encrypt
    }
    val icon = when (kind) {
        ChipKind.Encrypt -> R.drawable.ic_ov_encrypt
        ChipKind.Decrypt -> R.drawable.ic_ov_decrypt
        ChipKind.Key -> R.drawable.ic_ov_keys
    }
    OvActionChip(
        text = field.label,
        onClick = onClick,
        icon = painterResource(icon),
        kind = kind,
    )
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
            .background(OnionPalette.SurfaceRaised)
            .padding(horizontal = 10.dp, vertical = 6.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = if (tor.ready) "Tor ready" else "Tor: ${tor.percent}% ${tor.stage}",
                color = if (tor.ready) OnionPalette.Good else OnionPalette.TextMuted,
                fontSize = 12.sp,
                fontWeight = FontWeight.Medium,
                modifier = Modifier.weight(1f),
            )
            tor.isolationToken?.let {
                Text(
                    text = it,
                    color = OnionPalette.Teal,
                    fontSize = 11.sp,
                    fontFamily = FontFamily.Monospace,
                )
            }
        }
        if (!tor.ready) {
            Spacer(Modifier.height(5.dp))
            OvProgressBar(progress = tor.percent / 100f, modifier = Modifier.fillMaxWidth())
        }
        tor.circuit?.let { circuit ->
            Spacer(Modifier.height(6.dp))
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
    OvBadge(
        text = "${hop.role} ${hop.latencyMillis}ms",
        accent = OnionPalette.TextMuted,
    )
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
            .background(OnionPalette.SurfaceRaised)
            .padding(12.dp),
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = plaintext.title,
                    color = OnionPalette.TextPrimary,
                    fontSize = 16.sp,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.weight(1f),
                )
                SignatureBadge(plaintext)
            }
            Spacer(Modifier.height(8.dp))
            Box(
                modifier = Modifier
                    .height(120.dp)
                    .clip(RoundedCornerShape(10.dp))
                    .background(OnionPalette.Surface)
                    .padding(8.dp)
                    .verticalScroll(rememberScrollState()),
            ) {
                Text(
                    text = plaintext.body,
                    color = OnionPalette.TextPrimary,
                    fontSize = 14.sp,
                    fontFamily = FontFamily.Monospace,
                )
            }
            Spacer(Modifier.height(8.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
            ) {
                OvSecondaryButton(
                    text = "Insert into the page",
                    onClick = onInsert,
                    icon = painterResource(R.drawable.ic_ov_insert),
                )
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
    OvBadge(
        text = if (verified) {
            "good signature · ${plaintext.signerFingerprint.orEmpty().takeLast(16)}"
        } else {
            "signature not verified"
        },
        accent = if (verified) OnionPalette.Good else OnionPalette.Warn,
        container = if (verified) {
            OnionPalette.Good.copy(alpha = 0.14f)
        } else {
            OnionPalette.Warn.copy(alpha = 0.14f)
        },
    )
}

/**
 * The manager of the keys of the keyring.
 *
 * @param keys the keys of the keyring.
 * @param onCreate the user wants a new key.
 * @param onExport the user wants the public key of a certificate.
 * @param onDismiss the user closed the manager.
 */
@Composable
private fun KeyManagerDialog(
    keys: List<KeyInfo>,
    onCreate: () -> Unit,
    onExport: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = OnionPalette.SurfaceRaised,
        titleContentColor = OnionPalette.TextPrimary,
        textContentColor = OnionPalette.TextMuted,
        title = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Image(
                    painter = painterResource(R.drawable.ov_plugtalk_mark),
                    contentDescription = null,
                    modifier = Modifier.size(26.dp),
                )
                Spacer(Modifier.width(10.dp))
                Text(
                    text = "The keys of PlugTalk",
                    fontSize = 17.sp,
                    fontWeight = FontWeight.Bold,
                )
            }
        },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(max = 320.dp)
                    .verticalScroll(rememberScrollState()),
            ) {
                if (keys.isEmpty()) {
                    Text(
                        text = "There is no key yet. A key is needed to decrypt a " +
                            "message and to sign what is sent.",
                        fontSize = 13.sp,
                    )
                }
                keys.forEach { key ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = key.userId.ifEmpty { "the key of PlugTalk" },
                                color = OnionPalette.TextPrimary,
                                fontSize = 13.sp,
                                fontWeight = FontWeight.Medium,
                            )
                            Text(
                                text = key.fingerprint,
                                color = OnionPalette.TextMuted,
                                fontSize = 10.sp,
                                fontFamily = FontFamily.Monospace,
                            )
                        }
                        Spacer(Modifier.width(8.dp))
                        OvBadge(
                            text = if (key.hasSecret) "secret" else "public",
                            accent = if (key.hasSecret) OnionPalette.Good else OnionPalette.Amber,
                        )
                        Spacer(Modifier.width(6.dp))
                        OvIconButton(
                            icon = painterResource(R.drawable.ic_ov_keys),
                            contentDescription = "The public key of ${key.userId}",
                            onClick = { onExport(key.fingerprint) },
                            tint = OnionPalette.Teal,
                            size = 32.dp,
                        )
                    }
                }
            }
        },
        confirmButton = {
            OvPrimaryButton(
                text = "New key",
                onClick = onCreate,
                icon = painterResource(R.drawable.ic_ov_new_key),
            )
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Close", color = OnionPalette.TextMuted)
            }
        },
    )
}
