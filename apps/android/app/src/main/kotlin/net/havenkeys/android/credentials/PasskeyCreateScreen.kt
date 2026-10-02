package net.havenkeys.android.credentials

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.theme.HavenTheme

@Composable
fun PasskeyCreateScreen(
    state: PasskeyCreateUiState,
    onSelect: (String?) -> Unit,
    onSave: () -> Unit,
    onCancel: () -> Unit,
    onClose: () -> Unit,
) {
    Scaffold(containerColor = MaterialTheme.colorScheme.background) { padding ->
        Column(
            Modifier
                .padding(padding)
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            when {
                state.loading -> CircularProgressIndicator(Modifier.align(Alignment.CenterHorizontally))
                state.excluded -> {
                    Text(stringResource(R.string.passkey_exists), style = MaterialTheme.typography.titleMedium)
                    TextButton(onClick = onClose) { Text(stringResource(R.string.passkey_close)) }
                }
                else -> Choice(state, onSelect, onSave, onCancel)
            }
        }
    }
}

@Composable
private fun Choice(state: PasskeyCreateUiState, onSelect: (String?) -> Unit, onSave: () -> Unit, onCancel: () -> Unit) {
    Text(
        stringResource(R.string.passkey_save_title),
        style = MaterialTheme.typography.titleLarge,
        color = HavenTheme.colors.textStrong,
    )
    Text(state.rpId, style = MaterialTheme.typography.bodyLarge)
    Text(
        stringResource(
            R.string.passkey_account,
            state.userName.ifBlank { stringResource(R.string.passkey_no_account_name) },
        ),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Text(stringResource(R.string.passkey_save_to), style = MaterialTheme.typography.titleSmall)
    Column(Modifier.selectableGroup()) {
        state.homes.forEach { home ->
            HomeRow(home.title, home.username, state.selected == home.id) { onSelect(home.id) }
        }
        HomeRow(
            stringResource(R.string.passkey_new_login),
            stringResource(R.string.passkey_new_login_detail),
            state.selected == null,
        ) { onSelect(null) }
    }
    state.error?.let { Text(stringResource(errorText(it)), color = MaterialTheme.colorScheme.error) }
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
        TextButton(onClick = onCancel, modifier = Modifier.weight(1f)) { Text(stringResource(R.string.passkey_cancel)) }
        Button(onClick = onSave, enabled = !state.busy && state.error != "denied", modifier = Modifier.weight(1f)) {
            Text(stringResource(R.string.passkey_save))
        }
    }
}

@Composable
private fun HomeRow(title: String, detail: String?, selected: Boolean, onClick: () -> Unit) {
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = detail?.let { { Text(it) } },
        leadingContent = { RadioButton(selected = selected, onClick = null) },
        modifier = Modifier.selectable(selected = selected, onClick = onClick, role = Role.RadioButton),
    )
}
