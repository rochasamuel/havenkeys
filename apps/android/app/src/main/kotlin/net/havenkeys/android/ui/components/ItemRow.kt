package net.havenkeys.android.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.StickyNote2
import androidx.compose.material.icons.outlined.Badge
import androidx.compose.material.icons.outlined.CreditCard
import androidx.compose.material.icons.outlined.Key
import androidx.compose.material.icons.outlined.Timer
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

/**
 * One vault item in a list: what it is, its title and its non-secret
 * subtitle. [titleModifier] lets the title travel to the item's screen.
 */
@Composable
fun ItemRow(
    summary: ItemSummary,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    titleModifier: Modifier = Modifier,
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    ListItem(
        headlineContent = {
            Text(summary.title, titleModifier, maxLines = 1, overflow = TextOverflow.Ellipsis)
        },
        supportingContent = (summary.subtitle ?: summary.website)?.let { line ->
            { Text(line, maxLines = 1, overflow = TextOverflow.Ellipsis) }
        },
        leadingContent = { ItemBadge(summary) },
        trailingContent = if (summary.hasPasskey || summary.hasTotp) {
            {
                Row {
                    if (summary.hasPasskey) {
                        Icon(Icons.Outlined.Key, stringResource(R.string.item_passkey), Modifier.size(18.dp), muted)
                    }
                    if (summary.hasTotp) {
                        Icon(Icons.Outlined.Timer, stringResource(R.string.field_totp), Modifier.size(18.dp), muted)
                    }
                }
            }
        } else {
            null
        },
        colors = ListItemDefaults.colors(
            containerColor = MaterialTheme.colorScheme.background,
            headlineColor = HavenTheme.colors.textStrong,
            supportingColor = muted,
        ),
        modifier = modifier.clickable(onClick = onClick),
    )
}

/** A login shows its title's initial; the other kinds show what they are. */
@Composable
private fun ItemBadge(summary: ItemSummary) {
    Box(
        modifier = Modifier
            .size(40.dp)
            .clip(MaterialTheme.shapes.small)
            .background(HavenTheme.colors.avatarBg)
            .clearAndSetSemantics {},
        contentAlignment = Alignment.Center,
    ) {
        val tint = HavenTheme.colors.avatarFg
        val icon = when (summary.kind) {
            ItemKind.LOGIN -> null
            ItemKind.SECURE_NOTE -> Icons.AutoMirrored.Outlined.StickyNote2
            ItemKind.CARD -> Icons.Outlined.CreditCard
            ItemKind.IDENTITY -> Icons.Outlined.Badge
        }
        if (icon == null) {
            Text(
                text = summary.title.trim().take(1).uppercase(),
                style = MaterialTheme.typography.titleMedium,
                color = tint,
            )
        } else {
            Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(20.dp))
        }
    }
}

@Preview
@Composable
private fun ItemRowPreview() {
    HavenTheme {
        ItemRow(
            summary = ItemSummary(
                id = "00000000-0000-0000-0000-000000000000",
                kind = ItemKind.LOGIN,
                title = "Example",
                subtitle = "user@example.com",
                website = "https://example.com",
                hasTotp = true,
                hasPasskey = true,
                updatedAt = 0,
            ),
            onClick = {},
        )
    }
}
