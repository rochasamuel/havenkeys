package net.havenkeys.android.ui.autofillsetup

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.provider.Settings
import android.view.autofill.AutofillManager
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import androidx.lifecycle.compose.LifecycleResumeEffect
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.HavenTopBar
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * Whether HavenKeys is Android's autofill service, a way to choose it, and
 * how to make Chrome use it. Checked again each time the screen resumes, so
 * coming back from Android's settings shows the new state.
 */
@Composable
fun AutofillSetupScreen(online: Boolean, onBack: () -> Unit, onLock: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    var enabled by remember { mutableStateOf(isOurAutofillService(context)) }
    var openFailed by remember { mutableStateOf(false) }
    LifecycleResumeEffect(context) {
        enabled = isOurAutofillService(context)
        onPauseOrDispose { }
    }

    Scaffold(
        topBar = {
            HavenTopBar(
                stringResource(R.string.autofill_setup_title),
                online = online,
                onLock = onLock,
                onBack = onBack,
            )
        },
        containerColor = MaterialTheme.colorScheme.background,
        modifier = modifier,
    ) { padding ->
        Column(
            Modifier
                .padding(padding)
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            ServiceState(enabled)
            if (!enabled) {
                Button(
                    onClick = { openFailed = !requestAutofillService(context) },
                    modifier = Modifier.fillMaxWidth(),
                ) { Text(stringResource(R.string.autofill_setup_open)) }
            }
            if (openFailed) {
                Text(stringResource(R.string.autofill_setup_unavailable), color = MaterialTheme.colorScheme.error)
            }
            Text(
                stringResource(R.string.autofill_setup_chrome_title),
                style = MaterialTheme.typography.titleSmall,
                color = HavenTheme.colors.textStrong,
            )
            Text(
                stringResource(R.string.autofill_setup_chrome),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun ServiceState(enabled: Boolean) {
    Surface(
        color = if (enabled) MaterialTheme.colorScheme.surfaceContainerLow else HavenTheme.colors.brassSoft,
        shape = MaterialTheme.shapes.medium,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.padding(16.dp),
        ) {
            Icon(
                if (enabled) Icons.Outlined.CheckCircle else Icons.Outlined.Info,
                contentDescription = null,
                tint = HavenTheme.colors.brassInk,
            )
            Text(
                stringResource(if (enabled) R.string.autofill_setup_on else R.string.autofill_setup_off),
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}

/** True only when the chosen autofill service is this app's. */
private fun isOurAutofillService(context: Context): Boolean =
    context.getSystemService(AutofillManager::class.java)?.hasEnabledAutofillServices() == true

/** False when no settings screen answers the request (some phones remove it). */
@Suppress("SwallowedException")
private fun requestAutofillService(context: Context): Boolean = try {
    context.startActivity(
        Intent(Settings.ACTION_REQUEST_SET_AUTOFILL_SERVICE, "package:${context.packageName}".toUri()),
    )
    true
} catch (e: ActivityNotFoundException) {
    false
}
