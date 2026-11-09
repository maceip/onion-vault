package dev.onionvault

import android.content.Context
import android.util.Log
import java.io.File

/**
 * The CEF resources of the application.
 *
 * CEF needs its resource files (the pak files with the translations, the
 * ICU data and the locale files) on the file system.  They are shipped in
 * the assets of the APK and unpacked into the private directory of the
 * application, because Chromium maps them into the memory and cannot read
 * them from a compressed archive.
 */
object CefResources {

    private const val TAG = "OvCefResources"

    /** The directory inside the assets that holds the CEF resources. */
    private const val ASSET_DIRECTORY = "cef"

    /** The directory inside the assets that holds the locale files. */
    private const val LOCALES_DIRECTORY = "locales"

    /** The version marker of the unpacked resources. */
    private const val MARKER = ".onionvault-resources"

    /**
     * Unpack the CEF resources and return the directory that holds them.
     *
     * The directory is the `cef` subdirectory of the private files
     * directory; the engine passes it to `CefSettings.resources_dir_path`.
     */
    fun unpack(context: Context): File {
        val target = File(context.filesDir, "cef")
        val marker = File(target, MARKER)

        val expected = versionOf(context)
        if (marker.isFile && marker.readText() == expected) {
            Log.i(TAG, "the CEF resources are already unpacked")
            return target
        }

        target.deleteRecursively()
        target.mkdirs()

        val assets = context.assets.list(ASSET_DIRECTORY) ?: emptyArray()
        var copied = 0
        for (name in assets) {
            // The locale files live in a directory of their own; they are
            // unpacked separately, a directory cannot be copied as a file.
            if (name == LOCALES_DIRECTORY) {
                continue
            }
            copyAsset(context, "$ASSET_DIRECTORY/$name", File(target, name))
            copied++
        }

        val locales = context.assets.list("$ASSET_DIRECTORY/$LOCALES_DIRECTORY") ?: emptyArray()
        for (name in locales) {
            copyAsset(
                context,
                "$ASSET_DIRECTORY/$LOCALES_DIRECTORY/$name",
                File(target, "$LOCALES_DIRECTORY/$name"),
            )
            copied++
        }

        marker.writeText(expected)
        Log.i(TAG, "unpacked $copied CEF resource files")
        return target
    }

    /** The directory that holds the locale files. */
    fun locales(context: Context): File = File(context.filesDir, "cef/locales")

    /** Copy a single asset. */
    private fun copyAsset(context: Context, asset: String, target: File) {
        target.parentFile?.mkdirs()
        context.assets.open(asset).use { input ->
            target.outputStream().use { output -> input.copyTo(output) }
        }
    }

    /**
     * The version of the resources.
     *
     * The modification time of the asset directory is used, so that a new
     * build unpacks its resources again.
     */
    private fun versionOf(context: Context): String {
        val assets = context.assets.list(ASSET_DIRECTORY) ?: emptyArray()
        // The timestamp of the installed package changes with every update,
        // so a new build always unpacks its resources again.
        val stamp = try {
            context.packageManager.getPackageInfo(context.packageName, 0).lastUpdateTime
        } catch (error: Throwable) {
            0L
        }
        val locales =
            context.assets.list("$ASSET_DIRECTORY/$LOCALES_DIRECTORY") ?: emptyArray()
        return "$stamp:${assets.sorted().joinToString(",")}:${locales.sorted().joinToString(",")}"
    }
}
