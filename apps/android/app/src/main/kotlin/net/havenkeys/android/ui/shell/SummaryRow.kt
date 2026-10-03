package net.havenkeys.android.ui.shell

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.ItemRow
import net.havenkeys.android.ui.kit.RowLeading
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

/** Opens an item; [origin] names the list the row was in (an item can be in two lists at once). */
typealias OpenItem = (id: String, origin: String) -> Unit

/** The modifier that carries a row's title to the item screen; nothing by default. */
typealias SharedTitle = @Composable (id: String, origin: String) -> Modifier

val NoSharedTitle: SharedTitle = { _, _ -> Modifier }

/** The lists a row can be opened from. */
object Origins {
    const val IDENTITY = "identity"
    const val RECENT = "recent"
    const val FREQUENT = "frequent"
    const val SEARCH = "search"
    const val CATEGORY = "category"
}

/** A login shows its title's initial; the other kinds show what they are. */
internal fun ItemSummary.leading(): RowLeading = when (kind) {
    ItemKind.LOGIN -> RowLeading.Monogram(title)
    ItemKind.SECURE_NOTE -> RowLeading.Glyph(HavenIcon.Note, soft = true)
    ItemKind.CARD -> RowLeading.Glyph(HavenIcon.Card)
    ItemKind.IDENTITY -> RowLeading.Glyph(HavenIcon.IdCard)
}

/** A username or a card's ending from Rust, else the website's host. Never a secret. */
internal fun ItemSummary.secondLine(): String? = subtitle ?: website

/** One vault item as a kit row: overview fields only (Android spec §9.4). */
@Composable
fun SummaryRow(
    summary: ItemSummary,
    origin: String,
    onOpen: OpenItem,
    modifier: Modifier = Modifier,
    sharedTitle: SharedTitle = NoSharedTitle,
) {
    ItemRow(
        title = summary.title,
        subtitle = summary.secondLine(),
        leading = summary.leading(),
        onClick = { onOpen(summary.id, origin) },
        modifier = modifier,
        titleModifier = sharedTitle(summary.id, origin),
        hasPasskey = summary.hasPasskey,
        hasCode = summary.hasTotp,
    )
}
