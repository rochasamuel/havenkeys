package net.havenkeys.android.ui.generator

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.HavenTopBar
import net.havenkeys.android.ui.components.colourised
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.theme.HavenType
import uniffi.havenkeys_mobile.GeneratorOptions

/**
 * A new password for each change of the options. The password lives only in
 * this composition (`remember`, not saveable), so leaving the screen or the
 * lock wipe drops it.
 */
@Composable
fun GeneratorScreen(
    viewModel: GeneratorViewModel,
    clipboard: SensitiveClipboard,
    online: Boolean,
    onBack: () -> Unit,
    onLock: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var password by remember { mutableStateOf<String?>(null) }
    var failure by remember { mutableStateOf<String?>(null) }
    // Bumped by Regenerate, so the same options give a new password.
    var round by remember { mutableIntStateOf(0) }
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val resources = LocalResources.current
    val label = stringResource(R.string.field_password)

    LaunchedEffect(state.options, round) {
        when (val r = viewModel.generate()) {
            is Outcome.Ok -> {
                password = r.value
                failure = null
            }
            is Outcome.Failed -> {
                password = null
                failure = r.code
            }
        }
    }

    Scaffold(
        topBar = {
            HavenTopBar(stringResource(R.string.generator_title), online = online, onLock = onLock, onBack = onBack)
        },
        snackbarHost = { SnackbarHost(snackbar) },
        containerColor = MaterialTheme.colorScheme.background,
        modifier = modifier,
    ) { padding ->
        Column(
            Modifier
                .padding(padding)
                .fillMaxSize()
                .verticalScroll(rememberScrollState()),
        ) {
            Output(password, failure, state.entropyBits)
            Actions(
                canCopy = password != null,
                onRegenerate = { round++ },
                onCopy = {
                    val value = password
                    if (value != null) {
                        scope.launch {
                            val seconds = viewModel.clipboardClearSeconds()
                            clipboard.copy(label, value, seconds)
                            snackbar.showSnackbar(resources.getString(R.string.generator_copied, seconds))
                        }
                    }
                },
            )
            Options(state.options, viewModel::setOptions)
        }
    }
}

@Composable
private fun Actions(canCopy: Boolean, onRegenerate: () -> Unit, onCopy: () -> Unit) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        modifier = Modifier.padding(horizontal = 16.dp),
    ) {
        OutlinedButton(onClick = onRegenerate) {
            Icon(Icons.Outlined.Refresh, contentDescription = null, modifier = Modifier.padding(end = 8.dp))
            Text(stringResource(R.string.generator_regenerate))
        }
        Button(enabled = canCopy, onClick = onCopy) {
            Icon(Icons.Outlined.ContentCopy, contentDescription = null, modifier = Modifier.padding(end = 8.dp))
            Text(stringResource(R.string.generator_copy))
        }
    }
}

@Composable
private fun Output(password: String?, failure: String?, entropyBits: Double?) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        shape = MaterialTheme.shapes.medium,
        modifier = Modifier.fillMaxWidth().padding(16.dp),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                stringResource(R.string.generator_lede),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            when {
                password != null -> Text(
                    colourised(password, HavenTheme.colors),
                    style = HavenType.secret,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                failure != null -> Text(
                    stringResource(if (failure == "invalid_input") R.string.generator_failed else errorText(failure)),
                    color = MaterialTheme.colorScheme.error,
                )
            }
            if (password != null && entropyBits != null) {
                Text(
                    stringResource(
                        R.string.generator_strength,
                        stringResource(strengthLabel(strengthOf(entropyBits))),
                        entropyBits.roundToInt(),
                    ),
                    style = MaterialTheme.typography.labelMedium,
                    color = HavenTheme.colors.brassInk,
                )
            }
        }
    }
}

@Composable
private fun Options(options: GeneratorOptions, onChange: (GeneratorOptions) -> Unit) {
    SectionTitle(R.string.generator_length)
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.padding(horizontal = 16.dp),
    ) {
        Slider(
            value = options.length.toFloat(),
            onValueChange = { onChange(options.copy(length = it.roundToInt().toUInt())) },
            valueRange = GeneratorViewModel.MIN_LENGTH.toFloat()..GeneratorViewModel.MAX_LENGTH.toFloat(),
            modifier = Modifier.weight(1f),
        )
        Text(
            options.length.toString(),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(start = 16.dp),
        )
    }
    SectionTitle(R.string.generator_characters)
    Toggle(R.string.generator_uppercase, "A–Z", options.uppercase) { onChange(options.copy(uppercase = it)) }
    Toggle(R.string.generator_lowercase, "a–z", options.lowercase) { onChange(options.copy(lowercase = it)) }
    Toggle(R.string.generator_digits, "0–9", options.digits) { onChange(options.copy(digits = it)) }
    Toggle(R.string.generator_symbols, "!@#…", options.symbols) { onChange(options.copy(symbols = it)) }
    Toggle(R.string.generator_avoid_ambiguous, "l, 1, O, 0", options.avoidAmbiguous) {
        onChange(options.copy(avoidAmbiguous = it))
    }
}

@Composable
private fun SectionTitle(@StringRes text: Int) {
    Text(
        stringResource(text),
        style = MaterialTheme.typography.titleSmall,
        color = HavenTheme.colors.textStrong,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp),
    )
}

@Composable
private fun Toggle(@StringRes label: Int, hint: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    ListItem(
        headlineContent = { Text(stringResource(label)) },
        supportingContent = { Text(hint) },
        trailingContent = { Switch(checked = checked, onCheckedChange = onChange) },
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.background),
    )
}

@StringRes
private fun strengthLabel(strength: Strength): Int = when (strength) {
    Strength.WEAK -> R.string.generator_weak
    Strength.FAIR -> R.string.generator_fair
    Strength.STRONG -> R.string.generator_strong
    Strength.EXCELLENT -> R.string.generator_excellent
}
