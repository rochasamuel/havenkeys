package net.havenkeys.android.ui.autofillsetup

import android.content.ActivityNotFoundException
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.provider.Settings
import android.view.autofill.AutofillManager
import androidx.annotation.RequiresApi
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
import net.havenkeys.android.credentials.HavenCredentialService
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
    var passkeysOn by remember { mutableStateOf(isOurCredentialProvider(context)) }
    var openFailed by remember { mutableStateOf(false) }
    var providerOpenFailed by remember { mutableStateOf(false) }
    LifecycleResumeEffect(context) {
        enabled = isOurAutofillService(context)
        passkeysOn = isOurCredentialProvider(context)
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
            PasskeysSection(passkeysOn, providerOpenFailed) { providerOpenFailed = !openProviderSettings(context) }
        }
    }
}

@Composable
private fun PasskeysSection(passkeysOn: Boolean, openFailed: Boolean, onOpen: () -> Unit) {
    Text(
        stringResource(R.string.autofill_setup_passkeys_title),
        style = MaterialTheme.typography.titleSmall,
        color = HavenTheme.colors.textStrong,
    )
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
        Text(
            stringResource(R.string.autofill_setup_passkeys_old),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    } else {
        Text(
            stringResource(
                if (passkeysOn) R.string.autofill_setup_passkeys_on else R.string.autofill_setup_passkeys_off,
            ),
            style = MaterialTheme.typography.bodyMedium,
        )
        if (!passkeysOn) {
            Button(
                onClick = onOpen,
                modifier = Modifier.fillMaxWidth(),
            ) { Text(stringResource(R.string.autofill_setup_passkeys_open)) }
        }
        if (openFailed) {
            Text(stringResource(R.string.autofill_setup_passkeys_unavailable), color = MaterialTheme.colorScheme.error)
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

private fun isOurCredentialProvider(context: Context): Boolean =
    Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE &&
        context.getSystemService(android.credentials.CredentialManager::class.java)
            ?.isEnabledCredentialProviderService(ComponentName(context, HavenCredentialService::class.java)) == true

private fun openProviderSettings(context: Context): Boolean =
    Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE && openCredentialProviderSettings(context)

/** False when no settings screen answers (some phones remove it). */
@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
@Suppress("SwallowedException")
private fun openCredentialProviderSettings(context: Context): Boolean = try {
    context.startActivity(Intent(Settings.ACTION_CREDENTIAL_PROVIDER, "package:${context.packageName}".toUri()))
    true
} catch (e: ActivityNotFoundException) {
    false
}
