package net.havenkeys.android.ui.shell

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.ItemRow
import net.havenkeys.android.ui.kit.RowLeading
import uniffi.havenkeys_mobile.BankSummaryRecord
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

/** Opens an item. */
typealias OpenItem = (id: String) -> Unit

/** A login shows its title's initial; the other kinds show what they are. */
internal fun ItemSummary.leading(): RowLeading = when (kind) {
    ItemKind.LOGIN -> RowLeading.Monogram(title)
    ItemKind.SECURE_NOTE -> RowLeading.Glyph(HavenIcon.Note, soft = true)
    ItemKind.CARD -> RowLeading.Glyph(HavenIcon.Card)
    ItemKind.IDENTITY -> RowLeading.Glyph(HavenIcon.IdCard)
    ItemKind.BANK_ACCOUNT -> RowLeading.Glyph(HavenIcon.Bank)
}

/**
 * A username or a card's ending from Rust, a bank account's branch and
 * account in the app's language, else the website's host. Never a secret.
 */
internal fun ItemSummary.secondLine(context: Context): String? =
    (if (kind == ItemKind.BANK_ACCOUNT) bankSubtitle(context, bank) else null) ?: subtitle ?: website

@Composable
internal fun ItemSummary.secondLine(): String? = secondLine(LocalContext.current)

/** `Branch 1234 · •••45-6` (pt-BR `Ag 1234 · …`), either part alone, or the bank's name. */
internal fun bankSubtitle(context: Context, bank: BankSummaryRecord?): String? {
    bank ?: return null
    val parts = listOfNotNull(
        bank.branch?.let { context.getString(R.string.bank_branch_short, it) },
        bank.accountLast3?.let { "•••$it" },
    )
    return if (parts.isEmpty()) bank.bank else parts.joinToString(" · ")
}

/** "Branch 1234, Account ending 45 6": what TalkBack reads for [bankSubtitle]; null when it shows neither part. */
internal fun bankSpoken(context: Context, bank: BankSummaryRecord?): String? = bank?.let {
    listOfNotNull(
        it.branch?.let { b -> context.getString(R.string.bank_branch_spoken, b) },
        it.accountLast3?.let { end -> context.getString(R.string.bank_account_ending, spokenDigits(end)) },
    ).joinToString(", ").ifEmpty { null }
}

/** `45-6` read as "45 6", not "45 minus 6". */
private fun spokenDigits(ending: String): String = ending.replace('-', ' ')

/** What TalkBack hears for a masked second line: a bank account's or a card's ending in words. */
internal fun ItemSummary.spokenLine(context: Context): String? = when (kind) {
    ItemKind.BANK_ACCOUNT -> bankSpoken(context, bank)
    ItemKind.CARD -> subtitle?.takeIf { it.startsWith(CARD_MASK) }
        ?.let { context.getString(R.string.card_ending, it.removePrefix(CARD_MASK)) }
    else -> null
}

/** How Rust masks a card's number in its subtitle. */
private const val CARD_MASK = "•••• "

/** One vault item as a kit row: overview fields only (Android spec §9.4). */
@Composable
fun SummaryRow(
    summary: ItemSummary,
    onOpen: OpenItem,
    modifier: Modifier = Modifier,
) {
    ItemRow(
        title = summary.title,
        subtitle = summary.secondLine(),
        subtitleDescription = summary.spokenLine(LocalContext.current),
        leading = summary.leading(),
        onClick = { onOpen(summary.id) },
        modifier = modifier,
        hasPasskey = summary.hasPasskey,
        hasCode = summary.hasTotp,
    )
}
