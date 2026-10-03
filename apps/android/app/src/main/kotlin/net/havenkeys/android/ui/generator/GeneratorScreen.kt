package net.havenkeys.android.ui.generator

import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.ScreenBar
import net.havenkeys.android.ui.components.RevealedValue
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenSlider
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.kit.ToggleRow
import net.havenkeys.android.ui.kit.rememberToastState
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.GeneratorOptions

/** Every whole length from the core's minimum to its maximum is a stop. */
private val LengthSteps = (GeneratorViewModel.MAX_LENGTH - GeneratorViewModel.MIN_LENGTH).toInt() - 1

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
    val toasts = rememberToastState()
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

    HavenScaffold(
        modifier = modifier,
        topBar = { ScreenBar(onBack = onBack, online = online, onLock = onLock) },
        toastState = toasts,
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = HavenSpacing.gutter)
                .padding(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            LargeTitle(stringResource(R.string.generator_title))
            // Under the title, as on the desktop: the plate below holds only the password and its strength.
            HavenText(
                stringResource(R.string.generator_lede),
                Modifier.padding(bottom = 16.dp),
                color = HavenTheme.colors.muted,
            )
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
                            toasts.show(resources.getString(R.string.generator_copied, seconds))
                        }
                    }
                },
            )
            Options(state.options, viewModel::setOptions)
        }
    }
}

/** The desktop's generated password: mono at 20 on 30, a size up from a revealed field. */
@Composable
private fun generatedStyle() = HavenTheme.type.secret.copy(fontSize = 20.sp, lineHeight = 30.sp)

/** The password on the output plate (the desktop's 14dp radius), digits and symbols coloured. */
@Composable
private fun Output(password: String?, failure: String?, entropyBits: Double?) {
    val colors = HavenTheme.colors
    Column(
        Modifier
            .fillMaxWidth()
            .clip(HavenShape.output)
            .background(colors.group)
            .border(1.dp, colors.groupLine, HavenShape.output)
            .padding(start = 16.dp, end = 16.dp, top = 20.dp, bottom = 16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        when {
            password != null -> RevealedValue(password, style = generatedStyle())
            failure != null -> HavenText(
                stringResource(if (failure == "invalid_input") R.string.generator_failed else errorText(failure)),
                color = colors.danger,
            )
        }
        if (password != null && entropyBits != null) {
            HavenText(
                stringResource(
                    R.string.generator_strength,
                    stringResource(strengthLabel(strengthOf(entropyBits))),
                    entropyBits.roundToInt(),
                ),
                style = HavenTheme.type.label,
                color = colors.brassInk,
            )
        }
    }
}

@Composable
private fun Actions(canCopy: Boolean, onRegenerate: () -> Unit, onCopy: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(vertical = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        HavenButton(
            stringResource(R.string.generator_regenerate),
            onClick = onRegenerate,
            Modifier.weight(1f),
            style = ButtonStyle.Secondary,
            icon = HavenIcon.Refresh,
        )
        HavenButton(
            stringResource(R.string.generator_copy),
            onClick = onCopy,
            Modifier.weight(1f),
            enabled = canCopy,
            icon = HavenIcon.Copy,
        )
    }
}

@Composable
private fun Options(options: GeneratorOptions, onChange: (GeneratorOptions) -> Unit) {
    val length = stringResource(R.string.generator_length)
    InsetGroup {
        row {
            Column(Modifier.padding(horizontal = HavenSpacing.rowX, vertical = 8.dp)) {
                // The slider shows no number of its own (DESIGN.md): the row shows it. TalkBack hears
                // label and value from the slider, so the row is hidden from it rather than read twice.
                Row(Modifier.clearAndSetSemantics {}) {
                    val colors = HavenTheme.colors
                    HavenText(length, Modifier.weight(1f), style = HavenTheme.type.value, color = colors.text)
                    HavenText(
                        options.length.toString(),
                        style = HavenTheme.type.value,
                        color = colors.textStrong,
                    )
                }
                HavenSlider(
                    value = options.length.toFloat(),
                    onValueChange = { onChange(options.copy(length = it.roundToInt().toUInt())) },
                    valueRange = GeneratorViewModel.MIN_LENGTH.toFloat()..GeneratorViewModel.MAX_LENGTH.toFloat(),
                    label = length,
                    steps = LengthSteps,
                    valueText = options.length.toString(),
                )
            }
        }
    }
    Spacer(Modifier.height(HavenSpacing.groupGap))
    SectionHeader(stringResource(R.string.generator_characters))
    InsetGroup {
        row {
            Toggle(R.string.generator_uppercase, "A–Z", options.uppercase) { onChange(options.copy(uppercase = it)) }
        }
        row {
            Toggle(R.string.generator_lowercase, "a–z", options.lowercase) { onChange(options.copy(lowercase = it)) }
        }
        row { Toggle(R.string.generator_digits, "0–9", options.digits) { onChange(options.copy(digits = it)) } }
        row {
            Toggle(R.string.generator_symbols, "!@#…", options.symbols) { onChange(options.copy(symbols = it)) }
        }
        row {
            Toggle(R.string.generator_avoid_ambiguous, "l, 1, O, 0", options.avoidAmbiguous) {
                onChange(options.copy(avoidAmbiguous = it))
            }
        }
    }
}

@Composable
private fun Toggle(@StringRes label: Int, hint: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    ToggleRow(stringResource(label), checked, onChange, detail = hint)
}

@StringRes
private fun strengthLabel(strength: Strength): Int = when (strength) {
    Strength.WEAK -> R.string.generator_weak
    Strength.FAIR -> R.string.generator_fair
    Strength.STRONG -> R.string.generator_strong
    Strength.EXCELLENT -> R.string.generator_excellent
}
