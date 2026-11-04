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
        leading = summary.leading(),
        onClick = { onOpen(summary.id) },
        modifier = modifier,
        hasPasskey = summary.hasPasskey,
        hasCode = summary.hasTotp,
    )
}
