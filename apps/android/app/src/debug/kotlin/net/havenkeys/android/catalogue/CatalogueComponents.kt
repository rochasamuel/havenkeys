package net.havenkeys.android.catalogue

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import kotlinx.coroutines.delay
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.ChoiceRow
import net.havenkeys.android.ui.kit.ChoiceSheet
import net.havenkeys.android.ui.kit.CopyButton
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.DialogSurface
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowField
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenMenu
import net.havenkeys.android.ui.kit.HavenSheet
import net.havenkeys.android.ui.kit.HavenSlider
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.ItemRow
import net.havenkeys.android.ui.kit.ItemTile
import net.havenkeys.android.ui.kit.MenuItem
import net.havenkeys.android.ui.kit.MenuSurface
import net.havenkeys.android.ui.kit.Pill
import net.havenkeys.android.ui.kit.PillTone
import net.havenkeys.android.ui.kit.ProgressRing
import net.havenkeys.android.ui.kit.PullToRefresh
import net.havenkeys.android.ui.kit.RowLeading
import net.havenkeys.android.ui.kit.SecretTextField
import net.havenkeys.android.ui.kit.SectionAction
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.kit.SegmentedControl
import net.havenkeys.android.ui.kit.SheetSurface
import net.havenkeys.android.ui.kit.ToggleRow
import net.havenkeys.android.ui.kit.TrailingText
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

private const val COPIED_TOAST = "Password copied · clears in 30 s"

@Composable
internal fun ActionsSection() {
    val toasts = LocalCatalogueToasts.current
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        HavenButton("Unlock", onClick = {}, modifier = Modifier.fillMaxWidth())
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            HavenButton("Cancel", onClick = {}, style = ButtonStyle.Secondary)
            HavenButton("Skip", onClick = {}, style = ButtonStyle.Quiet)
            HavenButton("Remove", onClick = {}, style = ButtonStyle.Danger)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            HavenButton("Generate", onClick = {}, style = ButtonStyle.Secondary, icon = HavenIcon.Dice)
            HavenButton("Disabled", onClick = {}, enabled = false)
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            HavenIconButton(HavenIcon.Lock, "Lock", onClick = {})
            HavenIconButton(HavenIcon.More, "More", onClick = {})
            CopyButton("Password", onCopy = { toasts.show(COPIED_TOAST) })
            Pill("This device")
            Pill("Whole site", Modifier.padding(start = 8.dp), tone = PillTone.Outline)
        }
    }
}

@Composable
internal fun StructureSection() {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        HavenText(
            "HavenScaffold frames this screen: the bar above, the add button and the toasts below.",
            style = HavenTheme.type.body,
            color = HavenTheme.colors.muted,
        )
        SectionHeader("Recently added", action = SectionAction("Clear") {})
        InsetGroup {
            row {
                ItemRow(
                    "GitHub", "sam@example.com", RowLeading.Monogram("GitHub"),
                    onClick = {}, hasPasskey = true, hasCode = true,
                )
            }
            row { ItemRow("Wi-Fi at home", "Secure note", RowLeading.Glyph(HavenIcon.Note, soft = true), onClick = {}) }
            row { ItemRow("Visa ending 4242", "Card", RowLeading.Glyph(HavenIcon.Card), onClick = {}) }
            row {
                ItemRow(
                    "😀 A title that is long enough to be cut at the end of the row", null,
                    RowLeading.Monogram("😀"), onClick = {},
                )
            }
        }
        SectionHeader("Settings")
        InsetGroup {
            row { GroupRow(onClick = {}, icon = HavenIcon.Clock) { GroupRowText("Auto-lock", "After 5 minutes") } }
            row { GroupRow(onClick = {}, trailing = { TrailingText("12") }) { GroupRowText("Logins") } }
            row {
                GroupRow(trailing = { CopyButton("Username", onCopy = {}) }) {
                    GroupRowField("Username", "sam@example.com")
                }
            }
        }
    }
}

@Composable
internal fun InputsSection() {
    val username = rememberTextFieldState("sam@example.com")
    val server = rememberTextFieldState("http://vault.local")
    val secret = rememberTextFieldState("correct horse battery")
    var revealed by remember { mutableStateOf(false) }
    var biometrics by remember { mutableStateOf(true) }
    var screenOff by remember { mutableStateOf(false) }
    var length by remember { mutableFloatStateOf(24f) }
    var mode by remember { mutableIntStateOf(0) }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        InsetGroup {
            row { HavenTextField(username, "Username") }
            row { HavenTextField(server, "Server", error = "The address must start with https://") }
            row { SecretTextField(secret, "Password", revealed, { revealed = it }) }
        }
        InsetGroup {
            row { ToggleRow("Unlock with biometrics", biometrics, { biometrics = it }) }
            row { ToggleRow("Lock when the screen turns off", screenOff, { screenOff = it }, detail = "Recommended") }
        }
        HavenSlider(
            length, { length = it }, 8f..64f,
            label = "Length", steps = 55, valueText = "${length.roundToInt()}",
        )
        SegmentedControl(listOf("Random", "Words", "PIN"), mode, { mode = it })
    }
}

private fun sampleMenu() = listOf(
    MenuItem("Edit", {}, HavenIcon.Edit),
    MenuItem("Copy username", {}, HavenIcon.Copy),
    MenuItem("Delete", {}, HavenIcon.Trash, danger = true),
)

@Composable
private fun SampleTiles() {
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        listOf(
            "Login" to HavenIcon.Key, "Secure note" to HavenIcon.Note,
            "Card" to HavenIcon.Card, "Generate" to HavenIcon.Dice,
        )
            .forEach { (label, icon) ->
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    ItemTile(RowLeading.Glyph(icon, soft = true), size = 52.dp)
                    HavenText(label, style = HavenTheme.type.label, color = HavenTheme.colors.text)
                }
            }
    }
}

@Composable
internal fun OverlaysSection() {
    val toasts = LocalCatalogueToasts.current
    var sheet by remember { mutableStateOf(false) }
    var dialog by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var choice by remember { mutableIntStateOf(5) }
    var choosing by remember { mutableStateOf(false) }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        HavenText(
            "Drawn inline below; the buttons open the real ones, in secure windows that screenshots show black.",
            style = HavenTheme.type.body,
            color = HavenTheme.colors.muted,
        )
        SheetSurface("New item") { SampleTiles() }
        DialogSurface(
            title = "Remove this device?",
            confirm = DialogAction("Remove", {}, danger = true),
            message = "Its local copy of the vault is erased.",
            dismiss = DialogAction("Cancel", {}),
        )
        MenuSurface(sampleMenu(), onDismiss = {})
        ChoiceAndThirdAnswer(choice) { choice = it }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            HavenButton("Sheet", onClick = { sheet = true }, style = ButtonStyle.Secondary)
            HavenButton("Dialog", onClick = { dialog = true }, style = ButtonStyle.Secondary)
            HavenButton("Choice", onClick = { choosing = true }, style = ButtonStyle.Secondary)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Box {
                HavenButton("Menu", onClick = { menu = true }, style = ButtonStyle.Secondary)
                HavenMenu(menu, onDismiss = { menu = false }, items = sampleMenu())
            }
            HavenButton("Toast", onClick = { toasts.show(COPIED_TOAST) }, style = ButtonStyle.Secondary)
        }
    }
    if (sheet) HavenSheet(onDismiss = { sheet = false }, title = "New item") { SampleTiles() }
    if (dialog) {
        HavenDialog(
            title = "Remove this device?",
            onDismiss = { dialog = false },
            confirm = DialogAction("Remove", { dialog = false }, danger = true),
            message = "Its local copy of the vault is erased.",
            dismiss = DialogAction("Cancel", { dialog = false }),
        )
    }
    if (choosing) {
        ChoiceSheet(
            "Lock automatically",
            listOf(5, 15, 30),
            choice,
            label = { "After $it minutes" },
            onSelect = { choice = it },
            onDismiss = { choosing = false },
        )
    }
}

@Composable
private fun ChoiceAndThirdAnswer(choice: Int, onChoice: (Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        InsetGroup(Modifier.selectableGroup()) {
            row { ChoiceRow("After 5 minutes", selected = choice == 5, onClick = { onChoice(5) }) }
            row {
                ChoiceRow(
                    "After 15 minutes",
                    selected = choice == 15,
                    onClick = { onChoice(15) },
                    detail = "Recommended",
                )
            }
        }
        DialogSurface(
            title = "Fill your identity?",
            confirm = DialogAction("Fill with documents", {}),
            dismiss = DialogAction("Cancel", {}),
            alternative = DialogAction("Fill without documents", {}),
        )
    }
}

@Composable
internal fun FeedbackSection() {
    var refreshing by remember { mutableStateOf(false) }
    LaunchedEffect(refreshing) {
        if (refreshing) {
            delay(1_500)
            refreshing = false
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
            ProgressRing(0.7f, contentDescription = "21 seconds left")
            ProgressRing(0.12f, warn = true, contentDescription = "4 seconds left")
            ProgressRing(null, contentDescription = "Loading")
        }
        HavenText("Pull the list below down to refresh.", style = HavenTheme.type.body, color = HavenTheme.colors.muted)
        PullToRefresh(
            refreshing = refreshing,
            onRefresh = { refreshing = true },
            modifier = Modifier.fillMaxWidth().height(220.dp).clip(HavenShape.group)
                .background(HavenTheme.colors.group),
        ) {
            LazyColumn(Modifier.fillMaxSize()) {
                items(12) { HavenText("Row ${it + 1}", Modifier.padding(16.dp)) }
            }
        }
    }
}
