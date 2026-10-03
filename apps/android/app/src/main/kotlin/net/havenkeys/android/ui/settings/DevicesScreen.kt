package net.havenkeys.android.ui.settings

import android.text.format.DateUtils
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.OffsetDateTime
import java.time.format.DateTimeParseException
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.ScreenBar
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.Pill
import net.havenkeys.android.ui.shell.EmptyLine
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.shell.insetGroup
import net.havenkeys.android.ui.theme.HavenSpacing
import uniffi.havenkeys_mobile.DeviceInfo

/** The account's devices, this one marked; Revoke ends a device's session on the server. */
@Composable
fun DevicesScreen(
    viewModel: DevicesViewModel,
    online: Boolean,
    onBack: () -> Unit,
    onLock: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var revoking by remember { mutableStateOf<DeviceInfo?>(null) }
    val gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

    HavenScaffold(
        modifier = modifier,
        topBar = { ScreenBar(onBack = onBack, online = online, onLock = onLock) },
    ) { padding ->
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            item(key = "title") { LargeTitle(stringResource(R.string.devices_title), gutter) }
            state.errorCode?.let { code -> item(key = "error") { ErrorLine(code, gutter) } }
            if (state.devices.isEmpty() && !state.loading && state.errorCode == null) {
                item(key = "empty") { EmptyLine(stringResource(R.string.devices_empty), gutter) }
            }
            insetGroup(state.devices, key = { it.id }) { device -> DeviceRow(device, onRevoke = { revoking = device }) }
        }
    }

    revoking?.let { device ->
        HavenDialog(
            title = if (device.current) {
                stringResource(R.string.devices_revoke_this)
            } else {
                stringResource(R.string.devices_revoke_confirm, device.name)
            },
            onDismiss = { revoking = null },
            confirm = DialogAction(
                stringResource(R.string.devices_revoke),
                {
                    revoking = null
                    viewModel.revoke(device.id)
                },
                danger = true,
            ),
            dismiss = DialogAction(stringResource(R.string.settings_cancel), { revoking = null }),
        )
    }
}

@Composable
private fun DeviceRow(device: DeviceInfo, onRevoke: () -> Unit) {
    val seen = device.lastSeenAt?.let(::relativeTime)
    GroupRow(
        trailing = {
            HavenButton(stringResource(R.string.devices_revoke), onClick = onRevoke, style = ButtonStyle.Quiet)
        },
    ) {
        if (device.current) Pill(stringResource(R.string.devices_this_phone))
        GroupRowText(
            device.name,
            if (seen != null) {
                stringResource(R.string.devices_last_seen, seen)
            } else {
                stringResource(R.string.devices_never_seen)
            },
        )
    }
}

/** "5 minutes ago" in the phone's language; the server's text as is if it is not RFC 3339. */
@Suppress("SwallowedException")
private fun relativeTime(rfc3339: String): String = try {
    val millis = OffsetDateTime.parse(rfc3339).toInstant().toEpochMilli()
    DateUtils.getRelativeTimeSpanString(millis, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS).toString()
} catch (e: DateTimeParseException) {
    rfc3339
}
