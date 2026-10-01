package net.havenkeys.android.clipboard

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.PersistableBundle
import java.util.UUID
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Copy marked sensitive (hidden from the clipboard preview and keyboard
 * suggestions) and cleared after the vault's delay, unless something else
 * was copied since. Android 10+ hides the clipboard from background apps;
 * when the description cannot be read, the clip is cleared anyway, which may
 * remove a newer copy made in another app: the safe side for a secret.
 */
class SensitiveClipboard(private val context: Context, private val scope: CoroutineScope) {
    private val manager get() = context.getSystemService(ClipboardManager::class.java)

    /** Only ever called from an explicit tap on Copy. */
    fun copy(label: String, value: String, clearAfterSeconds: Int) {
        val token = UUID.randomUUID().toString()
        val clip = ClipData.newPlainText(label, value)
        clip.description.extras = PersistableBundle().apply {
            putBoolean(IS_SENSITIVE, true)
            putString(TOKEN, token)
        }
        manager.setPrimaryClip(clip)
        scope.launch {
            delay(clearAfterSeconds * MILLIS_PER_SECOND)
            val current = manager.primaryClipDescription?.extras?.getString(TOKEN)
            if (current == null || current == token) manager.clearPrimaryClip()
        }
    }

    private companion object {
        // ClipDescription.EXTRA_IS_SENSITIVE from Android 13; the same key is
        // the documented way to mark a clip sensitive on older versions.
        const val IS_SENSITIVE = "android.content.extra.IS_SENSITIVE"
        const val TOKEN = "net.havenkeys.clip"
        const val MILLIS_PER_SECOND = 1_000L
    }
}
