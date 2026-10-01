package net.havenkeys.android.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CloudOff
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenTheme

/** The vault's top bar: where you are, whether writes can reach the server, and Lock. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HavenTopBar(title: String, online: Boolean, onLock: () -> Unit, modifier: Modifier = Modifier) {
    TopAppBar(
        title = { Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        actions = {
            if (!online) OfflineBadge()
            IconButton(onClick = onLock) {
                Icon(Icons.Outlined.Lock, contentDescription = stringResource(R.string.vault_lock_now))
            }
        },
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor = MaterialTheme.colorScheme.background,
            scrolledContainerColor = MaterialTheme.colorScheme.surfaceContainerLow,
            titleContentColor = HavenTheme.colors.textStrong,
            actionIconContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
        ),
        modifier = modifier,
    )
}

@Composable
private fun OfflineBadge() {
    Surface(
        shape = MaterialTheme.shapes.extraLarge,
        color = HavenTheme.colors.brassSoft,
        contentColor = MaterialTheme.colorScheme.onSurface,
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                Icons.Outlined.CloudOff,
                contentDescription = null,
                tint = HavenTheme.colors.brassInk,
                modifier = Modifier.size(16.dp),
            )
            Text(stringResource(R.string.vault_offline), style = MaterialTheme.typography.labelMedium)
        }
    }
}

@Preview
@Composable
private fun HavenTopBarPreview() {
    HavenTheme { HavenTopBar(title = "Vault", online = false, onLock = {}) }
}
