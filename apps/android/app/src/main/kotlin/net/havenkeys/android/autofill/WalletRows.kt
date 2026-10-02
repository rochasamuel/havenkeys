package net.havenkeys.android.autofill

import android.content.Context
import android.os.Bundle
import android.service.autofill.Dataset
import android.service.autofill.FillResponse
import android.service.autofill.SaveInfo
import android.view.autofill.AutofillId
import android.view.autofill.AutofillValue
import net.havenkeys.android.R
import uniffi.havenkeys_mobile.CardChoice
import uniffi.havenkeys_mobile.IdentityRole

/** `•••• 1111 · 04/33`, and "Expired" for an expired card. Never more of the number. */
internal fun cardSubtitle(card: CardChoice, expired: Boolean, expiredWord: String): String = listOfNotNull(
    card.last4?.let { "•••• $it" },
    card.expiry?.let(::shortExpiry),
    expiredWord.takeIf { expired },
).joinToString(" · ")

/** `"2033-04"` → `"04/33"`; null for anything else. */
internal fun shortExpiry(expiry: String): String? =
    WIRE_EXPIRY.matchEntire(expiry)?.let { "${it.groupValues[2]}/${it.groupValues[1].takeLast(2)}" }

internal fun Shaped.autofillValue(): AutofillValue = when (this) {
    is Shaped.Text -> AutofillValue.forText(text)
    is Shaped.Pick -> AutofillValue.forList(index)
    is Shaped.Date -> AutofillValue.forDate(epochMillis)
}

internal fun valuedIn(
    screen: ParsedScreen,
    entries: List<Pair<Int, Shaped>>,
): List<Pair<AutofillId, AutofillValue?>> =
    entries.mapNotNull { (index, value) -> screen.ids.getOrNull(index)?.let { it to value.autofillValue() } }

internal fun documentNames(context: Context, roles: List<IdentityRole>): String = roles.mapNotNull {
    when (it) {
        IdentityRole.CPF -> R.string.autofill_doc_cpf
        IdentityRole.RG -> R.string.autofill_doc_rg
        IdentityRole.PASSPORT -> R.string.autofill_doc_passport
        IdentityRole.DRIVERS_LICENSE -> R.string.autofill_doc_drivers_license
        else -> null
    }
}.joinToString(", ") { context.getString(it) }

/** Android's card save sheet for the focused frame: the number required, the rest optional. */
internal fun cardSaveInfo(screen: ParsedScreen, frame: CardFrame): SaveInfo? {
    val ids = frame.fields.mapNotNull { f -> screen.ids.getOrNull(f.index)?.let { f.kind to it } }
    val required = ids.firstOrNull { it.first == CardKind.NUMBER }?.second
    val optional = ids.map { it.second }.filter { it != required }.distinct()
    return required?.let {
        SaveInfo.Builder(SaveInfo.SAVE_DATA_TYPE_CREDIT_CARD, arrayOf(it))
            .apply { if (optional.isNotEmpty()) setOptionalIds(optional.toTypedArray()) }
            // Checkouts rarely finish their activity: save when the fields go away.
            .setFlags(SaveInfo.FLAG_SAVE_ON_ALL_VIEWS_INVISIBLE)
            .build()
    }
}

/** Null when there is nothing to show or save. [saveKind] tells `onSaveRequest` what the sheet saves. */
internal fun responseOf(datasets: List<Dataset>, saveInfo: SaveInfo?, saveKind: String?): FillResponse? =
    if (datasets.isEmpty() && saveInfo == null) {
        null
    } else {
        FillResponse.Builder().apply {
            datasets.forEach(::addDataset)
            saveInfo?.let(::setSaveInfo)
            saveKind?.let { setClientState(Bundle().apply { putString(DatasetFactory.EXTRA_SAVE_KIND, it) }) }
        }.build()
    }

private val WIRE_EXPIRY = Regex("(\\d{4})-(\\d{2})")
