package dev.onionvault.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The list of the licence documents that travel with the application.
 *
 * The LGPL-3.0 of the engine asks for the recipient of the binary to be able
 * to read the licence of the library.  The documents are therefore shown by
 * the application itself and not only shipped in the archive of the release.
 *
 * @param documents the documents that are packaged into the application.
 * @param onOpenDocument the user wants to read a document.
 * @param onDismiss the user closed the list.
 */
@Composable
fun LegalNoticesDialog(
    documents: List<LegalDocument>,
    onOpenDocument: (LegalDocument) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = OnionPalette.SurfaceRaised,
        titleContentColor = OnionPalette.TextPrimary,
        textContentColor = OnionPalette.TextMuted,
        title = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = "The licences of OnionVault",
                    fontSize = 17.sp,
                    fontWeight = FontWeight.Bold,
                )
            }
        },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(max = 320.dp),
            ) {
                Text(
                    text = "The engine of the browser is the CEF port of " +
                        "Cefrium, which is free software under the LGPL-3.0-or-later. " +
                        "It is loaded as a shared library and can be replaced.",
                    fontSize = 12.sp,
                    modifier = Modifier.padding(bottom = 10.dp),
                )
                if (documents.isEmpty()) {
                    Text(
                        text = "The documents are not available in this build.",
                        fontSize = 13.sp,
                    )
                } else {
                    documents.forEach { document ->
                        DocumentRow(document = document, onClick = { onOpenDocument(document) })
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Close", color = OnionPalette.TextMuted)
            }
        },
    )
}

/**
 * A row of the list of the licence documents.
 *
 * @param document the document of the row.
 * @param onClick the user tapped the row.
 */
@Composable
private fun DocumentRow(document: LegalDocument, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .clickable(onClick = onClick)
            .padding(vertical = 8.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = document.title,
                color = OnionPalette.TextPrimary,
                fontSize = 13.sp,
                fontWeight = FontWeight.Medium,
            )
            if (document.sizeBytes > 0) {
                Text(
                    text = formatSize(document.sizeBytes),
                    color = OnionPalette.TextMuted,
                    fontSize = 10.sp,
                    fontFamily = FontFamily.Monospace,
                )
            }
        }
        Spacer(Modifier.width(8.dp))
        Text(
            text = "read",
            color = OnionPalette.Teal,
            fontSize = 12.sp,
            fontWeight = FontWeight.SemiBold,
        )
    }
}

/**
 * The text of a licence document.
 *
 * The text of the notice of the engine has tens of thousands of lines; they
 * are shown in a list that only builds the visible rows.
 *
 * @param document the document that is read.
 * @param loadLines the function that reads the document.
 * @param onDismiss the user closed the document.
 */
@Composable
fun LegalDocumentDialog(
    document: LegalDocument,
    loadLines: suspend () -> List<String>,
    onDismiss: () -> Unit,
) {
    var lines by remember(document.name) { mutableStateOf<List<String>?>(null) }

    LaunchedEffect(document.name) {
        lines = withContext(Dispatchers.IO) { loadLines() }
    }

    Dialog(onDismissRequest = onDismiss) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(max = 520.dp)
                .clip(RoundedCornerShape(14.dp))
                .background(OnionPalette.SurfaceRaised)
                .padding(14.dp),
        ) {
            Text(
                text = document.title,
                color = OnionPalette.TextPrimary,
                fontSize = 15.sp,
                fontWeight = FontWeight.Bold,
            )
            Spacer(Modifier.height(8.dp))

            val content = lines
            if (content == null) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(120.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    CircularProgressIndicator(color = OnionPalette.Teal)
                }
            } else {
                val listState = rememberLazyListState()
                LazyColumn(
                    state = listState,
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f, fill = false),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    items(content) { line ->
                        Text(
                            text = line,
                            color = OnionPalette.TextPrimary,
                            fontSize = 11.sp,
                            fontFamily = FontFamily.Monospace,
                        )
                    }
                }
            }

            Spacer(Modifier.height(10.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
            ) {
                OvSecondaryButton(text = "Close", onClick = onDismiss)
            }
        }
    }
}

/**
 * The size of a document in a form that a person can read.
 *
 * @param bytes the size in bytes.
 */
private fun formatSize(bytes: Long): String = when {
    bytes >= 1024L * 1024L -> "%.1f MiB".format(bytes / (1024.0 * 1024.0))
    bytes >= 1024L -> "%d KiB".format(bytes / 1024L)
    else -> "$bytes bytes"
}
