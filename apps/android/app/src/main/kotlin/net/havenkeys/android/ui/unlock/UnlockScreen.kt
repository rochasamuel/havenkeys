package net.havenkeys.android.ui.unlock

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Fingerprint
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.Visibility
import androidx.compose.material.icons.outlined.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.autofill.ContentType
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentType
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
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
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The master password and the Secret Key live only in this composition
 * (`remember`, never `rememberSaveable`) and are cleared as soon as they are
 * submitted.
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

@Composable
private fun UnlockForm(
    state: UnlockUiState,
    onSubmit: (password: String, secretKey: String?) -> Unit,
    onBiometric: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var password by remember { mutableStateOf("") }
    var secretKey by remember { mutableStateOf("") }
    val ready = password.isNotEmpty() && (!state.needsSecretKey || secretKey.isNotBlank()) && !state.busy
    val submit = {
        if (ready) {
            onSubmit(password, if (state.needsSecretKey) secretKey else null)
            password = ""
            secretKey = ""
        }
    }

    Surface(modifier = modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
        Column(
            modifier = Modifier
                .safeDrawingPadding()
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 48.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            UnlockHeader(state.needsSecretKey)
            MaskedField(
                value = password,
                onValueChange = { password = it },
                label = stringResource(R.string.unlock_password_hint),
                enabled = !state.busy,
                isError = state.errorCode != null,
                contentType = ContentType.Password,
                imeAction = if (state.needsSecretKey) ImeAction.Next else ImeAction.Done,
                onDone = submit,
            )
            if (state.needsSecretKey) {
                MaskedField(
                    value = secretKey,
                    onValueChange = { secretKey = it },
                    label = stringResource(R.string.unlock_secret_key),
                    enabled = !state.busy,
                    placeholder = stringResource(R.string.unlock_secret_key_placeholder),
                    onDone = submit,
                )
            }
            UnlockActions(state, ready, submit, onBiometric)
        }
    }
}

@Composable
private fun UnlockActions(state: UnlockUiState, ready: Boolean, onSubmit: () -> Unit, onBiometric: () -> Unit) {
    state.errorCode?.let { code ->
        Text(
            stringResource(errorText(code)),
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.fillMaxWidth(),
        )
    }
    Button(onClick = onSubmit, enabled = ready, modifier = Modifier.fillMaxWidth()) {
        Text(stringResource(if (state.busy) R.string.unlock_busy else R.string.unlock_button))
    }
    if (state.offerBiometric) {
        OutlinedButton(onClick = onBiometric, enabled = !state.busy, modifier = Modifier.fillMaxWidth()) {
            Icon(Icons.Outlined.Fingerprint, contentDescription = null, modifier = Modifier.padding(end = 8.dp))
            Text(stringResource(R.string.unlock_biometric))
        }
    }
}

@Composable
private fun UnlockHeader(needsSecretKey: Boolean) {
    Icon(
        Icons.Outlined.Lock,
        contentDescription = null,
        tint = HavenTheme.colors.brass,
        modifier = Modifier.size(40.dp),
    )
    Text(
        stringResource(R.string.unlock_title),
        style = MaterialTheme.typography.titleLarge,
        color = HavenTheme.colors.textStrong,
    )
    if (needsSecretKey) {
        Text(
            stringResource(R.string.unlock_enter_password_and_key),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** Masked until shown; a keyboard that neither suggests nor learns. */
@Composable
private fun MaskedField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    enabled: Boolean,
    isError: Boolean = false,
    placeholder: String? = null,
    contentType: ContentType? = null,
    imeAction: ImeAction = ImeAction.Done,
    onDone: () -> Unit = {},
) {
    var shown by remember { mutableStateOf(false) }
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        placeholder = placeholder?.let { { Text(it) } },
        singleLine = true,
        enabled = enabled,
        isError = isError,
        visualTransformation = if (shown) VisualTransformation.None else PasswordVisualTransformation(),
        keyboardOptions = KeyboardOptions(
            keyboardType = KeyboardType.Password,
            autoCorrectEnabled = false,
            imeAction = imeAction,
        ),
        keyboardActions = KeyboardActions(onDone = { onDone() }),
        trailingIcon = {
            IconButton(onClick = { shown = !shown }) {
                if (shown) {
                    Icon(Icons.Outlined.VisibilityOff, contentDescription = stringResource(R.string.hide, label))
                } else {
                    Icon(Icons.Outlined.Visibility, contentDescription = stringResource(R.string.reveal, label))
                }
            }
        },
        modifier = Modifier
            .fillMaxWidth()
            .then(if (contentType != null) Modifier.semantics { this.contentType = contentType } else Modifier),
    )
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
