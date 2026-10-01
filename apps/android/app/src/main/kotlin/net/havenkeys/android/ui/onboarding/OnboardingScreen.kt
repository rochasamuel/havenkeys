// One screen with its steps and the small form pieces they share.
@file:Suppress("TooManyFunctions")

package net.havenkeys.android.ui.onboarding

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.Keyboard
import androidx.compose.material.icons.outlined.Mail
import androidx.compose.material.icons.outlined.QrCodeScanner
import androidx.compose.material.icons.outlined.Visibility
import androidx.compose.material.icons.outlined.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.autofill.ContentType
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentType
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.KitPreview
import uniffi.havenkeys_mobile.LumaFrame

private const val MIN_PASSWORD_LENGTH = 10

/**
 * First run: scan the Emergency Kit, type it, or activate an invite.
 * Typed secrets live only in this composition (`remember`, never
 * `rememberSaveable`), so they are not written to the saved instance state.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OnboardingScreen(viewModel: OnboardingViewModel, onDone: () -> Unit, modifier: Modifier = Modifier) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val mode = state.mode
    LaunchedEffect(state.done) { if (state.done) onDone() }
    BackHandler(enabled = mode != OnboardingUiState.Mode.CHOOSE) { viewModel.back() }

    Scaffold(
        modifier = modifier,
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.onboarding_title)) },
                navigationIcon = {
                    if (mode != OnboardingUiState.Mode.CHOOSE) {
                        IconButton(onClick = viewModel::back) {
                            Icon(
                                Icons.AutoMirrored.Outlined.ArrowBack,
                                contentDescription = stringResource(R.string.onboarding_back),
                            )
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                    titleContentColor = HavenTheme.colors.textStrong,
                ),
            )
        },
    ) { padding ->
        val content = Modifier
            .padding(padding)
            .fillMaxSize()
        when (mode) {
            OnboardingUiState.Mode.CHOOSE -> ChooseStep(viewModel::choose, content)
            OnboardingUiState.Mode.SCAN -> ScanStep(state.errorCode, viewModel::onFrame, viewModel::back, content)
            OnboardingUiState.Mode.TYPE -> TypeStep(state, viewModel::signIn, content)
            OnboardingUiState.Mode.INVITE -> InviteStep(state, viewModel::activate, content)
            OnboardingUiState.Mode.PASSWORD -> KitPasswordStep(state, viewModel::signInWithKit, content)
        }
    }
}

@Composable
private fun ChooseStep(onChoose: (OnboardingUiState.Mode) -> Unit, modifier: Modifier) {
    FormColumn(modifier) {
        Text(
            stringResource(R.string.onboarding_how),
            style = MaterialTheme.typography.titleMedium,
            color = HavenTheme.colors.textStrong,
        )
        Choice(
            icon = Icons.Outlined.QrCodeScanner,
            title = stringResource(R.string.onboarding_scan_kit),
            subtitle = stringResource(R.string.onboarding_scan_kit_sub),
            onClick = { onChoose(OnboardingUiState.Mode.SCAN) },
        )
        Choice(
            icon = Icons.Outlined.Keyboard,
            title = stringResource(R.string.onboarding_type_kit),
            subtitle = stringResource(R.string.onboarding_type_kit_sub),
            onClick = { onChoose(OnboardingUiState.Mode.TYPE) },
        )
        Choice(
            icon = Icons.Outlined.Mail,
            title = stringResource(R.string.onboarding_invite_choice),
            subtitle = stringResource(R.string.onboarding_invite_sub),
            onClick = { onChoose(OnboardingUiState.Mode.INVITE) },
        )
    }
}

@Composable
private fun Choice(icon: ImageVector, title: String, subtitle: String, onClick: () -> Unit) {
    Surface(
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
    ) {
        ListItem(
            headlineContent = { Text(title) },
            supportingContent = { Text(subtitle) },
            leadingContent = { Icon(icon, contentDescription = null, tint = HavenTheme.colors.brass) },
            colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
        )
    }
}

@Composable
private fun ScanStep(
    errorCode: String?,
    onFrame: (LumaFrame) -> Unit,
    onCancel: () -> Unit,
    modifier: Modifier,
) {
    Box(modifier) {
        KitScanner(onFrame = onFrame, modifier = Modifier.fillMaxSize())
        Surface(
            color = MaterialTheme.colorScheme.surface.copy(alpha = 0.92f),
            shape = MaterialTheme.shapes.large,
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(16.dp)
                .fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(stringResource(R.string.onboarding_scanning), style = MaterialTheme.typography.bodyLarge)
                ErrorLine(errorCode)
                OutlinedButton(onClick = onCancel) { Text(stringResource(R.string.onboarding_cancel)) }
            }
        }
    }
}

@Composable
private fun TypeStep(
    state: OnboardingUiState,
    onSignIn: (server: String, email: String, password: String, secretKey: String) -> Unit,
    modifier: Modifier,
) {
    var server by rememberSaveable { mutableStateOf("") }
    var email by rememberSaveable { mutableStateOf("") }
    var secretKey by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    val ready = server.isNotBlank() && email.isNotBlank() && secretKey.isNotBlank() && password.isNotEmpty()

    FormColumn(modifier) {
        OutlinedTextField(
            value = server,
            onValueChange = { server = it },
            label = { Text(stringResource(R.string.onboarding_server)) },
            supportingText = { Text(stringResource(R.string.onboarding_server_hint)) },
            singleLine = true,
            enabled = !state.busy,
            keyboardOptions = plainKeyboard(KeyboardType.Uri),
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = email,
            onValueChange = { email = it },
            label = { Text(stringResource(R.string.onboarding_email)) },
            placeholder = { Text(stringResource(R.string.onboarding_email_placeholder)) },
            singleLine = true,
            enabled = !state.busy,
            keyboardOptions = plainKeyboard(KeyboardType.Email),
            modifier = Modifier.fillMaxWidth(),
        )
        SecretInput(
            value = secretKey,
            onValueChange = { secretKey = it },
            label = stringResource(R.string.onboarding_secret_key),
            hint = stringResource(R.string.onboarding_secret_key_hint),
            enabled = !state.busy,
        )
        SecretInput(
            value = password,
            onValueChange = { password = it },
            label = stringResource(R.string.onboarding_master_password),
            enabled = !state.busy,
            contentType = ContentType.Password,
        )
        ErrorLine(state.errorCode)
        SubmitButton(
            text = stringResource(if (state.busy) R.string.onboarding_signing_in else R.string.onboarding_sign_in),
            enabled = ready && !state.busy,
            onClick = { onSignIn(server, email, password, secretKey) },
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
    var invite by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var repeat by remember { mutableStateOf("") }
    val tooShort = password.isNotEmpty() && password.length < MIN_PASSWORD_LENGTH
    val mismatch = repeat.isNotEmpty() && repeat != password
    val ready = invite.isNotBlank() && password.length >= MIN_PASSWORD_LENGTH && repeat == password

    FormColumn(modifier) {
        SecretInput(
            value = invite,
            onValueChange = { invite = it },
            label = stringResource(R.string.onboarding_invite),
            hint = stringResource(R.string.onboarding_invite_hint),
            enabled = !state.busy,
        )
        SecretInput(
            value = password,
            onValueChange = { password = it },
            label = stringResource(R.string.onboarding_master_password),
            hint = stringResource(if (tooShort) R.string.onboarding_too_short else R.string.onboarding_password_hint),
            isError = tooShort,
            enabled = !state.busy,
            contentType = ContentType.NewPassword,
        )
        SecretInput(
            value = repeat,
            onValueChange = { repeat = it },
            label = stringResource(R.string.onboarding_repeat_password),
            hint = if (mismatch) stringResource(R.string.onboarding_mismatch) else null,
            isError = mismatch,
            enabled = !state.busy,
            contentType = ContentType.NewPassword,
        )
        ErrorLine(state.errorCode)
        SubmitButton(
            text = stringResource(if (state.busy) R.string.onboarding_creating else R.string.onboarding_create),
            enabled = ready && !state.busy,
            onClick = { onActivate(invite, password) },
        )
        Note(stringResource(R.string.onboarding_create_note))
    }
}

@Composable
private fun KitPasswordStep(state: OnboardingUiState, onSignIn: (password: String) -> Unit, modifier: Modifier) {
    var password by remember { mutableStateOf("") }

    FormColumn(modifier) {
        state.preview?.let { KitSummary(it) }
        Text(stringResource(R.string.onboarding_kit_password), style = MaterialTheme.typography.bodyLarge)
        SecretInput(
            value = password,
            onValueChange = { password = it },
            label = stringResource(R.string.onboarding_master_password),
            enabled = !state.busy,
            contentType = ContentType.Password,
        )
        ErrorLine(state.errorCode)
        SubmitButton(
            text = stringResource(if (state.busy) R.string.onboarding_signing_in else R.string.onboarding_sign_in),
            enabled = password.isNotEmpty() && !state.busy,
            onClick = { onSignIn(password) },
        )
        Note(stringResource(R.string.onboarding_sign_in_note))
    }
}

@Composable
private fun KitSummary(preview: KitPreview) {
    Surface(
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                stringResource(R.string.onboarding_kit_found),
                style = MaterialTheme.typography.titleMedium,
                color = HavenTheme.colors.textStrong,
            )
            LabelledValue(stringResource(R.string.onboarding_email), preview.email)
            LabelledValue(stringResource(R.string.onboarding_server), preview.serverUrl)
        }
    }
}

@Composable
private fun LabelledValue(label: String, value: String) {
    Column {
        Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(value, style = MaterialTheme.typography.bodyLarge)
    }
}

/**
 * A field for a secret: masked until the user shows it, a keyboard that
 * neither suggests nor learns, and no autofill hint unless [contentType]
 * names one.
 */
@Composable
private fun SecretInput(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    enabled: Boolean,
    hint: String? = null,
    isError: Boolean = false,
    contentType: ContentType? = null,
) {
    var shown by remember { mutableStateOf(false) }
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        supportingText = hint?.let { { Text(it) } },
        isError = isError,
        singleLine = true,
        enabled = enabled,
        visualTransformation = if (shown) VisualTransformation.None else PasswordVisualTransformation(),
        keyboardOptions = KeyboardOptions(
            keyboardType = KeyboardType.Password,
            autoCorrectEnabled = false,
            imeAction = ImeAction.Next,
        ),
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

private fun plainKeyboard(type: KeyboardType) =
    KeyboardOptions(keyboardType = type, autoCorrectEnabled = false, imeAction = ImeAction.Next)

@Composable
private fun FormColumn(modifier: Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier = modifier
            .imePadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        content = content,
    )
}

@Composable
private fun SubmitButton(text: String, enabled: Boolean, onClick: () -> Unit) {
    Button(onClick = onClick, enabled = enabled, modifier = Modifier.fillMaxWidth()) { Text(text) }
}

@Composable
private fun ErrorLine(code: String?) {
    if (code != null) {
        Text(
            stringResource(errorText(code)),
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

@Composable
private fun Note(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}
