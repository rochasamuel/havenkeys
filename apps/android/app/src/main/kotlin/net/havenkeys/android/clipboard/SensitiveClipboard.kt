package net.havenkeys.android.clipboard

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.PersistableBundle
import java.util.UUID
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.launch
import net.havenkeys.android.data.VaultEvent

/**
 * Copy marked sensitive (hidden from the clipboard preview and keyboard
 * suggestions) and cleared after the vault's delay or on lock, unless
 * something else was copied since. Android 10+ hides the clipboard from
 * background apps; when the description cannot be read, the clip is cleared
 * anyway, which may remove a newer copy made in another app: the safe side
 * for a secret.
 */
class SensitiveClipboard internal constructor(
    private val clipboard: () -> ClipboardManager,
    private val scope: CoroutineScope,
) {
    constructor(context: Context, scope: CoroutineScope) :
        this({ context.getSystemService(ClipboardManager::class.java) }, scope)

    private val manager get() = clipboard()
    private val lock = Any()
    private var ours: String? = null
    private var timer: Job? = null

    /** Only ever called from an explicit tap on Copy. */
    fun copy(label: String, value: String, clearAfterSeconds: Int) {
        val token = UUID.randomUUID().toString()
        val clip = ClipData.newPlainText(label, value)
        clip.description.extras = PersistableBundle().apply {
            putBoolean(IS_SENSITIVE, true)
            putString(TOKEN, token)
        }
        synchronized(lock) {
            timer?.cancel()
            manager.setPrimaryClip(clip)
            ours = token
            timer = scope.launch {
                delay(clearAfterSeconds * MILLIS_PER_SECOND)
                clearIfOurs()
            }
        }
    }

    /**
     * Clears the clipboard if it still holds (or may hold) the last value we
     * copied. Runs from the timer and the lock collector in the app's scope,
     * where a clipboard error must not end the process; nothing is logged.
     */
    @Suppress("SwallowedException", "TooGenericExceptionCaught")
    fun clearIfOurs() {
        synchronized(lock) {
            timer?.cancel()
            timer = null
            try {
                val current = manager.primaryClipDescription?.extras?.getString(TOKEN)
                if (stillOurs(current, ours)) manager.clearPrimaryClip()
            } catch (e: RuntimeException) {
                Unit
            }
            ours = null
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

/**
 * Whether to clear: only while a copy of ours is outstanding, and then when
 * the clipboard still shows our token or cannot be read ([current] null).
 */
internal fun stillOurs(current: String?, ours: String?): Boolean =
    ours != null && (current == null || current == ours)

/** The lock, signing out and removing the vault clear our clip (CLAUDE.md §15), as on the desktop. */
internal suspend fun clearClipboardOnLock(events: Flow<VaultEvent>, clear: () -> Unit) {
    events.collect { event ->
        when (event) {
            is VaultEvent.Locked, VaultEvent.SignedOut, VaultEvent.Removed -> clear()
            else -> Unit
        }
    }
}
