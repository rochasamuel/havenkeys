package net.havenkeys.android.ui.unlock

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.input.KeyboardActionHandler
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.withResumed
import java.io.IOException
import java.security.GeneralSecurityException
import kotlinx.coroutines.launch
import net.havenkeys.android.AppContainer
import net.havenkeys.android.R
import net.havenkeys.android.security.BootCount
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SecretTextField
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The master password and the Secret Key live only in this composition
 * (`remember`, never `rememberSaveable`); see [UnlockForm] for when each is emptied.
 */
@Composable
fun UnlockScreen(
    viewModel: UnlockViewModel,
    activity: FragmentActivity,
    container: AppContainer,
    onUnlocked: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    var prompting by remember { mutableStateOf(false) }
    var autoOffered by rememberSaveable { mutableStateOf(false) }
    val promptTitle = stringResource(R.string.biometric_prompt_title)
    val promptCancel = stringResource(R.string.biometric_cancel)

    // The scope is the composition's, on the main thread, as BiometricPrompt requires.
    val biometric: () -> Unit = {
        if (!prompting) {
            prompting = true
            scope.launch {
                try {
                    biometricUnlock(activity, container, viewModel, promptTitle, promptCancel)
                } finally {
                    prompting = false
                }
            }
        }
    }

    LaunchedEffect(state.unlocked) { if (state.unlocked) onUnlocked() }
    LaunchedEffect(Unit) {
        if (!autoOffered && viewModel.state.value.offerBiometric) {
            // A prompt started while the activity is stopped is never shown.
            lifecycle.withResumed { }
            autoOffered = true
            biometric()
        }
    }

    UnlockForm(
        state = state,
        onSubmit = viewModel::unlockPassword,
        onBiometric = biometric,
        modifier = modifier,
    )
}

/**
 * The master password and the Secret Key live only in this composition
 * (`remember { TextFieldState() }`, never `rememberTextFieldState`, which is
 * saved with the instance state). The password is emptied the moment it is
 * sent, before Rust answers; the Secret Key stays for another attempt (it is
 * long to retype) and is emptied once the vault opens and when the screen goes.
 */
@Composable
internal fun UnlockForm(
    state: UnlockUiState,
    onSubmit: (password: String, secretKey: String?) -> Unit,
    onBiometric: () -> Unit,
    modifier: Modifier = Modifier,
    // Parameters only so a test can watch them being emptied; never saveable.
    password: TextFieldState = remember { TextFieldState() },
    secretKey: TextFieldState = remember { TextFieldState() },
) {
    var passwordShown by remember { mutableStateOf(false) }
    var keyShown by remember { mutableStateOf(false) }
    val ready = password.text.isNotEmpty() && (!state.needsSecretKey || secretKey.text.isNotBlank()) && !state.busy
    val submit = {
        if (ready) {
            onSubmit(password.text.toString(), if (state.needsSecretKey) secretKey.text.toString() else null)
            password.clearText()
            passwordShown = false
            keyShown = false
        }
    }
    EmptiedOnLeave(state.unlocked, password, secretKey)
    // Only a refused password (or key) is the field's error; biometric, network and other failures are not.
    val fieldError = state.errorCode?.takeIf { it == UNLOCK_FAILED }?.let { stringResource(errorText(it)) }
    val otherError = state.errorCode?.takeIf { it != UNLOCK_FAILED }

    Column(
        modifier
            .fillMaxSize()
            .background(HavenTheme.colors.pane)
            .safeDrawingPadding()
            .imePadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 48.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        UnlockHeader(state.needsSecretKey)
        InsetGroup {
            row {
                SecretTextField(
                    password,
                    stringResource(R.string.unlock_password_hint),
                    revealed = passwordShown,
                    onRevealChange = { passwordShown = it },
                    error = fieldError,
                    enabled = !state.busy,
                    imeAction = if (state.needsSecretKey) ImeAction.Next else ImeAction.Done,
                    onKeyboardAction = if (state.needsSecretKey) null else KeyboardActionHandler { submit() },
                )
            }
            if (state.needsSecretKey) {
                row {
                    SecretTextField(
                        secretKey,
                        stringResource(R.string.unlock_secret_key),
                        revealed = keyShown,
                        onRevealChange = { keyShown = it },
                        enabled = !state.busy,
                        onKeyboardAction = { submit() },
                        placeholder = stringResource(R.string.unlock_secret_key_placeholder),
                    )
                }
            }
        }
        otherError?.let { ErrorLine(it) }
        UnlockButtons(state, ready, submit, onBiometric)
    }
}

/** The Secret Key is emptied once the vault opens; both fields when the screen goes. */
@Composable
private fun EmptiedOnLeave(unlocked: Boolean, password: TextFieldState, secretKey: TextFieldState) {
    LaunchedEffect(unlocked) { if (unlocked) secretKey.clearText() }
    DisposableEffect(password, secretKey) {
        onDispose {
            password.clearText()
            secretKey.clearText()
        }
    }
}

/** The code Rust answers when the master password (or the Secret Key) is wrong. */
private const val UNLOCK_FAILED = "unlock_failed"

@Composable
private fun UnlockButtons(state: UnlockUiState, ready: Boolean, onSubmit: () -> Unit, onBiometric: () -> Unit) {
    HavenButton(
        stringResource(if (state.busy) R.string.unlock_busy else R.string.unlock_button),
        onClick = onSubmit,
        Modifier.fillMaxWidth(),
        enabled = ready,
    )
    if (state.offerBiometric) {
        HavenButton(
            stringResource(R.string.unlock_biometric),
            onClick = onBiometric,
            Modifier.fillMaxWidth(),
            style = ButtonStyle.Secondary,
            enabled = !state.busy,
        )
    }
}

@Composable
private fun UnlockHeader(needsSecretKey: Boolean) {
    val colors = HavenTheme.colors
    IconGlyph(HavenIcon.Lock, contentDescription = null, tint = colors.brass, size = 40.dp)
    HavenText(
        lockedTitle(
            stringResource(R.string.unlock_title),
            stringResource(R.string.unlock_title_locked),
            colors.brassInk,
        ),
        Modifier.semantics { heading() },
        style = HavenTheme.type.display.copy(textAlign = TextAlign.Center),
        color = colors.textStrong,
    )
    if (needsSecretKey) {
        HavenText(
            stringResource(R.string.unlock_enter_password_and_key),
            style = HavenTheme.type.body.copy(textAlign = TextAlign.Center),
            color = colors.muted,
        )
    }
}

/**
 * BiometricPrompt → Keystore opens the bundle → Rust (spec §5.2). A missing
 * bundle, an invalidated key or bytes that do not open end biometric unlock
 * until it is turned on again.
 */
private suspend fun biometricUnlock(
    activity: FragmentActivity,
    container: AppContainer,
    viewModel: UnlockViewModel,
    title: String,
    cancel: String,
) {
    val keys = container.biometricKeys
    val cipher = keystoreOrNull { keys.decryptCipher() }
    if (cipher == null) {
        viewModel.bundleUnusable()
        return
    }
    val unlocked = container.biometricGate.authenticate(activity, title, cancel, cipher) ?: return
    val bytes = keystoreOrNull { keys.open(unlocked) }
    if (bytes == null) viewModel.bundleUnusable() else viewModel.unlockWithBundle(bytes, BootCount.current(activity))
}

@Suppress("SwallowedException")
private inline fun <T> keystoreOrNull(call: () -> T?): T? = try {
    call()
} catch (e: GeneralSecurityException) {
    null
} catch (e: IOException) {
    null
}

/** "HavenKeys is *locked*.": the one word in the serif italic and brass ink, as the desktop sets it. */
private fun lockedTitle(title: String, word: String, ink: Color): AnnotatedString = buildAnnotatedString {
    append(title)
    val at = title.indexOf(word)
    if (word.isNotEmpty() && at >= 0) {
        val italic = SpanStyle(fontStyle = FontStyle.Italic, fontWeight = FontWeight.Normal, color = ink)
        addStyle(italic, at, at + word.length)
    }
}
