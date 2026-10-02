package net.havenkeys.android.autofill

import android.content.Context
import android.service.autofill.Dataset
import android.service.autofill.FillResponse
import android.view.autofill.AutofillId
import android.view.autofill.AutofillValue
import android.view.inputmethod.InlineSuggestionsRequest
import net.havenkeys.android.R
import uniffi.havenkeys_mobile.CardChoice
import uniffi.havenkeys_mobile.CardValue
import uniffi.havenkeys_mobile.IdentityChoice
import uniffi.havenkeys_mobile.IdentityValue

/**
 * Turns a [WalletPlan] into Android's rows. A direct row carries its
 * values; a gated row carries none and opens HavenKeys, which asks the
 * user before Rust is asked (AutofillAuthActivity). Intents carry only the
 * mode and a card's ID.
 */
class WalletDatasets(
    private val context: Context,
    private val screen: ParsedScreen,
    private val routed: Routed,
    inlineRequest: InlineSuggestionsRequest?,
) {
    private val rows = Rows(context, inlineRequest)

    fun response(plan: WalletPlan): FillResponse? = when (plan) {
        WalletPlan.Nothing -> null
        WalletPlan.UnlockFirst -> rows.unlockResponse(ids(unlockTargets()))
        is WalletPlan.Cards -> cards(plan)
        is WalletPlan.Identity -> identity(plan)
    }

    /** The answer to a confirmed card row: this card's values only. */
    fun cardDataset(card: CardChoice, frames: List<CardFrame>, values: List<List<CardValue>>): Dataset? =
        rows.dataset(valuedIn(screen, WalletEntries.card(screen.fields, frames, values)), cardTitle(card), null, null)

    /** The answer to a confirmed identity row. */
    fun identityDataset(form: IdentityForm, choice: IdentityChoice, values: List<IdentityValue>): Dataset? =
        rows.dataset(
            valuedIn(screen, WalletEntries.identity(screen.fields, form, values)),
            identityTitle(choice),
            null,
            null,
        )

    private fun cards(plan: WalletPlan.Cards): FillResponse? {
        val expiredWord = context.getString(R.string.autofill_card_expired)
        val datasets = plan.rows.mapNotNull { row ->
            val subtitle = cardSubtitle(row.card, row.expired, expiredWord)
            val values = row.values
            if (values == null) {
                val auth = autofillSender(context, DatasetFactory.MODE_CARD, row.card.id)
                val targets = gated(WalletEntries.cardTargets(screen.fields, plan.frames))
                rows.dataset(targets, cardTitle(row.card), subtitle, auth)
            } else {
                val entries = valuedIn(screen, WalletEntries.card(screen.fields, plan.frames, values))
                rows.dataset(entries, cardTitle(row.card), subtitle, null)
            }
        }
        val saveInfo = if (plan.save) cardSaveInfo(screen, plan.frames.first()) else null
        return responseOf(datasets, saveInfo, saveInfo?.let { DatasetFactory.SAVE_KIND_CARD })
    }

    private fun identity(plan: WalletPlan.Identity): FillResponse? {
        val identity = routed as? Routed.Identity ?: return null
        val form = identity.form
        val title = identityTitle(plan.choice)
        val values = plan.values
        val main = when {
            plan.roles.isEmpty() -> null
            values == null -> rows.dataset(
                gated(WalletEntries.identityTargets(screen.fields, form, plan.roles)),
                title,
                plan.choice.email,
                autofillSender(context, DatasetFactory.MODE_IDENTITY, null),
            )
            else -> rows.dataset(
                valuedIn(screen, WalletEntries.identity(screen.fields, form, values)),
                title,
                plan.choice.email,
                null,
            )
        }
        val documents = plan.documents.takeIf { it.isNotEmpty() }?.let { docs ->
            rows.dataset(
                gated(WalletEntries.identityTargets(screen.fields, form, plan.roles + docs)),
                context.getString(R.string.autofill_identity_with_docs, documentNames(context, docs)),
                title,
                autofillSender(context, DatasetFactory.MODE_IDENTITY_DOCS, null),
            )
        }
        return responseOf(listOfNotNull(main, documents), identity.save?.saveInfoIn(screen), null)
    }

    private fun unlockTargets(): List<Int> = when (routed) {
        is Routed.Card -> WalletEntries.cardTargets(screen.fields, routed.form.frames)
        is Routed.Identity -> WalletEntries.identityTargets(screen.fields, routed.form, routed.form.roles)
        is Routed.Login -> emptyList()
    }

    private fun cardTitle(card: CardChoice) = card.title.ifBlank { context.getString(R.string.autofill_card_fallback) }

    private fun identityTitle(choice: IdentityChoice) =
        choice.title.ifBlank { context.getString(R.string.autofill_identity_fallback) }

    private fun ids(indices: List<Int>): List<AutofillId> = indices.mapNotNull(screen.ids::getOrNull)

    private fun gated(indices: List<Int>): List<Pair<AutofillId, AutofillValue?>> = ids(indices).map { it to null }
}
