package net.havenkeys.android.ui.edit

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.allCaps
import androidx.compose.foundation.text.input.maxLength
import androidx.compose.foundation.text.input.then
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.intl.Locale
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.item.accountTypeLabel
import net.havenkeys.android.ui.kit.ChoiceSheet
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowField
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.BankAccountType
import uniffi.havenkeys_mobile.ItemEdit

/** The digit box: one or two characters, wide enough for its label. */
private val DigitBox = 112.dp

/** "Not set", then the four types, as the desktop lists them. */
private val TYPES: List<BankAccountType?> = listOf(null) + BankAccountType.entries

/** A check digit's `x` as `X`, the same in every language. */
private val DigitCaps = InputTransformation.allCaps(Locale("en"))

/** The neutral example an empty website shows (a bank's own would look like a suggestion). */
private const val SITE_PLACEHOLDER = "example.com"

/**
 * A bank account's editor, after its title: bank, holder, type, agência and
 * conta each beside its digit, the password (as a login's: hidden until
 * Change, with the generator), then its websites and notes. The saved
 * account is never loaded: Change opens empty boxes, and left empty they
 * keep it. Pasting `12345-6` into a number box fills its digit box too.
 */
@Composable
internal fun BankEditFields(bank: BankBoxes, editor: EditorState, edit: ItemEdit, values: FieldValues) {
    val field = { key: String -> edit.fields.first { it.key == key } }
    InsetGroup {
        row { BankText(bank.bank, R.string.field_bank_bank, bank) { bank.bank = it } }
        row { BankText(bank.holder, R.string.field_bank_holder, bank) { bank.holder = it } }
        row { AccountTypeRow(bank) }
        row("branch") {
            NumberPair(
                bank,
                number = bank.branch to bank::typeBranch,
                digit = bank.branchDigit to { bank.branchDigit = it },
                labels = R.string.field_bank_branch to R.string.bank_branch_digit,
                maxDigit = MAX_BRANCH_DIGIT,
            )
        }
        row("account") { AccountEditor(bank) }
        row("password") { FieldEditor(field("password"), editor, values) }
    }
    Websites(editor, placeholder = SITE_PLACEHOLDER)
    InsetGroup { row { FieldEditor(field("notes"), editor, values) } }
}

/** A plain bank value, loaded with the account and sent in full. */
@Composable
private fun BankText(value: String, label: Int, bank: BankBoxes, onEdit: (String) -> Unit) {
    val text = rememberDraftText(bank, label, initial = { value }, onEdit = onEdit)
    HavenTextField(text, stringResource(label), keyboardOptions = KeyboardOptions(autoCorrectEnabled = false))
}

/** The account type, picked from a sheet as a website's match rule is. */
@Composable
private fun AccountTypeRow(bank: BankBoxes) {
    var choosing by remember { mutableStateOf(false) }
    val label = stringResource(R.string.field_bank_type)
    GroupRow(onClick = { choosing = true }) { GroupRowField(label, typeName(bank.accountType)) }
    if (choosing) {
        ChoiceSheet(
            title = label,
            values = TYPES,
            selected = bank.accountType,
            label = { typeName(it) },
            onSelect = { bank.accountType = it },
            onDismiss = { choosing = false },
        )
    }
}

@Composable
private fun typeName(type: BankAccountType?): String =
    stringResource(type?.let(::accountTypeLabel) ?: R.string.bank_type_none)

/** The saved account behind Change and Remove; once opened (or with none saved), its two boxes. */
@Composable
private fun AccountEditor(bank: BankBoxes) {
    val label = stringResource(R.string.field_bank_account)
    when {
        bank.accountRemoved -> RemovedRow(label, onUndo = { bank.accountRemoved = false })
        bank.hasAccount && !bank.accountOpen -> {
            val last3 = bank.accountLast3
            HiddenRow(
                label,
                masked = true,
                onChange = { bank.accountOpen = true },
                onRemove = { bank.accountRemoved = true },
                mask = "•••${last3.orEmpty()}",
                maskDescription = last3?.let { stringResource(R.string.bank_account_ending, it) }
                    ?: stringResource(R.string.hidden, label),
            )
        }
        else -> NumberPair(
            bank,
            number = bank.account to bank::typeAccount,
            digit = bank.accountDigit to { bank.accountDigit = it },
            labels = R.string.field_bank_account to R.string.bank_account_digit,
            maxDigit = MAX_ACCOUNT_DIGIT,
            // Typed, the boxes replace the saved account; left empty, they keep it.
            hint = if (bank.hasAccount && bank.account.isEmpty() && bank.accountDigit.isEmpty()) {
                stringResource(R.string.bank_keep_account)
            } else {
                null
            },
        )
    }
}

/**
 * A number box beside its check digit's. Both start again from the draft
 * after a split, so a pasted `12345-6` shows as `12345` and `6`.
 */
@Suppress("LongParameterList") // the two boxes' values, edits and labels, and the digit's length
@Composable
private fun NumberPair(
    bank: BankBoxes,
    number: Pair<String, (String) -> Unit>,
    digit: Pair<String, (String) -> Unit>,
    labels: Pair<Int, Int>,
    maxDigit: Int,
    hint: String? = null,
) {
    val numberText =
        rememberDraftText(bank, labels.first, bank.splits, initial = { number.first }, onEdit = number.second)
    val digitText =
        rememberDraftText(bank, labels.second, bank.splits, initial = { digit.first }, onEdit = digit.second)
    Row(Modifier.height(IntrinsicSize.Min)) {
        HavenTextField(
            numberText,
            stringResource(labels.first),
            Modifier.weight(1f),
            placeholder = stringResource(R.string.bank_number),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, autoCorrectEnabled = false),
            hint = hint,
        )
        // A hairline marks the digit box as a field of its own, as the group's rows are marked.
        Box(Modifier.fillMaxHeight().padding(vertical = 12.dp).width(1.dp).background(HavenTheme.colors.groupLine))
        HavenTextField(
            digitText,
            stringResource(R.string.bank_digit),
            Modifier.width(DigitBox),
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Ascii,
                capitalization = KeyboardCapitalization.Characters,
                autoCorrectEnabled = false,
            ),
            inputTransformation = InputTransformation.maxLength(maxDigit).then(DigitCaps),
            spokenLabel = stringResource(labels.second),
        )
    }
}
