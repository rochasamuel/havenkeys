package net.havenkeys.android.ui.edit

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.TextFieldBuffer
import androidx.compose.foundation.text.input.allCaps
import androidx.compose.foundation.text.input.maxLength
import androidx.compose.foundation.text.input.then
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.intl.Locale
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.item.accountTypeLabel
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.ChoiceSheet
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowField
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.BankAccountType
import uniffi.havenkeys_mobile.ItemEdit

/** The digit box: one or two characters, wide enough for its label. */
private val DigitBox = 112.dp

/** "Not set", then the four types, as the desktop lists them. */
private val TYPES: List<BankAccountType?> = listOf(null) + BankAccountType.entries

/** A check digit's `x` as `X`, the same in every language. */
private val DigitCaps = InputTransformation.allCaps(Locale("en"))

/** A muted example in the empty digit box, so its column reads as a field. */
private const val DIGIT_PLACEHOLDER = "0"

/** The neutral example an empty website shows (a bank's own would look like a suggestion). */
private const val SITE_PLACEHOLDER = "example.com"

/**
 * A bank account's editor, after its title, in the detail's order: bank,
 * agência and conta each beside its digit, type, holder, the password (as
 * a login's: hidden until Change, with the generator), then its websites
 * and notes. The saved account is never loaded: Change opens empty boxes,
 * and left empty they keep it. Pasting `12345-6` into a number box fills
 * its digit box too; typing never splits.
 */
@Composable
internal fun BankEditFields(bank: BankBoxes, editor: EditorState, edit: ItemEdit, values: FieldValues) {
    val field = { key: String -> edit.fields.first { it.key == key } }
    InsetGroup {
        row { BankText(bank.bank, R.string.field_bank_bank, bank) { bank.bank = it } }
        row("branch") {
            NumberPair(
                bank,
                number = NumberBox(bank.branch, MAX_BRANCH, bank::pasteBranch) { bank.branch = it },
                digit = bank.branchDigit to { bank.branchDigit = it },
                labels = R.string.field_bank_branch to R.string.bank_branch_digit,
                maxDigit = MAX_BRANCH_DIGIT,
            )
        }
        row("account") { AccountEditor(bank) }
        row("type") { AccountTypeRow(bank) }
        row("holder") { BankText(bank.holder, R.string.field_bank_holder, bank) { bank.holder = it } }
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
        else -> Column {
            NumberPair(
                bank,
                number = NumberBox(bank.account, MAX_ACCOUNT, bank::pasteAccount) { bank.account = it },
                digit = bank.accountDigit to { bank.accountDigit = it },
                labels = R.string.field_bank_account to R.string.bank_account_digit,
                maxDigit = MAX_ACCOUNT_DIGIT,
            )
            if (bank.hasAccount) KeepAccount(bank)
        }
    }
}

/**
 * Under the opened boxes of a saved account: while the number is empty,
 * that the saved one is kept; and Keep, back to it, as Undo is on a removed row.
 */
@Composable
private fun KeepAccount(bank: BankBoxes) {
    val keep = stringResource(R.string.bank_keep_description)
    Row(
        Modifier.fillMaxWidth().padding(start = HavenSpacing.rowX, end = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        HavenText(
            if (bank.account.isEmpty()) stringResource(R.string.bank_keep_account) else "",
            Modifier.weight(1f),
            style = HavenTheme.type.label,
            color = HavenTheme.colors.muted,
        )
        HavenButton(
            stringResource(R.string.bank_keep),
            onClick = bank::keepAccount,
            modifier = Modifier.semantics { contentDescription = keep },
            style = ButtonStyle.Quiet,
        )
    }
}

/** A number box: its draft value, its length, what a paste into it keeps ([paste]), and its edits. */
private class NumberBox(
    val value: String,
    val max: Int,
    val paste: (String) -> String?,
    val onEdit: (String) -> Unit,
) : InputTransformation {
    /**
     * One edit that inserts several characters is a paste: a `12345-6` that
     * splits keeps its number here (the digit goes to its box). Typing never
     * splits, and nothing grows past [max].
     */
    // `changes` is what tells a paste from typing; it is marked experimental.
    @OptIn(ExperimentalFoundationApi::class)
    override fun TextFieldBuffer.transformInput() {
        val pasted = (0 until changes.changeCount).any { changes.getRange(it).length > 1 }
        val number = if (pasted) paste(toString()) else null
        when {
            number != null -> replace(0, length, number)
            length > max -> revertAllChanges()
        }
    }
}

/**
 * A number box beside its check digit's. The digit box starts again from
 * the draft after a split, so a pasted `12345-6` shows as `12345` and `6`;
 * both start again on Keep.
 */
@Suppress("LongParameterList") // the two boxes, their labels, and the digit's length
@Composable
private fun NumberPair(
    bank: BankBoxes,
    number: NumberBox,
    digit: Pair<String, (String) -> Unit>,
    labels: Pair<Int, Int>,
    maxDigit: Int,
) {
    val numberText =
        rememberDraftText(bank, labels.first, bank.resets, initial = { number.value }, onEdit = number.onEdit)
    val digitText = rememberDraftText(
        bank,
        labels.second,
        bank.resets,
        bank.splits,
        initial = { digit.first },
        onEdit = digit.second,
    )
    Row(Modifier.height(IntrinsicSize.Min)) {
        HavenTextField(
            numberText,
            stringResource(labels.first),
            Modifier.weight(1f),
            placeholder = stringResource(R.string.bank_number),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, autoCorrectEnabled = false),
            inputTransformation = number,
        )
        // A hairline marks the digit box as a field of its own, as the group's rows are marked.
        Box(Modifier.fillMaxHeight().padding(vertical = 12.dp).width(1.dp).background(HavenTheme.colors.groupLine))
        HavenTextField(
            digitText,
            stringResource(R.string.bank_digit),
            Modifier.width(DigitBox),
            placeholder = DIGIT_PLACEHOLDER,
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
