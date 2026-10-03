package net.havenkeys.android.ui.items

import androidx.annotation.StringRes
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

/** The lists Items offers (spec §6.7). A route carries [arg], never a query. */
enum class Category(val arg: String, @StringRes val label: Int, val icon: HavenIcon) {
    ALL("all", R.string.vault_filter_all, HavenIcon.Grid),
    LOGINS("logins", R.string.vault_filter_logins, HavenIcon.Globe),
    PASSKEYS("passkeys", R.string.vault_filter_passkeys, HavenIcon.Key),
    NOTES("notes", R.string.vault_filter_notes, HavenIcon.Note),
    CARDS("cards", R.string.vault_filter_cards, HavenIcon.Card),
    ;

    fun keeps(item: ItemSummary): Boolean = when (this) {
        ALL -> true
        LOGINS -> item.kind == ItemKind.LOGIN
        PASSKEYS -> item.kind == ItemKind.LOGIN && item.hasPasskey
        NOTES -> item.kind == ItemKind.SECURE_NOTE
        CARDS -> item.kind == ItemKind.CARD
    }

    companion object {
        /** Null for anything else, the identity included: a restored or forged route opens nothing. */
        fun fromArg(arg: String?): Category? = entries.firstOrNull { it.arg == arg }
    }
}
