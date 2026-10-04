package net.havenkeys.android.ui.edit

import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.kit.HavenSheet
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.onboarding.KitScanner
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.LumaFrame

/**
 * The camera in a sheet, looking for the QR code a site shows when
 * two-factor authentication is turned on. Frames go to Rust one at a time
 * (any that arrive meanwhile are wiped and dropped), and Rust returns only an
 * `otpauth://totp` link: whatever else a code says never leaves it. The first
 * link goes to [onFound], once; the caller puts it in the field and closes.
 */
@Composable
internal fun TotpScanSheet(
    scan: suspend (LumaFrame) -> Outcome<String?>,
    onFound: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val busy = remember { AtomicBoolean(false) }
    val found = remember { AtomicBoolean(false) }
    val latestFound by rememberUpdatedState(onFound)
    val onFrame: (LumaFrame) -> Unit = { frame ->
        if (found.get() || !busy.compareAndSet(false, true)) {
            frame.bytes.fill(0)
        } else {
            scope.launch {
                try {
                    val link = (scan(frame) as? Outcome.Ok)?.value
                    if (link != null && found.compareAndSet(false, true)) latestFound(link)
                } finally {
                    busy.set(false)
                }
            }
        }
    }
    HavenSheet(onDismiss = onDismiss, title = stringResource(R.string.edit_totp_scan)) {
        HavenText(
            stringResource(R.string.edit_totp_scan_sub),
            Modifier.padding(bottom = 16.dp),
            color = HavenTheme.colors.muted,
        )
        KitScanner(
            onFrame = onFrame,
            modifier = Modifier.fillMaxWidth().aspectRatio(1f).clip(HavenShape.group),
            cameraNeeded = stringResource(R.string.edit_totp_camera_needed),
            cameraUnavailable = stringResource(R.string.edit_totp_camera_unavailable),
        )
    }
}
