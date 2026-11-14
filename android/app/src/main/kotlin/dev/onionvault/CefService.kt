package dev.onionvault

import android.app.Service
import android.content.Intent
import android.os.IBinder
import android.util.Log

/**
 * The CEF subprocesses of Trap.
 *
 * Android does not allow an application to fork a helper process: every
 * process has to be started through the ActivityManager.  CEF's Android
 * port therefore starts the renderer, the GPU and the utility process
 * through a service; the service runs the CEF main loop of the subprocess
 * with the command line that CEF prepared.
 *
 * The manifest declares the service once per process type:
 *
 * ```xml
 * <service android:name=".CefService" android:process=":cef_render" />
 * ```
 */
open class CefService : Service() {

    private companion object {
        const val TAG = "OvCefService"

        /** The command line of the subprocess. */
        const val EXTRA_ARGUMENTS = "arguments"

        /** The command line of the subprocess (alternative name). */
        const val EXTRA_SWITCHES = "switches"

        /** The type of the process (`renderer`, `gpu`, `utility`, ...). */
        const val EXTRA_PROCESS_TYPE = "process_type"
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val arguments = intent?.getStringArrayExtra(EXTRA_ARGUMENTS)
            ?: intent?.getStringArrayExtra(EXTRA_SWITCHES)
            ?: emptyArray()

        val processType = intent?.getStringExtra(EXTRA_PROCESS_TYPE)
            ?: processTypeOf(arguments)

        Log.i(TAG, "starting the CEF subprocess '$processType'")

        val result = OvNative.nativeRunSubprocess(processType, arguments)
        if (result != 0) {
            Log.e(TAG, "the CEF subprocess '$processType' failed with $result")
        }

        // The subprocess is done; the service has no reason to stay alive.
        stopSelf(startId)
        return START_NOT_STICKY
    }

    /** Read the process type out of the command line. */
    private fun processTypeOf(arguments: Array<String>): String {
        for (index in arguments.indices) {
            if (arguments[index] == "--type" && index + 1 < arguments.size) {
                return arguments[index + 1]
            }
            if (arguments[index].startsWith("--type=")) {
                return arguments[index].substringAfter('=')
            }
        }
        return ""
    }
}

/**
 * The renderer process of CEF.
 *
 * Android identifies a service by its class name, so every process type
 * needs its own class even though they all run the same code.
 */
class CefRenderService : CefService()

/** The GPU process of CEF. */
class CefGpuService : CefService()

/** The utility process of CEF. */
class CefUtilityService : CefService()
