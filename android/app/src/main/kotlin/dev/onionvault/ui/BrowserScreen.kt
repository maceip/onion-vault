package dev.onionvault.ui

import androidx.activity.BackEventCompat
import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.input.ImeAction
import dev.onionvault.R
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The screen of the browser.
 *
 * The chrome is the one of the design board of Trap:
 *
 *   * the top bar carries the menu, the wordmark, the two onions of the brand,
 *     the number of the tabs and the menu of the browser,
 *   * the dock holds the tiles of the features that exist: the routing over
 *     Tor, the leak protection, the keyring and the licences,
 *   * the address is the pill of the board: the shield of the leak
 *     protection, the address and the button that clears it,
 *   * the navigation row carries the buttons of the history, the start page,
 *     a new tab and the reload.
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
 * @param onHome the user tapped the start page.
 * @param onCreateKey the user wants a new PGP key.
 * @param onExportKey the user wants the public key of a certificate.
 * @param onToggleRouting the user changed the traffic policy.
 * @param onZoomIn the user wants a bigger page.
 * @param onZoomOut the user wants a smaller page.
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
    onHome: () -> Unit = {},
    onCreateKey: () -> Unit = {},
    onExportKey: (String) -> Unit = {},
    onToggleRouting: (Boolean) -> Unit = {},
    onZoomIn: () -> Unit = {},
    onZoomOut: () -> Unit = {},
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
    var menuOpen by remember { mutableStateOf(false) }
    var tabsOpen by remember { mutableStateOf(false) }
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
        TopBar(
            state = state,
            onMenu = { menuOpen = true },
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

        Dock(
            state = state,
            onRouting = { onToggleRouting(!state.hiddenServiceOnly) },
            onKeys = { keysOpen = true },
            onLegal = { legalOpen = true },
        )

        AddressRow(
            address = address,
            onAddressChange = { address = it },
            onNavigate = { onNavigate(address) },
            onClear = { address = "" },
            blocked = state.blocked,
        )

        NavigationRow(
            canGoBack = state.canGoBack,
            onBack = onBack,
            onForward = onForward,
            onHome = onHome,
            onNewTab = {
                address = ""
                onNewTab()
            },
            onReload = onReload,
            onMenu = { menuOpen = true },
        )

        TorStatusBar(state = state)

        state.pgp.plaintext?.let { plaintext ->
            PlaintextSheet(
                plaintext = plaintext,
                onInsert = { onInsertPlaintext(plaintext.nodeId) },
            )
        }
    }

    if (menuOpen) {
        MenuSheet(
            state = state,
            onNewTab = {
                menuOpen = false
                address = ""
                onNewTab()
            },
            onTabs = {
                menuOpen = false
                tabsOpen = true
            },
            onKeys = {
                menuOpen = false
                keysOpen = true
            },
            onLegal = {
                menuOpen = false
                legalOpen = true
            },
            onToggleRouting = { enabled ->
                menuOpen = false
                onToggleRouting(enabled)
            },
            onZoomIn = {
                menuOpen = false
                onZoomIn()
            },
            onZoomOut = {
                menuOpen = false
                onZoomOut()
            },
            onDismiss = { menuOpen = false },
        )
    }

    if (tabsOpen) {
        TabsDialog(
            state = state,
            onDismiss = { tabsOpen = false },
        )
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
 * The top bar of the browser.
 *
 * It carries the menu, the wordmark of the brand, the two onions, the number
 * of the tabs and the menu of the browser.
 *
 * @param state the state of the browser.
 * @param onMenu the user wants the menu of the browser.
 */
@Composable
private fun TopBar(
    state: BrowserUiState,
    onMenu: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(OnionPalette.SurfaceRaised)
            .padding(start = 8.dp, end = 8.dp, top = 8.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OvNeonButton(
            icon = painterResource(R.drawable.ic_neon_menu),
            contentDescription = "The menu of Trap",
            onClick = onMenu,
        )
        Spacer(Modifier.width(10.dp))
        OvBrandMark(modifier = Modifier.weight(1f))
        TorPill(state = state)
        Spacer(Modifier.width(6.dp))
        OvTabBadge(count = state.tabCount)
        Spacer(Modifier.width(6.dp))
        OvNeonButton(
            icon = painterResource(R.drawable.ic_neon_dots),
            contentDescription = "The menu of Trap",
            onClick = onMenu,
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
        container = if (tor.ready) {
            OnionPalette.Good.copy(alpha = 0.14f)
        } else {
            OnionPalette.Amber.copy(alpha = 0.14f)
        },
    )
}

/**
 * The dock of the browser.
 *
 * The tiles are the ones of the board that have a real function behind them:
 * the routing over Tor, the leak protection, the keyring and the licences.
 * The wallet, the bookmarks, the downloads and the microphone of the board
 * are not shown: there is no feature behind them.
 *
 * @param state the state of the browser.
 * @param onRouting the user changed the traffic policy.
 * @param onKeys the user wants the key manager.
 * @param onLegal the user wants the licences.
 */
@Composable
private fun Dock(
    state: BrowserUiState,
    onRouting: () -> Unit,
    onKeys: () -> Unit,
    onLegal: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(OnionPalette.Surface)
            .padding(horizontal = 10.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.Top,
    ) {
        OvNeonTile(
            icon = painterResource(R.drawable.ic_neon_onion),
            label = "ONION",
            onClick = onRouting,
            active = state.hiddenServiceOnly,
            modifier = Modifier.width(62.dp),
        )
        OvNeonTile(
            icon = painterResource(R.drawable.ic_neon_key),
            label = "PGP",
            onClick = onKeys,
            modifier = Modifier.width(62.dp),
        )
        OvNeonTile(
            icon = painterResource(R.drawable.ic_ov_legal),
            label = "LICENCES",
            onClick = onLegal,
            modifier = Modifier.width(62.dp),
        )
    }
}

/**
 * The row with the address of the page.
 *
 * @param address the address that is shown.
 * @param onAddressChange the user typed in the address.
 * @param onNavigate the user wants to go to the address.
 * @param onClear the user cleared the address.
 * @param blocked `true` when the last request was blocked.
 */
@Composable
private fun AddressRow(
    address: String,
    onAddressChange: (String) -> Unit,
    onNavigate: () -> Unit,
    onClear: () -> Unit,
    blocked: Boolean,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(OnionPalette.Surface)
            .padding(horizontal = 10.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OvAddressPlate(
            modifier = Modifier.weight(1f),
            border = if (blocked) OnionPalette.Bad else OnionPalette.TealDeep,
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(start = 12.dp, end = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Image(
                    painter = painterResource(R.drawable.ic_neon_shield),
                    contentDescription = null,
                    contentScale = ContentScale.Fit,
                    modifier = Modifier.size(18.dp),
                )
                Spacer(Modifier.width(8.dp))
                BasicTextField(
                    value = address,
                    onValueChange = onAddressChange,
                    modifier = Modifier.weight(1f),
                    singleLine = true,
                    textStyle = TextStyle(
                        color = OnionPalette.TextPrimary,
                        fontSize = 13.sp,
                        fontFamily = FontFamily.Monospace,
                    ),
                    cursorBrush = SolidColor(OnionPalette.Mint),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go),
                    keyboardActions = KeyboardActions(onGo = { onNavigate() }),
                    decorationBox = { inner ->
                        if (address.isEmpty()) {
                            Text(
                                text = "onion address",
                                color = OnionPalette.TextMuted,
                                fontSize = 13.sp,
                                fontFamily = FontFamily.Monospace,
                            )
                        }
                        inner()
                    },
                )
                if (address.isNotEmpty()) {
                    OvNeonButton(
                        icon = painterResource(R.drawable.ic_neon_cancel),
                        contentDescription = "Clear the address",
                        onClick = onClear,
                        plate = 28.dp,
                        glyph = 16.dp,
                        accent = OnionPalette.Pink,
                    )
                }
            }
        }
    }
}

/**
 * The row with the buttons of the navigation.
 *
 * @param canGoBack `true` when the back button is enabled.
 */
@Composable
private fun NavigationRow(
    canGoBack: Boolean,
    onBack: () -> Unit,
    onForward: () -> Unit,
    onHome: () -> Unit,
    onNewTab: () -> Unit,
    onReload: () -> Unit,
    onMenu: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(OnionPalette.Surface)
            .padding(horizontal = 10.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OvNeonButton(
            icon = painterResource(R.drawable.ic_neon_back),
            contentDescription = "Back",
            onClick = onBack,
            enabled = canGoBack,
        )
        OvNeonButton(
            icon = painterResource(R.drawable.ic_neon_forward),
            contentDescription = "Forward",
            onClick = onForward,
        )
        OvNeonButton(
            icon = painterResource(R.drawable.ic_neon_home),
            contentDescription = "The start page of Trap",
            onClick = onHome,
        )
        OvNeonButton(
            icon = painterResource(R.drawable.ic_neon_new_tab),
            contentDescription = "New tab",
            onClick = onNewTab,
        )
        OvNeonButton(
            icon = painterResource(R.drawable.ic_neon_reload),
            contentDescription = "Reload",
            onClick = onReload,
        )
        OvNeonButton(
            icon = painterResource(R.drawable.ic_neon_dots),
            contentDescription = "The menu of Trap",
            onClick = onMenu,
        )
    }
}

/**
 * The menu of the browser.
 *
 * The menu is the sheet of the board: the entries that have a real function
 * behind them.
 *
 * @param state the state of the browser.
 * @param onNewTab the user wants a new tab.
 * @param onKeys the user wants the key manager.
 * @param onLegal the user wants the licences.
 * @param onToggleRouting the user changed the traffic policy.
 * @param onZoomIn the user wants a bigger page.
 * @param onZoomOut the user wants a smaller page.
 * @param onDismiss the user closed the menu.
 */
@Composable
private fun MenuSheet(
    state: BrowserUiState,
    onNewTab: () -> Unit,
    onTabs: () -> Unit,
    onKeys: () -> Unit,
    onLegal: () -> Unit,
    onToggleRouting: (Boolean) -> Unit,
    onZoomIn: () -> Unit,
    onZoomOut: () -> Unit,
    onDismiss: () -> Unit,
) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black.copy(alpha = 0.55f))
            .clickable(onClick = onDismiss),
        contentAlignment = Alignment.TopEnd,
    ) {
        Column(
            modifier = Modifier
                .padding(top = 54.dp, end = 10.dp)
                .width(250.dp)
                .clip(RoundedCornerShape(16.dp))
                .background(OnionPalette.SurfaceRaised)
                .border(1.dp, OnionPalette.TealDeep, RoundedCornerShape(16.dp))
                .padding(vertical = 6.dp),
        ) {
            OvMenuRow(
                icon = painterResource(R.drawable.ic_neon_new_tab),
                label = "New Tab",
                onClick = onNewTab,
            )
            OvMenuRow(
                icon = painterResource(R.drawable.ic_neon_new_partial_tab),
                label = "Tabs",
                onClick = onTabs,
                trailing = state.tabCount.toString(),
            )
            OvMenuRow(
                icon = painterResource(R.drawable.ic_neon_key),
                label = "PGP Keyring",
                onClick = onKeys,
            )
            OvMenuRow(
                icon = painterResource(R.drawable.ic_neon_shield),
                label = "Security",
                onClick = { onToggleRouting(!state.hiddenServiceOnly) },
                trailing = if (state.hiddenServiceOnly) "hidden services only" else "clearnet allowed",
            )
            OvMenuRow(
                icon = painterResource(R.drawable.ic_neon_zoom_in),
                label = "Zoom In",
                onClick = onZoomIn,
                trailing = "${(state.zoomLevel * 100).toInt()}%",
            )
            OvMenuRow(
                icon = painterResource(R.drawable.ic_neon_zoom_out),
                label = "Zoom Out",
                onClick = onZoomOut,
            )
            OvMenuRow(
                icon = painterResource(R.drawable.ic_ov_legal),
                label = "Licences",
                onClick = onLegal,
            )
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
 * teal of the brand decrypts and the mint handles the keys.
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
 * The overview of the tabs of the browser.
 *
 * The browser carries one tab: the page that is open.  The overview is the
 * sheet of the board that tells how many tabs are open and which page they
 * show.
 *
 * @param state the state of the browser.
 * @param onDismiss the user closed the overview.
 */
@Composable
private fun TabsDialog(
    state: BrowserUiState,
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
                    painter = painterResource(R.drawable.ic_neon_new_partial_tab),
                    contentDescription = null,
                    contentScale = ContentScale.Fit,
                    modifier = Modifier.size(26.dp),
                )
                Spacer(Modifier.width(10.dp))
                Text(
                    text = "The tabs of Trap",
                    fontSize = 17.sp,
                    fontWeight = FontWeight.Bold,
                )
            }
        },
        text = {
            Column(modifier = Modifier.fillMaxWidth()) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(OnionPalette.Surface)
                        .border(1.dp, OnionPalette.TealDeep, RoundedCornerShape(10.dp))
                        .padding(10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Image(
                        painter = painterResource(R.drawable.ic_neon_new_tab),
                        contentDescription = null,
                        contentScale = ContentScale.Fit,
                        modifier = Modifier.size(24.dp),
                    )
                    Spacer(Modifier.width(10.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = state.title.ifEmpty { "Trap" },
                            color = OnionPalette.TextPrimary,
                            fontSize = 13.sp,
                            fontWeight = FontWeight.Medium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            text = state.url.ifEmpty { "the start page of Trap" },
                            color = OnionPalette.TextMuted,
                            fontSize = 11.sp,
                            fontFamily = FontFamily.Monospace,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    Spacer(Modifier.width(8.dp))
                    OvBadge(
                        text = state.tabCount.toString(),
                        accent = OnionPalette.Mint,
                        container = OnionPalette.Teal.copy(alpha = 0.14f),
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text("Close", color = OnionPalette.TextMuted)
            }
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
                    painter = painterResource(R.drawable.ov_trap_mark),
                    contentDescription = null,
                    modifier = Modifier.size(26.dp),
                )
                Spacer(Modifier.width(10.dp))
                Text(
                    text = "The keys of Trap",
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
                                text = key.userId.ifEmpty { "the key of Trap" },
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
