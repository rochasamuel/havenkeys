package net.havenkeys.android.autofill

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.app.slice.Slice
import android.content.Context
import android.content.Intent
import android.content.IntentSender
import android.os.Build
import android.service.autofill.Dataset
import android.service.autofill.Field
import android.service.autofill.FillResponse
import android.service.autofill.InlinePresentation
import android.service.autofill.Presentations
import android.view.View
import android.view.autofill.AutofillId
import android.view.autofill.AutofillValue
import android.view.inputmethod.InlineSuggestionsRequest
import android.widget.RemoteViews
import android.widget.inline.InlinePresentationSpec
import androidx.annotation.RequiresApi
import androidx.autofill.inline.UiVersions
import androidx.autofill.inline.v1.InlineSuggestionUi
import java.util.concurrent.atomic.AtomicInteger
import net.havenkeys.android.MainActivity
import net.havenkeys.android.R
import uniffi.havenkeys_mobile.FillValues

/**
 * Turns a [FillPlan] into what Android shows: a dropdown row, and a keyboard
 * chip on Android 11+ when the keyboard asks for one. Values go only into
 * datasets; an Intent carries nothing but [EXTRA_MODE] and [EXTRA_ITEM_ID].
 */
class DatasetFactory(
    private val context: Context,
    private val screen: ParsedScreen,
    private val form: LoginForm,
    inlineRequest: InlineSuggestionsRequest?,
) {
    private val rows = Rows(context, inlineRequest)
    private val usernameId = form.usernames.idIn(screen)
    private val passwordId = form.passwords.idIn(screen)
    private val otpId = form.otps.idIn(screen)

    fun response(plan: FillPlan): FillResponse? = when (plan) {
        FillPlan.Nothing -> null
        FillPlan.UnlockFirst -> unlockResponse()
        is FillPlan.Offer -> offerResponse(plan)
    }

    /** The answer to a tapped "fill" or search row: only this login's two values. */
    fun loginDataset(values: FillValues): Dataset? =
        dataset(filled(values), context.getString(R.string.app_name), null, null)

    /** The answer to a tapped "totp" row: the code, into the first code field. */
    fun totpDataset(code: String): Dataset? = dataset(
        listOfNotNull(otpId?.let { it to AutofillValue.forText(code) }),
        context.getString(R.string.app_name),
        null,
        null,
    )

    // Locked: one row that names no login; the fields' ids are all Android gets.
    private fun unlockResponse(): FillResponse? {
        val ids = (form.usernames + form.passwords + form.otps).mapNotNull(screen.ids::getOrNull).toTypedArray()
        if (ids.isEmpty()) return null
        val title = context.getString(R.string.autofill_unlock)
        val menu = rows.menu(title, null)
        val inline = rows.inline(title, null)
        val sender = sender(context, MODE_UNLOCK, null)
        val builder = FillResponse.Builder()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            builder.setAuthentication(ids, sender, rows.presentations(menu, inline))
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && inline != null) {
            @Suppress("DEPRECATION")
            builder.setAuthentication(ids, sender, menu, inline)
        } else {
            @Suppress("DEPRECATION")
            builder.setAuthentication(ids, sender, menu)
        }
        return builder.build()
    }

    private fun offerResponse(offer: FillPlan.Offer): FillResponse? {
        val datasets = offer.datasets.mapNotNull(::datasetOf).toMutableList()
        if (offer.search) searchDataset()?.let(datasets::add)
        if (datasets.isEmpty()) return null
        return FillResponse.Builder().apply { datasets.forEach(::addDataset) }.build()
    }

    private fun datasetOf(plan: DatasetPlan): Dataset? {
        val subtitle = if (form.otpOnly) context.getString(R.string.autofill_code) else plan.match.username
        val gated = plan.values == null && plan.totp == null
        val fields = when {
            gated && form.otpOnly -> listOfNotNull(otpId).map { it to null }
            gated -> listOfNotNull(usernameId, passwordId).map { it to null }
            plan.totp != null -> listOfNotNull(otpId?.let { it to AutofillValue.forText(plan.totp) })
            else -> filled(requireNotNull(plan.values))
        }
        val auth = if (gated) sender(context, if (form.otpOnly) MODE_TOTP else MODE_FILL, plan.match.id) else null
        return dataset(fields, plan.match.title, subtitle, auth)
    }

    private fun filled(values: FillValues) = listOfNotNull(
        usernameId?.let { id -> values.username?.let { id to AutofillValue.forText(it) } },
        passwordId?.let { id -> values.password?.let { id to AutofillValue.forText(it) } },
    )

    private fun searchDataset(): Dataset? = dataset(
        listOfNotNull(usernameId, passwordId).map { it to null },
        context.getString(R.string.autofill_search),
        null,
        sender(context, MODE_SEARCH, null),
    )

    /** Null when there is no field to fill: Android refuses a dataset without one. */
    private fun dataset(
        fields: List<Pair<AutofillId, AutofillValue?>>,
        title: String,
        subtitle: String?,
        auth: IntentSender?,
    ): Dataset? {
        if (fields.isEmpty()) return null
        val menu = rows.menu(title, subtitle)
        val inline = rows.inline(title, subtitle)
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val builder = Dataset.Builder(rows.presentations(menu, inline))
            fields.forEach { (id, value) -> builder.setField(id, value?.let { Field.Builder().setValue(it).build() }) }
            auth?.let(builder::setAuthentication)
            builder.build()
        } else {
            legacyDataset(fields, menu, inline, auth)
        }
    }

    // The builders Android 13 replaced with Presentations and Field.
    @Suppress("DEPRECATION")
    private fun legacyDataset(
        fields: List<Pair<AutofillId, AutofillValue?>>,
        menu: RemoteViews,
        inline: InlinePresentation?,
        auth: IntentSender?,
    ): Dataset {
        val builder = Dataset.Builder(menu)
        fields.forEach { (id, value) -> builder.setValue(id, value) }
        if (inline != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) builder.setInlinePresentation(inline)
        auth?.let(builder::setAuthentication)
        return builder.build()
    }

    companion object {
        const val EXTRA_MODE = "net.havenkeys.android.autofill.MODE"
        const val EXTRA_ITEM_ID = "net.havenkeys.android.autofill.ITEM_ID"
        const val MODE_FILL = "fill"
        const val MODE_TOTP = "totp"
        const val MODE_UNLOCK = "unlock"
        const val MODE_SEARCH = "search"

        // One code per PendingIntent: Android tells them apart only by it,
        // and a reused one would cancel a row still on screen.
        private val nextRequestCode = AtomicInteger(Rows.ATTRIBUTION_REQUEST)

        /**
         * Mutable on purpose: Android adds the structure and the inline
         * request to this Intent when the row is tapped, and an immutable
         * PendingIntent drops what is added. The Intent names our
         * non-exported activity explicitly, and the activity trusts none of
         * its extras: Rust re-checks the item id, and the structure must name
         * the calling app.
         */
        @SuppressLint("UnspecifiedImmutableFlag")
        private fun sender(context: Context, mode: String, itemId: String?): IntentSender {
            val activity =
                if (mode == MODE_SEARCH) AutofillSearchActivity::class.java else AutofillAuthActivity::class.java
            val intent = Intent(context, activity).putExtra(EXTRA_MODE, mode)
            itemId?.let { intent.putExtra(EXTRA_ITEM_ID, it) }
            val mutable = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0
            return PendingIntent.getActivity(
                context,
                nextRequestCode.incrementAndGet(),
                intent,
                PendingIntent.FLAG_CANCEL_CURRENT or mutable,
            ).intentSender
        }
    }
}

private fun List<Int>.idIn(screen: ParsedScreen): AutofillId? = firstOrNull()?.let(screen.ids::getOrNull)

/** How a row looks: a dropdown row, and a keyboard chip while the keyboard has room for one. */
private class Rows(private val context: Context, private val inlineRequest: InlineSuggestionsRequest?) {
    private var position = 0

    fun menu(title: String, subtitle: String?) = RemoteViews(context.packageName, R.layout.autofill_item).apply {
        setTextViewText(R.id.autofill_title, title)
        if (subtitle.isNullOrEmpty()) {
            setViewVisibility(R.id.autofill_subtitle, View.GONE)
        } else {
            setTextViewText(R.id.autofill_subtitle, subtitle)
        }
    }

    fun inline(title: String, subtitle: String?): InlinePresentation? {
        val index = position++
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) inlineAt(index, title, subtitle) else null
    }

    @RequiresApi(Build.VERSION_CODES.R)
    private fun inlineAt(index: Int, title: String, subtitle: String?): InlinePresentation? =
        specFor(index)?.let { InlinePresentation(chip(title, subtitle), it, false) }

    @RequiresApi(Build.VERSION_CODES.TIRAMISU)
    fun presentations(menu: RemoteViews, inline: InlinePresentation?): Presentations = Presentations.Builder()
        .setMenuPresentation(menu)
        .apply { inline?.let(::setInlinePresentation) }
        .build()

    @RequiresApi(Build.VERSION_CODES.R)
    private fun specFor(index: Int): InlinePresentationSpec? {
        val request = inlineRequest?.takeIf { index < it.maxSuggestionCount }
        val specs = request?.inlinePresentationSpecs.orEmpty()
        return specs.getOrNull(minOf(index, specs.size - 1))
            ?.takeIf { UiVersions.getVersions(it.style).contains(UiVersions.INLINE_UI_VERSION_1) }
    }

    // `slice` is how the library's own documentation hands the chip to
    // Android; lint flags it only because SlicedContent declares it
    // library-internal. Slice itself is deprecated but is what the API takes.
    @SuppressLint("RestrictedApi")
    @Suppress("DEPRECATION")
    @RequiresApi(Build.VERSION_CODES.R)
    private fun chip(title: String, subtitle: String?): Slice = InlineSuggestionUi.newContentBuilder(attribution())
        .setTitle(title)
        .apply { if (!subtitle.isNullOrEmpty()) setSubtitle(subtitle) }
        .setContentDescription(listOfNotNull(title, subtitle).joinToString(" "))
        .build()
        .slice

    // Long-pressing a chip opens HavenKeys; nothing is filled from it.
    private fun attribution(): PendingIntent = PendingIntent.getActivity(
        context,
        ATTRIBUTION_REQUEST,
        Intent(context, MainActivity::class.java),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    companion object {
        const val ATTRIBUTION_REQUEST = 0
    }
}
