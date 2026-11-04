package net.havenkeys.android.ui.item

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowField
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.InsetGroup
import uniffi.havenkeys_mobile.BankAccountType
import uniffi.havenkeys_mobile.BankField
import uniffi.havenkeys_mobile.BankViewRecord

/** A number and its check digit as people write them: `1234-5`. */
internal fun joinDigit(number: String, digit: String?): String = if (digit.isNullOrEmpty()) number else "$number-$digit"

@StringRes
internal fun accountTypeLabel(type: BankAccountType): Int = when (type) {
    BankAccountType.CHECKING -> R.string.bank_type_checking
    BankAccountType.SAVINGS -> R.string.bank_type_savings
    BankAccountType.PAYMENT -> R.string.bank_type_payment
    BankAccountType.OTHER -> R.string.bank_type_other
}

/** The header's one action for a bank account: its transfer details, as Rust writes them, to the clipboard. */
@Composable
internal fun CopyDetails(actions: FieldActions) {
    HavenButton(
        stringResource(R.string.bank_copy_details),
        onClick = actions::copyDetails,
        modifier = Modifier.padding(bottom = 16.dp),
        style = ButtonStyle.Secondary,
        icon = HavenIcon.Copy,
    )
}

/**
 * A bank account's fields in the desktop's order: bank, agência, conta,
 * type, holder, password, websites, notes, then its custom fields. The
 * account, its digit, the password and the notes are read from Rust only
 * on reveal or copy; the account's dots show its last characters.
 */
@Composable
internal fun BankFields(bank: BankViewRecord, actions: FieldActions) {
    val bankLabel = stringResource(R.string.field_bank_bank)
    val branchLabel = stringResource(R.string.field_bank_branch)
    val typeLabel = stringResource(R.string.field_bank_type)
    val holderLabel = stringResource(R.string.field_bank_holder)
    val websiteLabel = stringResource(R.string.field_website)
    val last3 = bank.summary.bank?.accountLast3
    // What TalkBack hears for the dots: the ending they show, else only that it is hidden.
    val ending = last3?.let { stringResource(R.string.bank_account_ending, it) }
        ?: stringResource(R.string.hidden, stringResource(R.string.field_bank_account))
    InsetGroup {
        bank.bank?.let { v -> row { ShownRow(bankLabel, v, onCopy = { actions.copyShown(bankLabel, v) }) } }
        bank.branch?.let { b ->
            val v = joinDigit(b, bank.branchDigit)
            row { ShownRow(branchLabel, v, onCopy = { actions.copyShown(branchLabel, v) }) }
        }
        if (bank.hasAccount) {
            row("account") {
                HiddenBankRow(R.string.field_bank_account, actions, mask = "•••${last3.orEmpty()}", ending) {
                    bankAccount()
                }
            }
        }
        bank.accountType?.let { t ->
            row { GroupRow { GroupRowField(typeLabel, stringResource(accountTypeLabel(t))) } }
        }
        bank.holderName?.let { v -> row { ShownRow(holderLabel, v, onCopy = { actions.copyShown(holderLabel, v) }) } }
        if (bank.hasPassword) {
            row("password") { HiddenBankRow(R.string.field_password, actions) { revealBank(BankField.PASSWORD) } }
        }
        bank.websites.forEach { site ->
            row { ShownRow(websiteLabel, site.url, onCopy = { actions.copyShown(websiteLabel, site.url) }) }
        }
        if (bank.hasNotes) row("notes") { HiddenBankRow(R.string.field_notes, actions) { revealBank(BankField.NOTES) } }
    }
    ItemFields(bank.customFields, actions, afterGroup = true)
}

/** A bank value behind the eye; [fetch] asks Rust for it on each reveal or copy. */
@Composable
private fun HiddenBankRow(
    @StringRes label: Int,
    actions: FieldActions,
    mask: String? = null,
    maskDescription: String? = null,
    fetch: suspend ItemViewModel.() -> Outcome<String>,
) {
    val name = stringResource(label)
    val reveal = rememberRevealState()
    SecretRow(
        name,
        reveal.value,
        onReveal = { actions.revealWith(reveal) { actions.viewModel.fetch() } },
        onCopy = { actions.copyWith(name) { actions.viewModel.fetch() } },
        mask = mask,
        maskDescription = maskDescription,
    )
}
