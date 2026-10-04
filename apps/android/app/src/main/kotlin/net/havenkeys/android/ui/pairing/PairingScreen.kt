package net.havenkeys.android.ui.pairing

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.Instant
import java.time.format.DateTimeParseException
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.ScreenBar
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenSheet
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.onboarding.KitScanner
import net.havenkeys.android.ui.settings.relativeTime
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.PairingRequestView

/**
 * Scan the new computer's code, show who is asking, and approve behind the
 * biometric check ([verifyUser]); Deny, or closing the sheet, denies.
 */
@Suppress("LongParameterList") // the screen's state and what the app lends it
@Composable
fun PairingScreen(
    viewModel: PairingViewModel,
    online: Boolean,
    canVerify: () -> Boolean,
    verifyUser: suspend (title: String, subtitle: String) -> Boolean,
    onBack: () -> Unit,
    onLock: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val gutter = Modifier.padding(horizontal = HavenSpacing.gutter)
    val title = stringResource(R.string.pairing_title)

    HavenScaffold(
        modifier = modifier,
        topBar = { ScreenBar(onBack = onBack, online = online, onLock = onLock) },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(bottom = padding.calculateBottomPadding())) {
            LargeTitle(title, gutter)
            when (state.stage) {
                PairingUiState.Stage.SCANNING, PairingUiState.Stage.CONFIRM ->
                    Scanning(state, online, viewModel::onFrame, gutter)
                PairingUiState.Stage.DONE -> Finished(R.string.pairing_done, onBack, gutter)
                PairingUiState.Stage.DENIED -> Finished(R.string.pairing_denied, onBack, gutter)
            }
        }
    }

    val request = state.request
    if (state.stage == PairingUiState.Stage.CONFIRM && request != null) {
        ConfirmSheet(
            state,
            request,
            onAllow = { viewModel.allow(canVerify()) { verifyUser(title, request.deviceName) } },
            onDeny = viewModel::deny,
        )
    }
}

@Composable
private fun Scanning(
    state: PairingUiState,
    online: Boolean,
    onFrame: (LumaFrame) -> Unit,
    gutter: Modifier,
) {
    HavenText(stringResource(R.string.pairing_scan_sub), gutter, color = HavenTheme.colors.muted)
    if (!online) {
        ErrorLine("offline", gutter)
        return
    }
    if (state.stage == PairingUiState.Stage.SCANNING) {
        KitScanner(
            onFrame = onFrame,
            modifier = gutter.padding(top = 16.dp).fillMaxWidth().aspectRatio(1f).clip(HavenShape.group),
            cameraNeeded = stringResource(R.string.pairing_camera_needed),
            cameraUnavailable = stringResource(R.string.pairing_camera_unavailable),
        )
    }
    state.errorCode?.let { ErrorLine(it, gutter) }
}

@Composable
private fun ConfirmSheet(state: PairingUiState, request: PairingRequestView, onAllow: () -> Unit, onDeny: () -> Unit) {
    val place = request.location?.let { "$it · ${request.ip}" } ?: request.ip
    val age = ageOf(request.createdAt)
    HavenSheet(
        onDismiss = onDeny,
        title = stringResource(R.string.pairing_confirm_title),
        dismissible = !state.busy,
    ) {
        InsetGroup {
            row {
                GroupRow {
                    GroupRowText(
                        request.deviceName,
                        if (age != null) stringResource(R.string.pairing_confirm_where, place, age) else place,
                    )
                }
            }
        }
        HavenText(
            stringResource(R.string.pairing_confirm_warning),
            Modifier.padding(top = 16.dp),
            color = HavenTheme.colors.muted,
        )
        state.errorCode?.let { ErrorLine(it) }
        Row(
            Modifier.fillMaxWidth().padding(top = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.End),
        ) {
            HavenButton(stringResource(R.string.pairing_deny), onDeny, style = ButtonStyle.Quiet, enabled = !state.busy)
            HavenButton(stringResource(R.string.pairing_allow), onAllow, enabled = !state.busy)
        }
    }
}

@Composable
private fun Finished(message: Int, onBack: () -> Unit, gutter: Modifier) {
    Column(gutter.fillMaxWidth().padding(top = 24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        HavenText(stringResource(message), color = HavenTheme.colors.textStrong)
        HavenButton(stringResource(R.string.pairing_finish), onBack)
    }
}

/** "2 minutes ago", or nothing when the server's time does not parse. */
@Suppress("SwallowedException")
private fun ageOf(rfc3339: String): String? =
    try {
        Instant.parse(rfc3339)
        relativeTime(rfc3339)
    } catch (e: DateTimeParseException) {
        null
    }
