package net.havenkeys.android.autofill

import java.time.YearMonth
import net.havenkeys.android.data.AutofillRepository
import net.havenkeys.android.data.Outcome
import uniffi.havenkeys_mobile.CardChoice
import uniffi.havenkeys_mobile.CardChoices
import uniffi.havenkeys_mobile.CardFrameRoles
import uniffi.havenkeys_mobile.CardValue
import uniffi.havenkeys_mobile.FrameFacts
import uniffi.havenkeys_mobile.IdentityChoice
import uniffi.havenkeys_mobile.IdentityRole
import uniffi.havenkeys_mobile.IdentityValue
import uniffi.havenkeys_mobile.TargetFacts

sealed interface WalletPlan {
    data object Nothing : WalletPlan

    data object UnlockFirst : WalletPlan

    /** [frames]: the form's frames Rust allows, the focused one first; each row's values follow them. */
    data class Cards(val rows: List<CardRow>, val frames: List<CardFrame>, val save: Boolean) : WalletPlan

    /** [values] (for [roles]) null when the row is gated; [documents] only ever through a gated row. */
    data class Identity(
        val choice: IdentityChoice,
        val roles: List<IdentityRole>,
        val values: List<IdentityValue>?,
        val documents: List<IdentityRole>,
    ) : WalletPlan {
        override fun toString() = "Identity(…)"
    }
}

/** [values]: one list per allowed frame; null when the row is gated. */
data class CardRow(
    val card: CardChoice,
    val expired: Boolean,
    val values: List<List<CardValue>>?,
) {
    override fun toString() = "CardRow(…)"
}

/**
 * What to offer a card or identity form. Rust answers which cards, which
 * frames, which roles and every value; nothing is read while locked.
 */
object WalletPlanner {
    const val MAX_CARDS = 5

    /** [direct]: unlocked, "Confirm before filling" off, and not the answer to an unlock row. */
    suspend fun plan(
        routed: Routed,
        target: TargetFacts,
        unlocked: Boolean,
        repo: AutofillRepository,
        direct: Boolean,
        today: YearMonth = YearMonth.now(),
    ): WalletPlan = when {
        routed is Routed.Login || repo.targetKind(target) !is Outcome.Ok -> WalletPlan.Nothing
        !unlocked -> WalletPlan.UnlockFirst
        routed is Routed.Card -> cards(routed.form, target, repo, direct, today)
        routed is Routed.Identity -> identity(routed.form, target, repo, direct)
        else -> WalletPlan.Nothing
    }

    fun frameFacts(frame: CardFrame) = FrameFacts(frame.webDomain, frame.webScheme)

    fun asked(frames: List<CardFrame>) = frames.map { CardFrameRoles(frameFacts(it), CardFormFinder.rolesOf(it)) }

    fun allowedFrames(form: CardForm, choices: CardChoices): List<CardFrame> =
        form.frames.filterIndexed { i, _ -> choices.frames.getOrElse(i) { false } }

    /** The roles the form asks for that the identity has: (plain, documents). */
    fun split(form: IdentityForm, choice: IdentityChoice): Pair<List<IdentityRole>, List<IdentityRole>> {
        val wanted = form.roles.filter { it in choice.roles }
        val (documents, plain) = wanted.partition { it.isDocument }
        return plain to documents
    }

    private suspend fun cards(
        form: CardForm,
        target: TargetFacts,
        repo: AutofillRepository,
        direct: Boolean,
        today: YearMonth,
    ): WalletPlan {
        val found = repo.cards(target, form.frames.map(::frameFacts))
        val choices = (found as? Outcome.Ok)?.value
        return when {
            found is Outcome.Failed -> refusal(found)
            choices == null || choices.insecure || choices.frames.firstOrNull() != true -> WalletPlan.Nothing
            else -> {
                val frames = allowedFrames(form, choices)
                val rows = rows(ordered(choices.cards, today), frames, target, repo, direct)
                val save = frames.first().fields.any { it.kind == CardKind.NUMBER }
                if (rows.isEmpty() && !save) WalletPlan.Nothing else WalletPlan.Cards(rows, frames, save)
            }
        }
    }

    private suspend fun rows(
        ordered: List<Pair<CardChoice, Boolean>>,
        frames: List<CardFrame>,
        target: TargetFacts,
        repo: AutofillRepository,
        direct: Boolean,
    ): List<CardRow> = ordered
        .mapNotNull { (card, expired) ->
            if (direct) {
                val values = (repo.cardValues(card.id, target, asked(frames)) as? Outcome.Ok)?.value
                values?.let { CardRow(card, expired, it) }
            } else {
                CardRow(card, expired, null)
            }
        }

    private suspend fun identity(
        form: IdentityForm,
        target: TargetFacts,
        repo: AutofillRepository,
        direct: Boolean,
    ): WalletPlan {
        val frame = FrameFacts(form.webDomain, form.webScheme)
        val found = repo.identity(target, frame)
        val choice = (found as? Outcome.Ok)?.value
        val (plain, documents) = choice?.let { split(form, it) } ?: (emptyList<IdentityRole>() to emptyList())
        return when {
            found is Outcome.Failed -> refusal(found)
            choice == null || (plain.isEmpty() && documents.isEmpty()) -> WalletPlan.Nothing
            else -> {
                val values = if (direct && plain.isNotEmpty()) {
                    (repo.identityValues(target, frame, plain, false) as? Outcome.Ok)?.value
                } else {
                    null
                }
                WalletPlan.Identity(choice, plain, values, documents)
            }
        }
    }

    /** Unexpired cards first, at most [MAX_CARDS], each with whether it is expired. */
    private fun ordered(cards: List<CardChoice>, today: YearMonth) = cards
        .map { card ->
            val expiry = card.expiry?.let { runCatching { YearMonth.parse(it) }.getOrNull() }
            card to (expiry?.isBefore(today) == true)
        }
        .sortedBy { (_, expired) -> expired }
        .take(MAX_CARDS)

    // Rust applied an overdue auto-lock on this very request.
    private fun refusal(failed: Outcome.Failed) =
        if (failed.code == "locked") WalletPlan.UnlockFirst else WalletPlan.Nothing
}
