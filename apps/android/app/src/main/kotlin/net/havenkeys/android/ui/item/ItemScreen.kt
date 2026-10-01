package net.havenkeys.android.ui.item

import android.content.res.Resources
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.HavenTopBar
import net.havenkeys.android.ui.components.SecretField
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.theme.HavenType
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ViewField

/**
 * One item: its overview from the ViewModel, and each hidden field read from
 * Rust only when the user taps reveal or copy. A revealed value or a code
 * lives in this composition only (spec §9.4).
 */
@Composable
fun ItemScreen(
    viewModel: ItemViewModel,
    clipboard: SensitiveClipboard,
    online: Boolean,
    onBack: () -> Unit,
    onLock: () -> Unit,
    modifier: Modifier = Modifier,
    titleModifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val resources = LocalResources.current
    val actions = remember(viewModel, clipboard, snackbar, scope, resources) {
        FieldActions(viewModel, clipboard, snackbar, scope, resources)
    }

    Scaffold(
        topBar = { HavenTopBar(title = "", online = online, onLock = onLock, onBack = onBack) },
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
            val view = state.view
            if (view != null) {
                Text(
                    text = view.summary.title,
                    style = MaterialTheme.typography.headlineSmall,
                    color = HavenTheme.colors.textStrong,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp).then(titleModifier),
                )
                Surface(
                    color = MaterialTheme.colorScheme.surfaceContainerLow,
                    shape = MaterialTheme.shapes.medium,
                    modifier = Modifier.padding(16.dp),
                ) {
                    Column {
                        view.fields.forEachIndexed { i, field ->
                            // Keyed: after a reload adds or removes a field, a
                            // revealed value stays with its own row.
                            key(field.key) {
                                if (i > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                                FieldRow(field, actions)
                            }
                        }
                    }
                }
            }
            state.errorCode?.let { code ->
                Text(
                    text = stringResource(errorText(code)),
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(16.dp),
                )
            }
        }
    }
}

/** What a field can do; every value passes through here without being kept. */
private class FieldActions(
    val viewModel: ItemViewModel,
    private val clipboard: SensitiveClipboard,
    private val snackbar: SnackbarHostState,
    private val scope: CoroutineScope,
    private val resources: Resources,
) {
    fun reveal(key: String, into: RevealState) {
        if (into.value != null) {
            into.clear()
            return
        }
        scope.launch {
            when (val r = viewModel.reveal(key)) {
                is Outcome.Ok -> into.show(r.value)
                is Outcome.Failed -> snackbar.showSnackbar(resources.getString(errorText(r.code)))
            }
        }
    }

    fun copyField(key: String, label: String) {
        scope.launch {
            when (val r = viewModel.reveal(key)) {
                is Outcome.Ok -> copy(label, r.value)
                is Outcome.Failed -> snackbar.showSnackbar(resources.getString(errorText(r.code)))
            }
        }
    }

    fun copyShown(label: String, value: String) {
        scope.launch { copy(label, value) }
    }

    private suspend fun copy(label: String, value: String) {
        val seconds = viewModel.clipboardClearSeconds()
        clipboard.copy(label, value, seconds)
        snackbar.showSnackbar(resources.getString(R.string.copied, label, seconds))
    }
}

@Composable
private fun FieldRow(field: ViewField, actions: FieldActions) {
    val label = stringResource(fieldLabel(field.label))
    val shown = field.value
    when {
        field.kind == FieldKind.TOTP -> TotpField(label, actions)
        shown != null -> SecretField(
            label = label,
            revealed = shown,
            onReveal = {},
            onCopy = { actions.copyShown(label, shown) },
            masked = false,
        )
        else -> {
            val reveal = rememberRevealState()
            SecretField(
                label = label,
                revealed = reveal.value,
                onReveal = { actions.reveal(field.key, reveal) },
                onCopy = { actions.copyField(field.key, label) },
            )
        }
    }
}

/**
 * The live code. Collected only while the screen is visible, so Rust is not
 * asked for codes in the background.
 */
@Composable
private fun TotpField(label: String, actions: FieldActions) {
    val ticks = remember(actions) { actions.viewModel.totpTicks() }
    val now by ticks.collectAsStateWithLifecycle(initialValue = null)
    val code = (now as? Outcome.Ok)?.value
    Row(
        modifier = Modifier.padding(start = 16.dp, top = 8.dp, bottom = 8.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                text = label,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            when {
                code != null -> Row(
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(groupedCode(code.code), style = HavenType.code, color = MaterialTheme.colorScheme.onSurface)
                    TotpRing(code.secondsRemaining.toInt(), code.period.toInt())
                }
                now is Outcome.Failed -> Text(
                    text = stringResource(R.string.item_code_failed),
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
        if (code != null) {
            IconButton(onClick = { actions.copyShown(label, code.code) }) {
                Icon(Icons.Outlined.ContentCopy, contentDescription = stringResource(R.string.copy, label))
            }
        }
    }
}

private val labels = mapOf(
    "username" to R.string.field_username,
    "password" to R.string.field_password,
    "totp" to R.string.field_totp,
    "website" to R.string.field_website,
    "notes" to R.string.field_notes,
    "content" to R.string.field_content,
    "card.holder" to R.string.field_card_holder,
    "card.number" to R.string.field_card_number,
    "card.code" to R.string.field_card_code,
    "card.expiry" to R.string.field_card_expiry,
    "card.notes" to R.string.field_card_notes,
    "identity.first_name" to R.string.field_identity_first_name,
    "identity.middle_name" to R.string.field_identity_middle_name,
    "identity.last_name" to R.string.field_identity_last_name,
    "identity.gender" to R.string.field_identity_gender,
    "identity.birth_date" to R.string.field_identity_birth_date,
    "identity.occupation" to R.string.field_identity_occupation,
    "identity.company" to R.string.field_identity_company,
    "identity.job_title" to R.string.field_identity_job_title,
    "identity.cpf" to R.string.field_identity_cpf,
    "identity.rg" to R.string.field_identity_rg,
    "identity.passport" to R.string.field_identity_passport,
    "identity.drivers_license" to R.string.field_identity_drivers_license,
    "identity.email" to R.string.field_identity_email,
    "identity.mobile_phone" to R.string.field_identity_mobile_phone,
    "identity.home_phone" to R.string.field_identity_home_phone,
    "identity.work_phone" to R.string.field_identity_work_phone,
    "identity.street" to R.string.field_identity_street,
    "identity.number" to R.string.field_identity_number,
    "identity.complement" to R.string.field_identity_complement,
    "identity.neighborhood" to R.string.field_identity_neighborhood,
    "identity.city" to R.string.field_identity_city,
    "identity.state" to R.string.field_identity_state,
    "identity.postal_code" to R.string.field_identity_postal_code,
    "identity.country" to R.string.field_identity_country,
    "identity.username" to R.string.field_identity_username,
    "identity.website" to R.string.field_identity_website,
    "identity.notes" to R.string.field_identity_notes,
)

/** The text for a `ViewField.label` key; a custom field (empty label) reads "Field". */
@StringRes
internal fun fieldLabel(label: String): Int = labels[label] ?: R.string.field_custom
