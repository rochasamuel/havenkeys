package net.havenkeys.android.autofill

import net.havenkeys.android.data.AutofillRepository
import net.havenkeys.android.data.Outcome
import uniffi.havenkeys_mobile.CardChoice
import uniffi.havenkeys_mobile.CardValue
import uniffi.havenkeys_mobile.FrameFacts
import uniffi.havenkeys_mobile.IdentityChoice
import uniffi.havenkeys_mobile.IdentityRole
import uniffi.havenkeys_mobile.IdentityValue
import uniffi.havenkeys_mobile.TargetFacts

/**
 * A gated card row the user is being asked about. Preparing reads only the
 * overview; Rust is asked for values only by [values], after the user's tap.
 */
internal class CardConfirmation(val card: CardChoice, val frames: List<CardFrame>) {
    suspend fun values(repo: AutofillRepository, target: TargetFacts): List<List<CardValue>>? =
        (repo.cardValues(card.id, target, WalletPlanner.asked(frames)) as? Outcome.Ok)?.value

    companion object {
        /**
         * Null unless [itemId] names a card Rust offers this form, and the
         * focused frame may get it. The ID comes from our own row's Intent,
         * which the app being filled can rewrite.
         */
        suspend fun prepare(
            repo: AutofillRepository,
            form: CardForm,
            target: TargetFacts,
            itemId: String,
        ): CardConfirmation? {
            val choices = (repo.cards(target, form.frames.map(WalletPlanner::frameFacts)) as? Outcome.Ok)?.value
            val card = choices?.cards?.firstOrNull { it.id == itemId }
            return if (card != null && !choices.insecure && choices.frames.firstOrNull() == true) {
                CardConfirmation(card, WalletPlanner.allowedFrames(form, choices))
            } else {
                null
            }
        }
    }
}

/** A gated identity row; [documents] are asked for only when the user chose them. */
internal class IdentityConfirmation(
    val choice: IdentityChoice,
    val roles: List<IdentityRole>,
    val documents: List<IdentityRole>,
    val frame: FrameFacts,
) {
    suspend fun values(repo: AutofillRepository, target: TargetFacts, withDocuments: Boolean): List<IdentityValue>? {
        val asked = if (withDocuments) roles + documents else roles
        return (repo.identityValues(target, frame, asked, withDocuments) as? Outcome.Ok)?.value
    }

    companion object {
        suspend fun prepare(repo: AutofillRepository, form: IdentityForm, target: TargetFacts): IdentityConfirmation? {
            val frame = FrameFacts(form.webDomain, form.webScheme)
            val choice = (repo.identity(target, frame) as? Outcome.Ok)?.value
            return choice?.let { c ->
                val (plain, documents) = WalletPlanner.split(form, c)
                IdentityConfirmation(c, plain, documents, frame).takeIf { plain.isNotEmpty() || documents.isNotEmpty() }
            }
        }
    }
}
