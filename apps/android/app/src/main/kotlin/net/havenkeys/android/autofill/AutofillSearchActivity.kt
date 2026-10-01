package net.havenkeys.android.autofill

import android.content.pm.PackageManager
import android.os.Bundle
import android.view.WindowManager
import android.widget.Toast
import androidx.activity.compose.setContent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import net.havenkeys.android.HavenApp
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.AutofillMatch

/**
 * "Search HavenKeys…" (spec §7.2): the user picks a login for an app that
 * nothing binds yet. Only after they confirm "Use <login> in <app>?" does
 * Rust bind it and fill. Not exported: only our PendingIntent reaches it.
 */
class AutofillSearchActivity : FragmentActivity() {
    private val container get() = (application as HavenApp).container

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.setFlags(WindowManager.LayoutParams.FLAG_SECURE, WindowManager.LayoutParams.FLAG_SECURE)
        val tapped = tappedRequest() ?: return cancel()
        val appLabel = appLabel(tapped.screen.packageName)
        unlockThen(container) {
            setContent {
                HavenTheme {
                    SearchScreen(
                        search = container.autofillRepository::search,
                        appLabel = appLabel,
                        onConfirmed = { match -> bindAndFill(tapped, match) },
                    )
                }
            }
        }
    }

    private suspend fun bindAndFill(tapped: TappedRequest, match: AutofillMatch): String? =
        when (val r = container.autofillRepository.bindAndFill(match.id, tapped.target)) {
            is Outcome.Failed -> r.code
            is Outcome.Ok -> {
                val dataset = tapped.factory(this).loginDataset(r.value.values)
                if (dataset != null) {
                    if (!r.value.saved) Toast.makeText(this, R.string.autofill_filled_once, Toast.LENGTH_LONG).show()
                    finishWith(dataset)
                }
                if (dataset == null) NOTHING_TO_FILL else null
            }
        }

    private fun appLabel(packageName: String): String = try {
        packageManager.getApplicationLabel(packageManager.getApplicationInfo(packageName, 0)).toString()
    } catch (@Suppress("SwallowedException") e: PackageManager.NameNotFoundException) {
        packageName
    }

    private companion object {
        const val NOTHING_TO_FILL = "not_found"
    }
}

/** [onConfirmed] answers an error code, or null once it has filled. */
@Composable
private fun SearchScreen(
    search: suspend (String) -> Outcome<List<AutofillMatch>>,
    appLabel: String,
    onConfirmed: suspend (AutofillMatch) -> String?,
) {
    var query by remember { mutableStateOf("") }
    var results by remember { mutableStateOf(emptyList<AutofillMatch>()) }
    var picked by remember { mutableStateOf<AutofillMatch?>(null) }
    var errorCode by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(query) {
        if (query.isBlank()) {
            results = emptyList()
            return@LaunchedEffect
        }
        delay(SEARCH_DEBOUNCE_MS)
        results = (search(query.trim()) as? Outcome.Ok)?.value.orEmpty()
    }

    Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
        Column(modifier = Modifier.safeDrawingPadding().imePadding().padding(16.dp)) {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                label = { Text(stringResource(R.string.autofill_search)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            errorCode?.let {
                Text(
                    stringResource(errorText(it)),
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 8.dp),
                )
            }
            Results(results, searched = query.isNotBlank(), enabled = !busy, onPick = { picked = it })
        }
    }

    picked?.let { match ->
        ConfirmUse(
            title = match.title,
            appLabel = appLabel,
            busy = busy,
            onConfirm = {
                busy = true
                scope.launch {
                    errorCode = onConfirmed(match)
                    busy = false
                    picked = null
                }
            },
            onDismiss = { if (!busy) picked = null },
        )
    }
}

@Composable
private fun Results(
    results: List<AutofillMatch>,
    searched: Boolean,
    enabled: Boolean,
    onPick: (AutofillMatch) -> Unit,
) {
    if (searched && results.isEmpty()) {
        Text(stringResource(R.string.vault_no_matches), modifier = Modifier.padding(top = 16.dp))
    }
    LazyColumn {
        items(results, key = { it.id }) { match ->
            ListItem(
                headlineContent = { Text(match.title) },
                supportingContent = match.username?.let { { Text(it) } },
                modifier = Modifier.clickable(enabled = enabled) { onPick(match) },
            )
        }
    }
}

@Composable
private fun ConfirmUse(
    title: String,
    appLabel: String,
    busy: Boolean,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        text = { Text(stringResource(R.string.autofill_use_in_app, title, appLabel)) },
        confirmButton = {
            TextButton(enabled = !busy, onClick = onConfirm) { Text(stringResource(R.string.autofill_use)) }
        },
        dismissButton = {
            TextButton(enabled = !busy, onClick = onDismiss) { Text(stringResource(R.string.autofill_cancel)) }
        },
    )
}

private const val SEARCH_DEBOUNCE_MS = 200L
