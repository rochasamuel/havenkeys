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

// One code per PendingIntent: Android tells them apart only by it,
// and a reused one would cancel a row still on screen.
private val nextRequestCode = AtomicInteger(Rows.ATTRIBUTION_REQUEST)

/**
 * Mutable on purpose: Android adds the structure and the inline
 * request to this Intent when the row is tapped, and an immutable
 * PendingIntent drops what is added. The Intent names our
 * non-exported activity explicitly, and the activity trusts none of
 * its extras: Rust re-checks the item id, and the structure must name
 * the calling app. The search mode opens the search activity; every other mode the auth activity.
 */
@SuppressLint("UnspecifiedImmutableFlag")
internal fun autofillSender(context: Context, mode: String, itemId: String?): IntentSender {
    val activity =
        if (mode == DatasetFactory.MODE_SEARCH) AutofillSearchActivity::class.java else AutofillAuthActivity::class.java
    val intent = Intent(context, activity).putExtra(DatasetFactory.EXTRA_MODE, mode)
    itemId?.let { intent.putExtra(DatasetFactory.EXTRA_ITEM_ID, it) }
    val mutable = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0
    return PendingIntent.getActivity(
        context,
        nextRequestCode.incrementAndGet(),
        intent,
        PendingIntent.FLAG_CANCEL_CURRENT or mutable,
    ).intentSender
}

/** How a row looks: a dropdown row, and a keyboard chip while the keyboard has room for one. */
internal class Rows(private val context: Context, private val inlineRequest: InlineSuggestionsRequest?) {
    private var position = 0

    fun menu(title: String, subtitle: String?, @DrawableRes icon: Int? = null) =
        RemoteViews(context.packageName, R.layout.autofill_item).apply {
        icon?.let {
            setImageViewResource(R.id.autofill_icon, it)
            setViewVisibility(R.id.autofill_icon, View.VISIBLE)
        }
        setTextViewText(R.id.autofill_title, title)
        if (subtitle.isNullOrEmpty()) {
            setViewVisibility(R.id.autofill_subtitle, View.GONE)
        } else {
            setTextViewText(R.id.autofill_subtitle, subtitle)
        }
    }

    fun inline(title: String, subtitle: String?, @DrawableRes icon: Int? = null): InlinePresentation? {
        val index = position++
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) inlineAt(index, title, subtitle, icon) else null
    }

    @RequiresApi(Build.VERSION_CODES.R)
    private fun inlineAt(index: Int, title: String, subtitle: String?, icon: Int?): InlinePresentation? =
        specFor(index)?.let { InlinePresentation(chip(title, subtitle, icon), it, false) }

    /** Null when there is no field to fill: Android refuses a dataset without one. */
    fun dataset(
        fields: List<Pair<AutofillId, AutofillValue?>>,
        title: String,
        subtitle: String?,
        auth: IntentSender?,
        @DrawableRes icon: Int? = null,
    ): Dataset? {
        if (fields.isEmpty()) return null
        val menu = menu(title, subtitle, icon)
        val inline = inline(title, subtitle, icon)
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val builder = Dataset.Builder(presentations(menu, inline))
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

    /** Locked: one row that names nothing from the vault; the fields' ids are all Android gets. */
    fun unlockResponse(ids: List<AutofillId>): FillResponse? {
        if (ids.isEmpty()) return null
        val title = context.getString(R.string.autofill_unlock)
        val menu = menu(title, null)
        val inline = inline(title, null)
        val sender = autofillSender(context, DatasetFactory.MODE_UNLOCK, null)
        val builder = FillResponse.Builder()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            builder.setAuthentication(ids.toTypedArray(), sender, presentations(menu, inline))
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && inline != null) {
            @Suppress("DEPRECATION")
            builder.setAuthentication(ids.toTypedArray(), sender, menu, inline)
        } else {
            @Suppress("DEPRECATION")
            builder.setAuthentication(ids.toTypedArray(), sender, menu)
        }
        return builder.build()
    }

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
    private fun chip(title: String, subtitle: String?, icon: Int?): Slice =
        InlineSuggestionUi.newContentBuilder(attribution())
        .apply { icon?.let { setStartIcon(Icon.createWithResource(context, it)) } }
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
