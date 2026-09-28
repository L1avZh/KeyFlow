package app.keyflow.mobile.util

import android.content.ClipData
import android.content.ClipDescription
import android.content.Context
import android.os.Build
import android.os.PersistableBundle
import androidx.core.content.getSystemService
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Copies a secret to the clipboard and, best-effort, clears it again
 * after [clearAfterSeconds] — the same feature as the desktop app's
 * "clear clipboard after copying a secret" setting. This is
 * best-effort, not a guarantee: Android has no API to force-clear
 * another app's clipboard read, and on Android 13+ the OS itself may
 * already show a "content copied" toast and clipboard access is
 * restricted for background apps, which is a stronger mitigation than
 * anything this app can add. Marking the clip
 * `EXTRA_IS_SENSITIVE`/`isSensitive` also asks Android 13+ to skip
 * showing a preview of the copied text in its own clipboard-copy toast.
 */
object ClipboardUtil {
    fun copyWithAutoClear(context: Context, label: String, secret: String, clearAfterSeconds: Int, scope: CoroutineScope) {
        val clipboard = context.getSystemService<android.content.ClipboardManager>() ?: return
        val clip = ClipData.newPlainText(label, secret)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            clip.description.extras = PersistableBundle().apply { putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true) }
        }
        clipboard.setPrimaryClip(clip)

        if (clearAfterSeconds <= 0) return
        scope.launch {
            delay(clearAfterSeconds * 1000L)
            // Only clear if our own copy is still the current clip —
            // never stomp on something the user copied from elsewhere
            // afterward.
            val current = clipboard.primaryClip
            val stillOurs = current != null && current.itemCount > 0 && current.getItemAt(0).text == secret
            if (stillOurs) {
                clipboard.setPrimaryClip(ClipData.newPlainText("", ""))
            }
        }
    }
}
