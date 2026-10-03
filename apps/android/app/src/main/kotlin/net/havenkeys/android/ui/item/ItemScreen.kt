package net.havenkeys.android.ui.item

import android.content.res.Resources
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.ScreenBar
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenMenu
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.ItemTile
import net.havenkeys.android.ui.kit.MenuItem
import net.havenkeys.android.ui.kit.ToastState
import net.havenkeys.android.ui.kit.ToastTone
import net.havenkeys.android.ui.kit.rememberToastState
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.leading
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.ViewField

/** The header's tile: larger than a row's, so the row's tile grows into it. */
private val HeaderTile = 56.dp

/**
 * One item: its overview from the ViewModel, and each hidden field read from
 * Rust only when the user taps reveal or copy. A revealed value or a code
 * lives in this composition only (spec §9.4). [titleModifier] and
 * [tileModifier] carry the tapped row's title and tile into the header.
 */
@Composable
fun ItemScreen(
    viewModel: ItemViewModel,
    clipboard: SensitiveClipboard,
    online: Boolean,
    navigation: ItemNavigation,
    modifier: Modifier = Modifier,
    titleModifier: Modifier = Modifier,
    tileModifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val toasts = rememberToastState()
    val scope = rememberCoroutineScope()
    val resources = LocalResources.current
    val actions = remember(viewModel, clipboard, toasts, scope, resources) {
        FieldActions(viewModel, clipboard, toasts, scope, resources)
    }
    var confirmDelete by remember { mutableStateOf(false) }

    HavenScaffold(
        modifier = modifier,
        topBar = {
            ScreenBar(onBack = navigation.onBack, online = online, onLock = navigation.onLock) {
                ItemActions(state.view, online, navigation.onEdit, onDelete = { confirmDelete = true })
            }
        },
        toastState = toasts,
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = HavenSpacing.gutter)
                .padding(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            state.view?.let { view ->
                ItemHeader(view.summary, titleModifier, tileModifier)
                ItemFields(view, actions)
            }
            state.errorCode?.let { ErrorLine(it) }
        }
    }
    val view = state.view
    if (confirmDelete && view != null) {
        DeleteDialog(
            view,
            onCancel = { confirmDelete = false },
            onDelete = {
                confirmDelete = false
                actions.delete(navigation.onDeleted)
            },
        )
    }
}

@Composable
private fun ItemHeader(summary: ItemSummary, titleModifier: Modifier, tileModifier: Modifier) {
    Row(Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 20.dp), verticalAlignment = Alignment.CenterVertically) {
        ItemTile(summary.leading(), tileModifier, size = HeaderTile)
        Spacer(Modifier.width(14.dp))
        HavenText(
            summary.title,
            Modifier.weight(1f).then(titleModifier).semantics { heading() },
            style = HavenTheme.type.headline,
            color = HavenTheme.colors.textStrong,
        )
    }
}

/**
 * The fields as one group. Each reveal is keyed by its field inside its row,
 * so a reload that moves a field starts that row masked rather than showing
 * a value on the wrong row.
 */
@Composable
private fun ItemFields(view: ItemView, actions: FieldActions) {
    if (view.fields.isEmpty()) return
    InsetGroup {
        view.fields.forEach { field -> row { key(field.key) { FieldRow(field, actions) } } }
    }
}

/** Edit, and Delete behind More; both write to the server, so both need it online. */
@Composable
private fun ItemActions(view: ItemView?, online: Boolean, onEdit: () -> Unit, onDelete: () -> Unit) {
    HavenIconButton(
        HavenIcon.Edit,
        stringResource(R.string.item_edit),
        onClick = onEdit,
        enabled = online && view != null,
    )
    // The identity is never deleted from the phone.
    if (view == null || view.summary.kind == ItemKind.IDENTITY) return
    var open by remember { mutableStateOf(false) }
    Box {
        // Delete is More's only entry, so More itself waits for the server.
        HavenIconButton(
            HavenIcon.More,
            stringResource(R.string.vault_more),
            onClick = { open = true },
            enabled = online,
        )
        HavenMenu(
            expanded = open,
            onDismiss = { open = false },
            items = listOf(MenuItem(stringResource(R.string.item_delete), onDelete, HavenIcon.Trash, danger = true)),
        )
    }
}

@Composable
private fun DeleteDialog(view: ItemView, onCancel: () -> Unit, onDelete: () -> Unit) {
    HavenDialog(
        title = stringResource(R.string.item_confirm_delete, view.summary.title),
        onDismiss = onCancel,
        confirm = DialogAction(stringResource(R.string.item_delete), onDelete, danger = true),
        message = if (view.summary.hasPasskey) stringResource(R.string.item_passkey_warning) else null,
        dismiss = DialogAction(stringResource(R.string.item_cancel), onCancel),
    )
}

/** What a field (and the item) can do; every value passes through here without being kept. */
private class FieldActions(
    val viewModel: ItemViewModel,
    private val clipboard: SensitiveClipboard,
    private val toasts: ToastState,
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
                is Outcome.Failed -> fail(r.code)
            }
        }
    }

    fun copyField(key: String, label: String) {
        scope.launch {
            when (val r = viewModel.reveal(key)) {
                is Outcome.Ok -> copy(label, r.value)
                is Outcome.Failed -> fail(r.code)
            }
        }
    }

    fun delete(onDeleted: () -> Unit) {
        scope.launch {
            when (val r = viewModel.delete()) {
                is Outcome.Ok -> onDeleted()
                is Outcome.Failed -> fail(r.code)
            }
        }
    }

    fun copyShown(label: String, value: String) {
        scope.launch { copy(label, value) }
    }

    private suspend fun copy(label: String, value: String) {
        val seconds = viewModel.clipboardClearSeconds()
        clipboard.copy(label, value, seconds)
        viewModel.copied()
        toasts.show(resources.getString(R.string.copied, label, seconds))
    }

    private fun fail(code: String) = toasts.show(resources.getString(errorText(code)), ToastTone.Alert)
}

@Composable
private fun FieldRow(field: ViewField, actions: FieldActions) {
    val label = stringResource(fieldLabel(field.label))
    val shown = field.value
    when {
        field.kind == FieldKind.TOTP -> CodeField(label, actions)
        shown != null -> ShownRow(label, shown, onCopy = { actions.copyShown(label, shown) })
        else -> {
            val reveal = rememberRevealState()
            SecretRow(
                label,
                reveal.value,
                onReveal = { actions.reveal(field.key, reveal) },
                onCopy = { actions.copyField(field.key, label) },
            )
        }
    }
}

/** The live code, asked of Rust once a second only while the screen is visible. */
@Composable
private fun CodeField(label: String, actions: FieldActions) {
    val ticks = remember(actions) { actions.viewModel.totpTicks() }
    val now by ticks.collectAsStateWithLifecycle(initialValue = null)
    CodeRow(
        label,
        (now as? Outcome.Ok)?.value,
        failed = now is Outcome.Failed,
        onCopy = { code -> actions.copyShown(label, code) },
    )
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
