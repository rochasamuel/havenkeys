package net.havenkeys.android.autofill

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.app.slice.Slice
import android.content.Context
import android.content.Intent
import android.content.IntentSender
import android.graphics.drawable.Icon
import android.os.Build
import android.service.autofill.Dataset
import android.service.autofill.Field
import android.service.autofill.FillResponse
import android.service.autofill.InlinePresentation
import android.service.autofill.Presentations
import android.service.autofill.SaveInfo
import android.view.View
import android.view.autofill.AutofillId
import android.view.autofill.AutofillValue
import android.view.inputmethod.InlineSuggestionsRequest
import android.widget.RemoteViews
import android.widget.inline.InlinePresentationSpec
import androidx.annotation.DrawableRes
import androidx.annotation.RequiresApi
import androidx.autofill.inline.UiVersions
import androidx.autofill.inline.v1.InlineSuggestionUi
import java.util.concurrent.atomic.AtomicInteger
import net.havenkeys.android.MainActivity
import net.havenkeys.android.R
import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.FillValues

/**
 * Turns a [FillPlan] into what Android shows: a dropdown row, and a keyboard
 * chip on Android 11+ when the keyboard asks for one. Values go only into
 * datasets; an Intent carries nothing but [EXTRA_MODE] and [EXTRA_ITEM_ID].
 */
class DatasetFactory(
    private val context: Context,
    private val screen: ParsedScreen,
    private val form: LoginForm?,
    private val save: SaveForm?,
    inlineRequest: InlineSuggestionsRequest?,
) {
    private val rows = Rows(context, inlineRequest)
    private val usernameId = form?.usernames.orEmpty().idIn(screen)
    private val passwordId = form?.passwords.orEmpty().idIn(screen)
    private val otpId = form?.otps.orEmpty().idIn(screen)
    private val otpOnly = form?.otpOnly == true

    fun response(plan: FillPlan): FillResponse? = when (plan) {
        FillPlan.Nothing -> null
        FillPlan.UnlockFirst -> unlockResponse()
        is FillPlan.Offer -> offerResponse(plan)
    }

    /** The answer to a tapped "fill" or search row: only this login's two values. */
    fun loginDataset(values: FillValues): Dataset? =
        rows.dataset(filled(values), context.getString(R.string.app_name), null, null)

    /** The answer to a tapped "totp" row: the code, into the first code field. */
    fun totpDataset(code: String): Dataset? = rows.dataset(
        listOfNotNull(otpId?.let { it to AutofillValue.forText(code) }),
        context.getString(R.string.app_name),
        null,
        null,
    )

    private fun unlockResponse(): FillResponse? {
        val fields = form?.let { it.usernames + it.passwords + it.otps }.orEmpty()
        return rows.unlockResponse(fields.mapNotNull(screen.ids::getOrNull))
    }

    private fun offerResponse(offer: FillPlan.Offer): FillResponse? {
        // A copy row fills nothing: tapping it opens HavenKeys, which copies that login's code.
        fun copyDataset(match: AutofillMatch) = rows.dataset(
            listOfNotNull(otpId).map { it to null },
            context.getString(R.string.autofill_copy_code),
            match.title,
            autofillSender(context, MODE_COPY_TOTP, match.id),
            R.drawable.ic_autofill_copy,
        )
        val datasets = offer.datasets.mapNotNull(::datasetOf).toMutableList()
        offer.copies.mapNotNullTo(datasets, ::copyDataset)
        if (offer.search) searchDataset()?.let(datasets::add)
        val saveInfo = if (offer.save) save?.saveInfoIn(screen) else null
        if (datasets.isEmpty() && saveInfo == null) return null
        return FillResponse.Builder().apply {
            datasets.forEach(::addDataset)
            saveInfo?.let(::setSaveInfo)
        }.build()
    }

    private fun datasetOf(plan: DatasetPlan): Dataset? {
        val subtitle = if (otpOnly) context.getString(R.string.autofill_code) else plan.match.username
        val gated = plan.values == null && plan.totp == null
        val fields = when {
            gated && otpOnly -> listOfNotNull(otpId).map { it to null }
            gated -> listOfNotNull(usernameId, passwordId).map { it to null }
            plan.totp != null -> listOfNotNull(otpId?.let { it to AutofillValue.forText(plan.totp) })
            else -> filled(requireNotNull(plan.values))
        }
        val mode = if (otpOnly) MODE_TOTP else MODE_FILL
        val auth = if (gated) autofillSender(context, mode, plan.match.id) else null
        return rows.dataset(fields, plan.match.title, subtitle, auth)
    }

    private fun filled(values: FillValues) = listOfNotNull(
        usernameId?.let { id -> values.username?.let { id to AutofillValue.forText(it) } },
        passwordId?.let { id -> values.password?.let { id to AutofillValue.forText(it) } },
    )

    private fun searchDataset(): Dataset? = rows.dataset(
        listOfNotNull(usernameId, passwordId).map { it to null },
        context.getString(R.string.autofill_search),
        null,
        autofillSender(context, MODE_SEARCH, null),
    )

    companion object {
        const val EXTRA_MODE = "net.havenkeys.android.autofill.MODE"
        const val EXTRA_ITEM_ID = "net.havenkeys.android.autofill.ITEM_ID"
        const val MODE_FILL = "fill"
        const val MODE_TOTP = "totp"
        const val MODE_COPY_TOTP = "copy-totp"
        const val MODE_UNLOCK = "unlock"
        const val MODE_SEARCH = "search"
        const val MODE_CARD = "card"
        const val MODE_IDENTITY = "identity"
        const val MODE_IDENTITY_DOCS = "identity-docs"

        /** In the response's client state: which kind of save Android's sheet confirms. */
        const val EXTRA_SAVE_KIND = "net.havenkeys.android.autofill.SAVE_KIND"
        const val SAVE_KIND_CARD = "card"
    }
}

/**
 * Android's save sheet, shown when the form is submitted. Its "Save" is
 * the user's confirmation; Rust decides what it adds or updates.
 */
internal fun SaveForm.saveInfoIn(screen: ParsedScreen): SaveInfo? {
    val passwordId = password?.let(screen.ids::getOrNull)
    val usernameId = username?.let(screen.ids::getOrNull)
    val currentId = current?.let(screen.ids::getOrNull)
    return when {
        passwordId != null -> {
            val types = SaveInfo.SAVE_DATA_TYPE_PASSWORD or
                (if (usernameId != null) SaveInfo.SAVE_DATA_TYPE_USERNAME else 0)
            SaveInfo.Builder(types, arrayOf(passwordId))
                .apply {
                    val optional = listOfNotNull(usernameId, currentId)
                    if (optional.isNotEmpty()) setOptionalIds(optional.toTypedArray())
                }
                // Web pages and single-activity apps rarely finish: save when the fields go away.
                .setFlags(SaveInfo.FLAG_SAVE_ON_ALL_VIEWS_INVISIBLE)
                .build()
        }
        // A username-first sign-in: keep this screen for the password's.
        usernameId != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q ->
            SaveInfo.Builder(SaveInfo.SAVE_DATA_TYPE_USERNAME, arrayOf(usernameId))
                .setFlags(SaveInfo.FLAG_DELAY_SAVE)
                .build()
        else -> null
    }
}

private fun List<Int>.idIn(screen: ParsedScreen): AutofillId? = firstOrNull()?.let(screen.ids::getOrNull)
