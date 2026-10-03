// One screen with its steps and the small form pieces they share.
@file:Suppress("TooManyFunctions")

package net.havenkeys.android.ui.onboarding

import androidx.activity.compose.BackHandler
import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowField
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SecretTextField
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.KitPreview
import uniffi.havenkeys_mobile.LumaFrame

private const val MIN_PASSWORD_LENGTH = 10

/**
 * First run: scan the Emergency Kit, type it, or activate an invite.
 * Typed secrets (master password, Secret Key, invite) live only in this
 * composition: plain `remember`, never `rememberSaveable`, so they are not
 * written to the saved instance state. A password is emptied when sent; the
 * Secret Key and the invite stay for another attempt; all are emptied when
 * their step is left. Only the server and email (not secrets) are saved.
 */
@Composable
fun OnboardingScreen(viewModel: OnboardingViewModel, onDone: () -> Unit, modifier: Modifier = Modifier) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val mode = state.mode
    LaunchedEffect(state.done) { if (state.done) onDone() }
    BackHandler(enabled = mode != OnboardingUiState.Mode.CHOOSE) { viewModel.back() }
    HavenScaffold(
        modifier = modifier,
        topBar = { OnboardingBar(showBack = mode != OnboardingUiState.Mode.CHOOSE, onBack = viewModel::back) },
    ) { _ ->
        val content = Modifier.fillMaxSize()
        when (mode) {
            OnboardingUiState.Mode.CHOOSE -> ChooseStep(viewModel::choose, content)
            OnboardingUiState.Mode.SCAN -> ScanStep(state.errorCode, viewModel::onFrame, viewModel::back, content)
            OnboardingUiState.Mode.TYPE -> TypeStep(state, viewModel::signIn, content)
            OnboardingUiState.Mode.INVITE -> InviteStep(state, viewModel::activate, content)
            OnboardingUiState.Mode.PASSWORD -> KitPasswordStep(state, viewModel::signInWithKit, content)
        }
    }
}

/**
 * Back (from a step) above the large title; the slot keeps its height so the title does not jump.
 * It is as tall as ScreenBar (4dp above and below the button), so the title sits where it does there.
 */
@Composable
private fun OnboardingBar(showBack: Boolean, onBack: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = HavenSpacing.gutter)) {
        Box(Modifier.padding(vertical = 4.dp).heightIn(min = HavenSpacing.touch)) {
            if (showBack) {
                HavenIconButton(
                    HavenIcon.ChevronLeft,
                    stringResource(R.string.onboarding_back),
                    onClick = onBack,
                    modifier = Modifier.offset(x = (-12).dp),
                )
            }
        }
        LargeTitle(stringResource(R.string.onboarding_title))
    }
}

@Composable
private fun ChooseStep(onChoose: (OnboardingUiState.Mode) -> Unit, modifier: Modifier) {
    FormColumn(modifier) {
        HavenText(
            stringResource(R.string.onboarding_how),
            style = HavenTheme.type.titleSmall,
            color = HavenTheme.colors.textStrong,
        )
        InsetGroup {
            row {
                Choice(HavenIcon.Qr, R.string.onboarding_scan_kit, R.string.onboarding_scan_kit_sub) {
                    onChoose(OnboardingUiState.Mode.SCAN)
                }
            }
            row {
                Choice(HavenIcon.Edit, R.string.onboarding_type_kit, R.string.onboarding_type_kit_sub) {
                    onChoose(OnboardingUiState.Mode.TYPE)
                }
            }
            row {
                Choice(HavenIcon.Plus, R.string.onboarding_invite_choice, R.string.onboarding_invite_sub) {
                    onChoose(OnboardingUiState.Mode.INVITE)
                }
            }
        }
    }
}

@Composable
private fun Choice(icon: HavenIcon, @StringRes title: Int, @StringRes subtitle: Int, onClick: () -> Unit) {
    GroupRow(onClick = onClick, icon = icon) { GroupRowText(stringResource(title), stringResource(subtitle)) }
}

@Composable
private fun ScanStep(errorCode: String?, onFrame: (LumaFrame) -> Unit, onCancel: () -> Unit, modifier: Modifier) {
    Box(modifier) {
        KitScanner(onFrame = onFrame, modifier = Modifier.fillMaxSize())
        Column(
            Modifier
                .align(Alignment.BottomCenter)
                .padding(HavenSpacing.gutter)
                .fillMaxWidth()
                .clip(HavenShape.group)
                .background(HavenTheme.colors.raised)
                .padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            HavenText(
                stringResource(R.string.onboarding_scanning),
                style = HavenTheme.type.value,
                color = HavenTheme.colors.textStrong,
            )
            MaybeError(errorCode)
            HavenButton(stringResource(R.string.onboarding_cancel), onClick = onCancel, style = ButtonStyle.Secondary)
        }
    }
}

/**
 * A typed secret and whether it is shown. Plain state, never saved with the instance; it is
 * emptied when sent and when its step leaves the composition.
 */
internal class Secret {
    val text = TextFieldState()
    var shown by mutableStateOf(false)

    fun clear() {
        text.clearText()
        shown = false
    }
}

@Composable
internal fun rememberSecret(): Secret {
    val secret = remember { Secret() }
    DisposableEffect(secret) { onDispose { secret.clear() } }
    return secret
}

@Composable
private fun SecretRow(
    secret: Secret,
    @StringRes label: Int,
    enabled: Boolean,
    error: String? = null,
    hint: String? = null,
    imeAction: ImeAction = ImeAction.Next,
) {
    SecretTextField(
        secret.text,
        stringResource(label),
        revealed = secret.shown,
        onRevealChange = { secret.shown = it },
        error = error,
        enabled = enabled,
        imeAction = imeAction,
        hint = hint,
    )
}

@Composable
private fun TypeStep(
    state: OnboardingUiState,
    onSignIn: (server: String, email: String, password: String, secretKey: String) -> Unit,
    modifier: Modifier,
) {
    // Not secrets: kept across a restore, as before. The secrets are `remember`ed only.
    val server = rememberSaveable(saver = TextFieldState.Saver) { TextFieldState() }
    val email = rememberSaveable(saver = TextFieldState.Saver) { TextFieldState() }
    val secretKey = rememberSecret()
    val password = rememberSecret()
    val ready = server.text.isNotBlank() && email.text.isNotBlank() && secretKey.text.text.isNotBlank() &&
        password.text.text.isNotEmpty()
    FormColumn(modifier) {
        InsetGroup {
            row {
                HavenTextField(
                    server,
                    stringResource(R.string.onboarding_server),
                    enabled = !state.busy,
                    keyboardOptions = plainKeyboard(KeyboardType.Uri),
                    hint = stringResource(R.string.onboarding_server_hint),
                )
            }
            row {
                HavenTextField(
                    email,
                    stringResource(R.string.onboarding_email),
                    placeholder = stringResource(R.string.onboarding_email_placeholder),
                    enabled = !state.busy,
                    keyboardOptions = plainKeyboard(KeyboardType.Email),
                )
            }
            row {
                SecretRow(
                    secretKey,
                    R.string.onboarding_secret_key,
                    !state.busy,
                    hint = stringResource(R.string.onboarding_secret_key_hint),
                )
            }
            row { SecretRow(password, R.string.onboarding_master_password, !state.busy) }
        }
        MaybeError(state.errorCode)
        SubmitButton(
            text = stringResource(if (state.busy) R.string.onboarding_signing_in else R.string.onboarding_sign_in),
            enabled = ready && !state.busy,
            onClick = {
                val key = secretKey.text.text.toString()
                onSignIn(server.text.toString(), email.text.toString(), password.text.text.toString(), key)
                // The password is emptied as it is sent; the Secret Key stays for another attempt
                // (long to retype) and is emptied when the step is left, which success does too.
                password.clear()
            },
        )
        Note(stringResource(R.string.onboarding_sign_in_note))
    }
}

@Composable
private fun InviteStep(
    state: OnboardingUiState,
    onActivate: (invite: String, password: String) -> Unit,
    modifier: Modifier,
) {
    val invite = rememberSecret()
    val password = rememberSecret()
    val repeat = rememberSecret()
    val tooShort = password.text.text.isNotEmpty() && password.text.text.length < MIN_PASSWORD_LENGTH
    val mismatch = repeat.text.text.isNotEmpty() && repeat.text.text.toString() != password.text.text.toString()
    val ready = invite.text.text.isNotBlank() && password.text.text.length >= MIN_PASSWORD_LENGTH &&
        repeat.text.text.toString() == password.text.text.toString()
    FormColumn(modifier) {
        InsetGroup {
            row {
                SecretRow(
                    invite,
                    R.string.onboarding_invite,
                    !state.busy,
                    hint = stringResource(R.string.onboarding_invite_hint),
                )
            }
            row {
                SecretRow(
                    password,
                    R.string.onboarding_master_password,
                    !state.busy,
                    error = if (tooShort) stringResource(R.string.onboarding_too_short) else null,
                    hint = stringResource(R.string.onboarding_password_hint),
                )
            }
            row {
                SecretRow(
                    repeat,
                    R.string.onboarding_repeat_password,
                    !state.busy,
                    error = if (mismatch) stringResource(R.string.onboarding_mismatch) else null,
                    imeAction = ImeAction.Done,
                )
            }
        }
        MaybeError(state.errorCode)
        SubmitButton(
            text = stringResource(if (state.busy) R.string.onboarding_creating else R.string.onboarding_create),
            enabled = ready && !state.busy,
            onClick = {
                onActivate(invite.text.text.toString(), password.text.text.toString())
                // The passwords are emptied as they are sent; the invite stays for another attempt
                // and is emptied when the step is left, which success does too.
                password.clear()
                repeat.clear()
            },
        )
        Note(stringResource(R.string.onboarding_create_note))
    }
}

@Composable
private fun KitPasswordStep(state: OnboardingUiState, onSignIn: (password: String) -> Unit, modifier: Modifier) {
    val password = rememberSecret()
    FormColumn(modifier) {
        state.preview?.let { KitSummary(it) }
        // A lede in muted body, as unlock's and the generator's are.
        HavenText(
            stringResource(R.string.onboarding_kit_password),
            style = HavenTheme.type.body,
            color = HavenTheme.colors.muted,
        )
        InsetGroup {
            row { SecretRow(password, R.string.onboarding_master_password, !state.busy, imeAction = ImeAction.Done) }
        }
        MaybeError(state.errorCode)
        SubmitButton(
            text = stringResource(if (state.busy) R.string.onboarding_signing_in else R.string.onboarding_sign_in),
            enabled = password.text.text.isNotEmpty() && !state.busy,
            onClick = {
                onSignIn(password.text.text.toString())
                password.clear()
            },
        )
        Note(stringResource(R.string.onboarding_sign_in_note))
    }
}

@Composable
private fun KitSummary(preview: KitPreview) {
    Column {
        SectionHeader(stringResource(R.string.onboarding_kit_found))
        InsetGroup {
            row { GroupRow { GroupRowField(stringResource(R.string.onboarding_email), preview.email) } }
            row { GroupRow { GroupRowField(stringResource(R.string.onboarding_server), preview.serverUrl) } }
        }
    }
}

private fun plainKeyboard(type: KeyboardType) =
    KeyboardOptions(keyboardType = type, autoCorrectEnabled = false, imeAction = ImeAction.Next)

@Composable
private fun FormColumn(modifier: Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier
            .verticalScroll(rememberScrollState())
            .padding(horizontal = HavenSpacing.gutter, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        content = content,
    )
}

@Composable
private fun SubmitButton(text: String, enabled: Boolean, onClick: () -> Unit) {
    HavenButton(text, onClick = onClick, Modifier.fillMaxWidth(), enabled = enabled)
}

@Composable
private fun MaybeError(code: String?) {
    if (code != null) ErrorLine(code)
}

@Composable
private fun Note(text: String) {
    HavenText(text, style = HavenTheme.type.rowSubtitle, color = HavenTheme.colors.muted)
}
