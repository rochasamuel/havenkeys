package net.havenkeys.android.catalogue

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsPropertyKey
import androidx.compose.ui.semantics.SemanticsPropertyReceiver
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.kit.AddButton
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.SegmentedControl
import net.havenkeys.android.ui.kit.ToastState
import net.havenkeys.android.ui.kit.rememberToastState
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

internal const val CATALOGUE_LIST = "catalogue"

/** One group of the catalogue; [components] are the ui/kit file names it shows (CatalogueTest checks them all). */
internal class CatalogueSection(val title: String, val components: List<String>, val content: @Composable () -> Unit)

/** Lets a test read which theme the catalogue is drawn in (CatalogueTest checks that picking a theme changes it). */
internal val CatalogueDark = SemanticsPropertyKey<Boolean>("CatalogueDark")
private var SemanticsPropertyReceiver.catalogueDark by CatalogueDark

internal val LocalCatalogueToasts = staticCompositionLocalOf<ToastState> { error("no toast state") }

internal fun catalogueSections(): List<CatalogueSection> = listOf(
    CatalogueSection("Foundations", listOf("HavenIcon")) { Foundations() },
    CatalogueSection(
        "Actions",
        listOf("HavenButton", "HavenIconButton", "CopyButton", "Pill", "AddButton"),
    ) { ActionsSection() },
    CatalogueSection(
        "Structure",
        listOf("HavenScaffold", "SectionHeader", "InsetGroup", "GroupRow", "ItemRow"),
    ) { StructureSection() },
    CatalogueSection(
        "Inputs",
        listOf("HavenTextField", "SecretTextField", "HavenSwitch", "HavenSlider", "SegmentedControl"),
    ) { InputsSection() },
    CatalogueSection(
        "Overlays",
        listOf("HavenSheet", "HavenDialog", "Toast", "HavenMenu", "ChoiceSheet"),
    ) { OverlaysSection() },
    CatalogueSection("Feedback", listOf("ProgressRing", "PullToRefresh")) { FeedbackSection() },
)

private val ThemeChoices = listOf("System", "Light", "Dark")

/** Every kit component, in a theme picked at the top. HavenScaffold frames it; the add button and toasts are live. */
@Composable
internal fun KitCatalogue() {
    var choice by rememberSaveable { mutableIntStateOf(0) }
    val dark = when (choice) {
        1 -> false
        2 -> true
        else -> isSystemInDarkTheme()
    }
    HavenTheme(darkTheme = dark) {
        val toasts = rememberToastState()
        CompositionLocalProvider(LocalCatalogueToasts provides toasts) {
            HavenScaffold(
                topBar = {
                    Column(Modifier.padding(horizontal = HavenSpacing.gutter, vertical = 8.dp)) {
                        HavenText("HavenKeys kit", style = HavenTheme.type.title, color = HavenTheme.colors.textStrong)
                        SegmentedControl(ThemeChoices, choice, { choice = it })
                    }
                },
                floatingButton = { AddButton(onClick = { toasts.show("Add pressed") }) },
                toastState = toasts,
            ) { padding ->
                LazyColumn(
                    Modifier.fillMaxSize().testTag(CATALOGUE_LIST).semantics { catalogueDark = dark },
                    contentPadding = PaddingValues(
                        start = HavenSpacing.gutter,
                        end = HavenSpacing.gutter,
                        bottom = padding.calculateBottomPadding() + 24.dp,
                    ),
                ) {
                    catalogueSections().forEach { section ->
                        item(key = section.title) {
                            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                                HavenText(
                                    section.title,
                                    Modifier.padding(top = 28.dp).semantics { heading() },
                                    style = HavenTheme.type.headline,
                                    color = HavenTheme.colors.textStrong,
                                )
                                section.content()
                            }
                        }
                    }
                }
            }
        }
    }
}
