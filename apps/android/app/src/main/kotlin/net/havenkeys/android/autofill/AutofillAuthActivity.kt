package net.havenkeys.android.autofill

import android.app.Activity
import android.app.assist.AssistStructure
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.os.Parcelable
import android.view.autofill.AutofillManager
import android.view.inputmethod.InlineSuggestionsRequest
import android.widget.Toast
import androidx.activity.compose.setContent
import androidx.core.content.IntentCompat
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.viewmodel.compose.viewModel
import kotlinx.coroutines.launch
import net.havenkeys.android.AppContainer
import net.havenkeys.android.HavenApp
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.clipboardClearSeconds
import net.havenkeys.android.security.hardenWindow
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.unlock.UnlockScreen
import net.havenkeys.android.ui.unlock.UnlockViewModel
import uniffi.havenkeys_mobile.LockState
import uniffi.havenkeys_mobile.TargetFacts

/**
 * Opened by a tapped "Unlock HavenKeys" row or a gated dataset (spec §7.3).
 * Unlocks if needed, then asks Rust for the fill; Rust re-checks the target
 * and its refusal returns nothing. Not exported: only our PendingIntents
 * reach it.
 */
class AutofillAuthActivity : FragmentActivity() {
    private val container get() = (application as HavenApp).container
    private var answering = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        hardenWindow()
        val tapped = tappedRequest() ?: return cancel()
        val mode = intent.getStringExtra(DatasetFactory.EXTRA_MODE)
        val itemId = intent.getStringExtra(DatasetFactory.EXTRA_ITEM_ID)
        unlockThen(container) { answer(tapped, mode, itemId) }
    }

    private fun answer(tapped: TappedRequest, mode: String?, itemId: String?) {
        if (answering) return
        answering = true
        when (val routed = tapped.routed) {
            is Routed.Login -> lifecycleScope.launch { answerLogin(tapped, routed, mode, itemId) }
            is Routed.Card -> lifecycleScope.launch { answerCard(tapped, routed.form, mode, itemId) }
            is Routed.Identity -> lifecycleScope.launch { answerIdentity(tapped, routed.form, mode) }
        }
    }

    private suspend fun answerLogin(tapped: TappedRequest, login: Routed.Login, mode: String?, itemId: String?) {
        val repo = container.autofillRepository
        val factory = tapped.factory(this)
        val form = login.form ?: return cancel()
        val result: Parcelable? = when {
            mode == DatasetFactory.MODE_UNLOCK -> factory.response(
                FillPlanner.plan(
                    form,
                    tapped.target,
                    container.isUnlocked(),
                    repo,
                    saveable = login.save != null,
                ),
            )
            itemId == null -> null
            mode == DatasetFactory.MODE_FILL ->
                (repo.fill(itemId, tapped.target) as? Outcome.Ok)?.value?.let(factory::loginDataset)
            mode == DatasetFactory.MODE_TOTP ->
                (repo.totp(itemId, tapped.target) as? Outcome.Ok)?.value?.let(factory::totpDataset)
            mode == DatasetFactory.MODE_COPY_TOTP -> {
                (repo.totp(itemId, tapped.target) as? Outcome.Ok)?.value?.let { copyCode(it) }
                null // Nothing is filled: the copy is the whole answer.
            }
            else -> null
        }
        if (result == null) cancel() else finishWith(result)
    }
}

/**
 * A tapped "Copy code" row: the code Rust gave for this target goes on the
 * clipboard (sensitive, cleared on the usual timer and on lock).
 */
private suspend fun AutofillAuthActivity.copyCode(code: String) {
    val container = (application as HavenApp).container
    val seconds = container.settingsRepository.clipboardClearSeconds()
    container.clipboard.copy(getString(R.string.autofill_code), code, seconds)
    Toast.makeText(applicationContext, getString(R.string.autofill_code_copied, seconds), Toast.LENGTH_LONG).show()
}

/** What a tapped row is about, rebuilt from the structure Android attaches, never from our extras. */
internal class TappedRequest(
    val screen: ParsedScreen,
    val routed: Routed,
    val target: TargetFacts,
    private val inlineRequest: InlineSuggestionsRequest?,
) {
    fun factory(activity: Activity): DatasetFactory {
        val login = routed as? Routed.Login
        return DatasetFactory(activity, screen, login?.form, login?.save, inlineRequest)
    }

    fun wallet(activity: Activity) = WalletDatasets(activity, screen, routed, inlineRequest)
}

/**
 * Null when there is no structure, nothing to fill or no app to name. The
 * app being filled starts us and receives the answer, and it could replace
 * the structure: the structure counts only when it names that same app.
 */
internal fun Activity.tappedRequest(): TappedRequest? {
    val structure = IntentCompat.getParcelableExtra(
        intent,
        AutofillManager.EXTRA_ASSIST_STRUCTURE,
        AssistStructure::class.java,
    )
    val screen = structure
        ?.let { StructureParser().parse(it) }
        ?.takeIf { structureNamesCaller(it.packageName, callingPackage) }
    val routed = screen?.let { FormRouter.route(it.fields) }?.takeIf { it !is Routed.Login || it.form != null }
    if (screen == null || routed == null) return null
    val target = when (routed) {
        is Routed.Login -> packageManager.targetOf(screen, routed.form?.webDomain, routed.form?.webScheme)
        else -> packageManager.targetOf(screen, screen.pageDomain, screen.pageScheme)
    }
    // Android adds the keyboard's request to this Intent from Android 12 on.
    val inlineRequest = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        IntentCompat.getParcelableExtra(
            intent,
            AutofillManager.EXTRA_INLINE_SUGGESTIONS_REQUEST,
            InlineSuggestionsRequest::class.java,
        )
    } else {
        null
    }
    return TappedRequest(screen, routed, target, inlineRequest)
}

/**
 * The mitigation for our mutable PendingIntents: the structure Android
 * attaches can be replaced by the app that starts us, which also receives our
 * answer, so it counts only when it names that same, known app.
 */
internal fun structureNamesCaller(structurePackage: String, callingPackage: String?): Boolean =
    structurePackage.isNotEmpty() && structurePackage == callingPackage

/**
 * Runs [then] once the vault is unlocked, showing the unlock screen first
 * when it is locked. `unlockedHere`: the user just proved who they are on
 * this screen (password, or biometrics through the Keystore).
 */
internal fun FragmentActivity.unlockThen(container: AppContainer, then: (unlockedHere: Boolean) -> Unit) {
    lifecycleScope.launch {
        if (container.isUnlocked()) {
            then(false)
            return@launch
        }
        setContent {
            HavenTheme {
                UnlockScreen(
                    viewModel = viewModel {
                        UnlockViewModel(
                            container.vaultRepository,
                            biometricAvailable = container.biometricGate.available(this@unlockThen),
                            hasBundle = container::hasBiometricUnlock,
                            deleteBundle = container::forgetBiometricUnlock,
                        )
                    },
                    activity = this@unlockThen,
                    container = container,
                    onUnlocked = { then(true) },
                )
            }
        }
    }
}

internal suspend fun AppContainer.isUnlocked(): Boolean =
    (vaultRepository.status() as? Outcome.Ok)?.value?.state == LockState.UNLOCKED

internal fun Activity.cancel() {
    setResult(Activity.RESULT_CANCELED)
    finish()
}

/** The fill goes back to Android as the autofill API requires; nothing else carries it. */
internal fun Activity.finishWith(result: Parcelable) {
    setResult(Activity.RESULT_OK, Intent().putExtra(AutofillManager.EXTRA_AUTHENTICATION_RESULT, result))
    finish()
}
