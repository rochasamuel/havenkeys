package net.havenkeys.android.autofill

import net.havenkeys.android.data.AutofillRepository
import net.havenkeys.android.data.Outcome
import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.FillValues
import uniffi.havenkeys_mobile.TargetFacts
import uniffi.havenkeys_mobile.TargetKind

sealed interface FillPlan {
    data object Nothing : FillPlan
    data object UnlockFirst : FillPlan
    /**
     * [copies]: the logins whose code is also offered for copying, on a code
     * step. A code on its own field is filled; one split across a box per
     * digit (Android fills only the tapped box) is pasted from the copy.
     */
    data class Offer(
        val datasets: List<DatasetPlan>,
        val search: Boolean,
        val copies: List<AutofillMatch> = emptyList(),
        /** Attach a SaveInfo: the screen has a login to save. */
        val save: Boolean = false,
    ) : FillPlan
}

/** `values`/`totp` null: the dataset is gated and Rust is asked on tap. */
data class DatasetPlan(val match: AutofillMatch, val values: FillValues?, val totp: String?) {
    // The values are a password and a code; the match names the login.
    override fun toString() = "DatasetPlan(…)"
}

/** A code-only step (the second page of a sign-in): offered the codes, never the login. */
val LoginForm.otpOnly: Boolean get() = otps.isNotEmpty() && passwords.isEmpty() && usernames.isEmpty()

val LoginForm.hasLoginFields: Boolean get() = usernames.isNotEmpty() || passwords.isNotEmpty()

/**
 * What to offer for one fill request. Every answer comes from Rust: the
 * target, the matches and each value. A match Rust refuses to fill is not
 * offered, and nothing from the vault is read while it is locked.
 */
object FillPlanner {
    /** Matches offered per request (the "Search HavenKeys…" entry comes on top). */
    const val MAX_DATASETS = 5

    /**
     * [form] is null when the screen has nothing to fill (a sign-up form);
     * [saveable] when it has something to save. Saving is offered only while
     * unlocked.
     */
    suspend fun plan(
        form: LoginForm?,
        target: TargetFacts,
        unlocked: Boolean,
        repo: AutofillRepository,
        saveable: Boolean = false,
    ): FillPlan {
        val kind = repo.targetKind(target).valueOrNull()
        return when {
            kind == null -> FillPlan.Nothing
            !unlocked -> if (form != null) FillPlan.UnlockFirst else FillPlan.Nothing
            // Nothing to fill: no login is read at all.
            form == null -> if (saveable) FillPlan.Offer(emptyList(), search = false, save = true) else FillPlan.Nothing
            else -> offer(form, target, kind, repo, saveable)
        }
    }

    private suspend fun offer(
        form: LoginForm,
        target: TargetFacts,
        kind: TargetKind,
        repo: AutofillRepository,
        saveable: Boolean,
    ): FillPlan {
        val matches = when (val found = repo.matches(target)) {
            is Outcome.Ok -> found.value
            // Rust applied an overdue auto-lock on this very request.
            is Outcome.Failed -> return if (found.code == "locked") FillPlan.UnlockFirst else FillPlan.Nothing
        }
        val datasets = datasetsOf(matches, form, target, repo)
        val search = kind == TargetKind.APP && form.hasLoginFields
        // The code itself is asked of Rust only when a copy is tapped.
        val copies = if (form.otpOnly) datasets.map { it.match } else emptyList()
        val offered = datasets.isNotEmpty() || search || saveable
        return if (offered) FillPlan.Offer(datasets, search, copies, save = saveable) else FillPlan.Nothing
    }

    private suspend fun datasetsOf(
        matches: List<AutofillMatch>,
        form: LoginForm,
        target: TargetFacts,
        repo: AutofillRepository,
    ): List<DatasetPlan> {
        val direct = !repo.confirmBeforeFilling()
        return matches
            .filter { !form.otpOnly || it.hasTotp }
            .take(MAX_DATASETS)
            .mapNotNull { match ->
                when {
                    !direct -> DatasetPlan(match, null, null)
                    form.otpOnly -> repo.totp(match.id, target).valueOrNull()?.let { DatasetPlan(match, null, it) }
                    else -> repo.fill(match.id, target).valueOrNull()?.let { DatasetPlan(match, it, null) }
                }
            }
    }

    private fun <T> Outcome<T>.valueOrNull(): T? = (this as? Outcome.Ok)?.value
}
