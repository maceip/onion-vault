package dev.onionvault

import android.content.Context
import android.util.Log
import dev.onionvault.ui.LegalDocument
import java.io.IOException

/**
 * The licence documents that travel with the application.
 *
 * The LGPL-3.0 of the engine requires that the recipient of the binary can
 * read the licence of the library: the texts are therefore packaged into the
 * APK and shown by the interface, instead of being a footnote in the
 * repository.
 *
 * The documents are read from the assets of the application.  The directory is
 * filled by the Gradle build: the texts of this repository are copied into it
 * and the continuous integration adds the notice of the engine, which belongs
 * to the exact build that is packaged into the APK.
 */
object LegalNotices {

    /** The directory of the assets that holds the documents. */
    private const val DIRECTORY = "licenses"

    /** The size of a block of text, in characters. */
    private const val BLOCK_SIZE = 4096

    /**
     * The titles of the documents that the application knows.
     *
     * A document that is not in this table is shown under its file name.
     */
    private val TITLES = mapOf(
        "LGPL-3.0.txt" to "GNU Lesser General Public License 3.0",
        "GPL-3.0.txt" to "GNU General Public License 3.0",
        "LGPL-2.1.txt" to "GNU Lesser General Public License 2.1",
        "CEFRIUM-NOTICE.txt" to "The notice of the CEF engine",
    )

    /**
     * The documents that are packaged into the application.
     *
     * The order of the list is the order in which the documents are shown:
     * the licences of the libraries first, the notice of the engine last.
     *
     * @param context the context of the application.
     */
    fun list(context: Context): List<LegalDocument> {
        val names = context.assets.list(DIRECTORY)?.toList().orEmpty()

        return names
            .filter { it.endsWith(".txt", ignoreCase = true) }
            .sortedWith(compareBy({ order(it) }, { it }))
            .map { name ->
                LegalDocument(
                    name = name,
                    title = titleOf(name),
                    sizeBytes = sizeOf(context, name),
                )
            }
    }

    /**
     * The text of a document, in blocks.
     *
     * The notice of the engine is 2.2 MiB long and has tens of thousands of
     * lines.  The text is therefore split into blocks of a few kilobytes, at
     * the end of a line, so that the reader does not have to keep one string
     * per line in memory and the list of the interface stays short.
     *
     * @param context the context of the application.
     * @param name the file name of the document.
     */
    fun read(context: Context, name: String): List<String> = try {
        context.assets.open("$DIRECTORY/$name").use { stream ->
            val blocks = ArrayList<String>()
            val block = StringBuilder()
            stream.bufferedReader().forEachLine { line ->
                block.append(line).append('\n')
                if (block.length >= BLOCK_SIZE) {
                    blocks.add(block.toString())
                    block.setLength(0)
                }
            }
            if (block.isNotEmpty()) {
                blocks.add(block.toString())
            }
            blocks
        }
    } catch (error: IOException) {
        Log.w(TAG, "cannot read the licence document $name", error)
        emptyList()
    }

    /** The title of a document. */
    fun titleOf(name: String): String = TITLES[name] ?: name

    /** The size of a document in bytes. */
    private fun sizeOf(context: Context, name: String): Long = try {
        // The documents are compressed inside the APK, so their length cannot
        // be asked for; the stream is read once and only counted.
        var total = 0L
        val buffer = ByteArray(16 * 1024)
        context.assets.open("$DIRECTORY/$name").use { stream ->
            while (true) {
                val read = stream.read(buffer)
                if (read <= 0) {
                    break
                }
                total += read
            }
        }
        total
    } catch (error: IOException) {
        Log.w(TAG, "cannot measure the licence document $name", error)
        0L
    }

    /**
     * The order of a document.
     *
     * The licences come first, in the order of their versions, and the notice
     * of the engine comes last.
     */
    private fun order(name: String): Int = when (name) {
        "LGPL-2.1.txt" -> 0
        "GPL-3.0.txt" -> 1
        "LGPL-3.0.txt" -> 2
        else -> 3
    }

    /** The tag of the log of this object. */
    private const val TAG = "OvLegalNotices"
}
