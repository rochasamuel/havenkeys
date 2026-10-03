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
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import androidx.lifecycle.compose.LifecycleResumeEffect
import net.havenkeys.android.R
import net.havenkeys.android.credentials.HavenCredentialService
import net.havenkeys.android.ui.components.ScreenBar
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.theme.HavenSpacing
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

    HavenScaffold(
        modifier = modifier,
        topBar = { ScreenBar(onBack = onBack, online = online, onLock = onLock) },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = HavenSpacing.gutter)
                .padding(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            LargeTitle(stringResource(R.string.autofill_setup_title))
            ServiceState(enabled)
            if (!enabled) {
                HavenButton(
                    stringResource(R.string.autofill_setup_open),
                    onClick = { openFailed = !requestAutofillService(context) },
                    Modifier.fillMaxWidth(),
                )
            }
            if (openFailed) Problem(stringResource(R.string.autofill_setup_unavailable))
            Heading(stringResource(R.string.autofill_setup_chrome_title))
            HavenText(stringResource(R.string.autofill_setup_chrome), color = HavenTheme.colors.muted)
            PasskeysSection(passkeysOn, providerOpenFailed) { providerOpenFailed = !openProviderSettings(context) }
        }
    }
}

@Composable
private fun PasskeysSection(passkeysOn: Boolean, openFailed: Boolean, onOpen: () -> Unit) {
    Heading(stringResource(R.string.autofill_setup_passkeys_title))
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
        HavenText(stringResource(R.string.autofill_setup_passkeys_old), color = HavenTheme.colors.muted)
        return
    }
    HavenText(
        stringResource(if (passkeysOn) R.string.autofill_setup_passkeys_on else R.string.autofill_setup_passkeys_off),
        color = HavenTheme.colors.text,
    )
    if (!passkeysOn) {
        // Secondary: Open settings above may already be the screen's one primary.
        HavenButton(
            stringResource(R.string.autofill_setup_passkeys_open),
            onClick = onOpen,
            Modifier.fillMaxWidth(),
            style = ButtonStyle.Secondary,
        )
    }
    if (openFailed) Problem(stringResource(R.string.autofill_setup_passkeys_unavailable))
}

@Composable
private fun ServiceState(enabled: Boolean) {
    InsetGroup {
        row {
            GroupRow(icon = if (enabled) HavenIcon.Check else HavenIcon.Alert) {
                GroupRowText(stringResource(if (enabled) R.string.autofill_setup_on else R.string.autofill_setup_off))
            }
        }
    }
}

@Composable
private fun Heading(text: String) {
    HavenText(
        text,
        Modifier.padding(top = 12.dp).semantics { heading() },
        style = HavenTheme.type.groupTitle,
        color = HavenTheme.colors.textStrong,
    )
}

@Composable
private fun Problem(text: String) {
    HavenText(text, Modifier.semantics { liveRegion = LiveRegionMode.Polite }, color = HavenTheme.colors.danger)
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
