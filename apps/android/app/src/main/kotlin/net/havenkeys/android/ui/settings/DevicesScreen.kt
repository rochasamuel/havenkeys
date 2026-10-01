package net.havenkeys.android.ui.settings

import android.text.format.DateUtils
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.OffsetDateTime
import java.time.format.DateTimeParseException
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.HavenTopBar
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.theme.HavenTheme
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

    Scaffold(
        topBar = {
            HavenTopBar(stringResource(R.string.devices_title), online = online, onLock = onLock, onBack = onBack)
        },
        containerColor = MaterialTheme.colorScheme.background,
        modifier = modifier,
    ) { padding ->
        LazyColumn(Modifier.padding(padding).fillMaxSize()) {
            state.errorCode?.let { code ->
                item {
                    Text(
                        stringResource(errorText(code)),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    )
                }
            }
            if (state.devices.isEmpty() && !state.loading && state.errorCode == null) {
                item { Text(stringResource(R.string.devices_empty), modifier = Modifier.padding(16.dp)) }
            }
            items(state.devices, key = { it.id }) { device ->
                DeviceRow(device, onRevoke = { revoking = device })
            }
        }
    }

    revoking?.let { device ->
        AlertDialog(
            onDismissRequest = { revoking = null },
            text = {
                Text(
                    if (device.current) {
                        stringResource(R.string.devices_revoke_this)
                    } else {
                        stringResource(R.string.devices_revoke_confirm, device.name)
                    },
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    revoking = null
                    viewModel.revoke(device.id)
                }) { Text(stringResource(R.string.devices_revoke)) }
            },
            dismissButton = {
                TextButton(onClick = { revoking = null }) { Text(stringResource(R.string.settings_cancel)) }
            },
        )
    }
}

@Composable
private fun DeviceRow(device: DeviceInfo, onRevoke: () -> Unit) {
    val seen = device.lastSeenAt?.let(::relativeTime)
    ListItem(
        headlineContent = { Text(device.name) },
        overlineContent = if (device.current) {
            { Text(stringResource(R.string.devices_this_phone), color = HavenTheme.colors.brassInk) }
        } else {
            null
        },
        supportingContent = {
            Text(
                if (seen != null) {
                    stringResource(R.string.devices_last_seen, seen)
                } else {
                    stringResource(R.string.devices_never_seen)
                },
            )
        },
        trailingContent = { TextButton(onClick = onRevoke) { Text(stringResource(R.string.devices_revoke)) } },
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.background),
    )
}

/** "5 minutes ago" in the phone's language; the server's text as is if it is not RFC 3339. */
@Suppress("SwallowedException")
private fun relativeTime(rfc3339: String): String = try {
    val millis = OffsetDateTime.parse(rfc3339).toInstant().toEpochMilli()
    DateUtils.getRelativeTimeSpanString(millis, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS).toString()
} catch (e: DateTimeParseException) {
    rfc3339
}
