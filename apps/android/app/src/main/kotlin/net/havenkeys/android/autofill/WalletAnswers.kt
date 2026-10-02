package net.havenkeys.android.autofill

import android.os.Parcelable
import androidx.activity.compose.setContent
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.launch
import net.havenkeys.android.HavenApp
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.TargetKind

/**
 * A tapped card row. After an unlock: the rows again, every one gated.
 * A gated row: the user confirms here first; then only this card's values,
 * for this form's allowed frames, as one dataset.
 */
internal suspend fun AutofillAuthActivity.answerCard(
    tapped: TappedRequest,
    form: CardForm,
    mode: String?,
    itemId: String?,
) {
    val container = (application as HavenApp).container
    val repo = container.autofillRepository
    when {
        mode == DatasetFactory.MODE_UNLOCK -> finishOrCancel(
            tapped.wallet(this).response(
                WalletPlanner.plan(Routed.Card(form), tapped.target, container.isUnlocked(), repo, direct = false),
            ),
        )
        mode == DatasetFactory.MODE_CARD && itemId != null -> {
            val confirmation = CardConfirmation.prepare(repo, form, tapped.target, itemId) ?: return cancel()
            val card = confirmation.card
            val name = listOfNotNull(card.title.ifBlank { null }, card.last4?.let { "•••• $it" }).joinToString(" ")
            confirm(
                question = getString(R.string.autofill_fill_card_in, name, placeOf(tapped)),
                detail = appDetail(tapped),
                confirmLabel = getString(R.string.autofill_fill),
                alternativeLabel = null,
            ) { _ ->
                val values = confirmation.values(repo, tapped.target)
                finishOrCancel(values?.let { tapped.wallet(this).cardDataset(card, confirmation.frames, it) })
            }
        }
        else -> cancel()
    }
}

/** A tapped identity row: unlock rows again gated; gated rows confirmed here, documents only when chosen. */
internal suspend fun AutofillAuthActivity.answerIdentity(tapped: TappedRequest, form: IdentityForm, mode: String?) {
    if (mode == DatasetFactory.MODE_UNLOCK) return answerIdentityUnlock(tapped, form)
    val repo = (application as HavenApp).container.autofillRepository
    val confirmation = IdentityConfirmation.prepare(repo, form, tapped.target)
    val documents = mode == DatasetFactory.MODE_IDENTITY_DOCS && confirmation?.documents?.isNotEmpty() == true
    if (confirmation == null || (mode != DatasetFactory.MODE_IDENTITY && !documents)) return cancel()
    val place = placeOf(tapped)
    val docNames = documentNames(this, confirmation.documents)
    confirm(
        question = if (documents) {
            getString(R.string.autofill_identity_also_asks, place, docNames)
        } else {
            getString(R.string.autofill_fill_identity_in, place)
        },
        detail = appDetail(tapped),
        confirmLabel = if (documents) {
            getString(R.string.autofill_identity_with_docs, docNames)
        } else {
            getString(R.string.autofill_fill)
        },
        alternativeLabel = if (documents && confirmation.roles.isNotEmpty()) {
            getString(R.string.autofill_identity_without_docs)
        } else {
            null
        },
    ) { withDocuments ->
        val values = confirmation.values(repo, tapped.target, withDocuments && documents)
        finishOrCancel(values?.let { tapped.wallet(this).identityDataset(form, confirmation.choice, it) })
    }
}

private suspend fun AutofillAuthActivity.answerIdentityUnlock(tapped: TappedRequest, form: IdentityForm) {
    val container = (application as HavenApp).container
    val plan = WalletPlanner.plan(
        Routed.Identity(form, null),
        tapped.target,
        container.isUnlocked(),
        container.autofillRepository,
        direct = false,
    )
    finishOrCancel(tapped.wallet(this).response(plan))
}

/**
 * Shows the question; [then] runs once, with true for the main button and
 * false for the alternative. Cancel answers nothing.
 */
private fun AutofillAuthActivity.confirm(
    question: String,
    detail: String?,
    confirmLabel: String,
    alternativeLabel: String?,
    then: suspend (Boolean) -> Unit,
) {
    setContent {
        HavenTheme {
            WalletConfirmDialog(
                question = question,
                detail = detail,
                confirmLabel = confirmLabel,
                alternativeLabel = alternativeLabel,
                onConfirm = { lifecycleScope.launch { then(true) } },
                onAlternative = { lifecycleScope.launch { then(false) } },
                onDismiss = { cancel() },
            )
        }
    }
}

/** The site a browser shows, or the app's package: what identifies it (spec §7.2), never the app's own label. */
private suspend fun AutofillAuthActivity.placeOf(tapped: TappedRequest): String {
    val repo = (application as HavenApp).container.autofillRepository
    val browser = (repo.targetKind(tapped.target) as? Outcome.Ok)?.value == TargetKind.BROWSER
    return if (browser) tapped.target.webDomain.orEmpty() else tapped.target.packageName
}

/** For an app, its own label as a caution, as "Use … in …?" shows it. */
private fun AutofillAuthActivity.appDetail(tapped: TappedRequest): String? =
    SaveRequestReader(packageManager).appTitle(tapped.target.packageName)
        ?.takeIf { it.isNotBlank() && it != tapped.target.packageName && tapped.target.webDomain == null }
        ?.let { getString(R.string.autofill_app_label, it) }

private fun AutofillAuthActivity.finishOrCancel(result: Parcelable?) {
    if (result == null) cancel() else finishWith(result)
}
