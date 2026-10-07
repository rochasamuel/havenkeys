package net.havenkeys.android.screens

import android.graphics.Bitmap
import android.graphics.Canvas
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.dp
import java.io.File
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import net.havenkeys.android.R
import net.havenkeys.android.autofill.AutofillSearchScreen
import net.havenkeys.android.autofill.CallerApp
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.credentials.PasskeyCreateActions
import net.havenkeys.android.credentials.PasskeyCreateContent
import net.havenkeys.android.credentials.PasskeyCreateUiState
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeAutofillRepository
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.autofillsetup.AutofillSetupScreen
import net.havenkeys.android.ui.edit.EditNavigation
import net.havenkeys.android.ui.edit.EditScreen
import net.havenkeys.android.ui.edit.EditTarget
import net.havenkeys.android.ui.edit.EditViewModel
import net.havenkeys.android.ui.generator.GeneratorScreen
import net.havenkeys.android.ui.generator.GeneratorViewModel
import net.havenkeys.android.ui.health.HealthNavigation
import net.havenkeys.android.ui.health.HealthScreen
import net.havenkeys.android.ui.health.HealthViewModel
import net.havenkeys.android.ui.item.ItemNavigation
import net.havenkeys.android.ui.item.ItemScreen
import net.havenkeys.android.ui.item.ItemViewModel
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.DialogSurface
import net.havenkeys.android.ui.kit.SheetSurface
import net.havenkeys.android.ui.onboarding.OnboardingScreen
import net.havenkeys.android.ui.onboarding.OnboardingUiState
import net.havenkeys.android.ui.onboarding.OnboardingViewModel
import net.havenkeys.android.ui.settings.DevicesScreen
import net.havenkeys.android.ui.settings.DevicesViewModel
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.unlock.UnlockForm
import net.havenkeys.android.ui.unlock.UnlockUiState
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowLooper
import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.DeviceInfo
import uniffi.havenkeys_mobile.EditField
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.Generated
import uniffi.havenkeys_mobile.HealthCountsView
import uniffi.havenkeys_mobile.HealthIssueView
import uniffi.havenkeys_mobile.HealthKind
import uniffi.havenkeys_mobile.HealthView
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.MatchKind
import uniffi.havenkeys_mobile.TotpNow
import uniffi.havenkeys_mobile.ViewField
import uniffi.havenkeys_mobile.Website

private const val SETTLE_MS = 2_000L
private const val TYPING_MS = 500L

/**
 * Renders the stage 4 screens with fake data for design review
 * (apps/android/.impeccable/review). Runs only when the build passes a directory:
 * `./gradlew testGithubDebugUnitTest --tests '*ScreenScreenshots*' -PscreensDir=.impeccable/review`.
 * Sheets and dialogs are drawn inline (SheetSurface, DialogSurface): their own windows do not reach the decor view.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class ScreenScreenshots {
    /** One rendered screen: its name, its content, and what the test does on it before the shot. */
    private class Shot(val name: String, val before: () -> Unit = {}, val content: @Composable () -> Unit)

    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    private val dir: File? = System.getProperty("havenkeys.screens.dir")?.takeIf { it.isNotBlank() }?.let(::File)

    private val app = RuntimeEnvironment.getApplication()
    private val github = ItemSummary(
        "1", ItemKind.LOGIN, "GitHub", "sam@example.com", "github.com", true, true, 0, 0,
        tags = emptyList(),
    )
    private val vault = FakeVaultRepository().apply {
        view = Outcome.Ok(
            ItemView(
                github,
                listOf(
                    ViewField("username", "username", FieldKind.TEXT, "sam@example.com"),
                    ViewField("password", "password", FieldKind.SECRET, null),
                    ViewField("totp", "totp", FieldKind.TOTP, null),
                    ViewField("website", "website", FieldKind.URL, "https://github.com"),
                ),
            ),
        )
        revealed = Outcome.Ok("vR7#kq2-Lm9!xT4w")
        totpNow = Outcome.Ok(TotpNow("381492", 30u, 18u))
        edit = Outcome.Ok(
            ItemEdit(
                ItemKind.LOGIN,
                "GitHub",
                listOf(Website("github.com", MatchKind.DOMAIN)),
                listOf(
                    EditField("username", FieldKind.TEXT, true, "sam@example.com"),
                    EditField("password", FieldKind.SECRET, true, null),
                    EditField("totp", FieldKind.TOTP, true, null),
                    EditField("notes", FieldKind.SECRET, false, null),
                ),
                false,
                true,
                3L,
                tags = emptyList(),
            ),
        )
        generated = Outcome.Ok(Generated("vR7#kq2-Lm9!xT4w", 104.0))
        items = Outcome.Ok(
            listOf(
                github,
                ItemSummary(
                    "2", ItemKind.LOGIN, "GitLab", "sam@example.com", "gitlab.com", false, false, 0, 0,
                    tags = emptyList(),
                ),
                ItemSummary(
                    "3", ItemKind.LOGIN, "Example Bank", "sam.rocha", "bank.example", false, false, 0, 0,
                    tags = emptyList(),
                ),
                ItemSummary(
                    "4", ItemKind.LOGIN, "Old forum", null, "forum.example", false, false, 0, 0,
                    tags = emptyList(),
                ),
            ),
        )
        healthView = Outcome.Ok(
            HealthView(
                HealthCountsView(1u, 2u, 1u, 1u, 1u, 1u, 0u),
                listOf(
                    HealthIssueView("1", listOf(HealthKind.WEAK, HealthKind.REUSED), 0u, null, false),
                    HealthIssueView("2", listOf(HealthKind.REUSED, HealthKind.TWO_FACTOR), 0u, null, false),
                    HealthIssueView("3", listOf(HealthKind.PASSKEY), null, null, false),
                    HealthIssueView("4", listOf(HealthKind.INSECURE, HealthKind.OLD), null, null, false),
                ),
            ),
        )
    }

    /** The same login with tags, in a vault whose other tags the editor suggests. */
    private val tagged = FakeVaultRepository().apply {
        val summary = github.copy(tags = listOf("staging", "work"))
        view = vault.view.let { (it as Outcome.Ok).copy(value = it.value.copy(summary = summary)) }
        totpNow = vault.totpNow
        edit = vault.edit.let { (it as Outcome.Ok).copy(value = it.value.copy(tags = listOf("staging", "work"))) }
        items = Outcome.Ok(
            listOf(
                summary,
                ItemSummary(
                    "2", ItemKind.LOGIN, "Stripe", "sam@example.com", "stripe.com", false, false, 0, 0,
                    tags = listOf("stripe", "work"),
                ),
            ),
        )
    }

    private val typeATag = {
        val field = hasSetTextAction() and hasText(app.getString(R.string.edit_add_tag))
        rule.onNode(field).performScrollTo().performTextInput("st")
        rule.waitForIdle()
        rule.onNode(hasText("stripe") and hasClickAction()).performScrollTo()
        rule.waitForIdle()
    }

    private fun editing(kind: ItemKind, title: String, fields: List<EditField>) = FakeVaultRepository().apply {
        edit = Outcome.Ok(ItemEdit(kind, title, emptyList(), fields, false, true, 3L, tags = emptyList()))
    }

    private val note = editing(
        ItemKind.SECURE_NOTE,
        "Wi-Fi at home",
        listOf(EditField("content", FieldKind.SECRET, true, null)),
    )
    private val card = editing(
        ItemKind.CARD,
        "Visa",
        listOf(
            EditField("card.holder", FieldKind.TEXT, true, "Sam Rocha"),
            EditField("card.number", FieldKind.SECRET, true, null),
            EditField("card.code", FieldKind.SECRET, true, null),
            EditField("card.expiry", FieldKind.TEXT, true, "08/29"),
            EditField("card.notes", FieldKind.SECRET, false, null),
        ),
    )
    private val identity = editing(
        ItemKind.IDENTITY,
        "Sam Rocha",
        listOf("first_name" to "Sam", "last_name" to "Rocha", "email" to "sam@example.com", "city" to "Recife")
            .map { (k, v) -> EditField("identity.$k", FieldKind.TEXT, true, v) } +
            listOf("cpf", "passport").map { EditField("identity.$it", FieldKind.SECRET, it == "cpf", null) },
    )
    private val accounts = FakeAccountRepository().apply {
        deviceList = Outcome.Ok(
            listOf(
                DeviceInfo("d1", "Pixel 8", "2026-01-01T00:00:00Z", null, true, null),
                DeviceInfo("d2", "Work laptop", "2026-01-01T00:00:00Z", "2026-10-03T10:00:00Z", false, "d1"),
            ),
        )
    }
    private val clipboard = SensitiveClipboard(app, CoroutineScope(SupervisorJob()))
    private val events = VaultEventsHub()
    private val passkey = PasskeyCreateUiState(
        loading = false,
        rpId = "github.com",
        userName = "sam",
        homes = listOf(
            AutofillMatch("h1", "GitHub", "sam@example.com", true),
            AutofillMatch("h2", "GitHub work", "sam@work.example", false),
        ),
        selected = "h1",
    )

    @Composable
    private fun Item(repo: FakeVaultRepository = vault) {
        ItemScreen(
            remember { ItemViewModel(repo, FakeSettingsRepository(), events, "1") },
            clipboard,
            true,
            ItemNavigation({}, {}, {}, {}),
        )
    }

    @Composable
    private fun Health() {
        HealthScreen(
            remember { HealthViewModel(vault, events) },
            true,
            HealthNavigation({}, {}, {}, {}),
        )
    }

    @Composable
    private fun Editor(repo: FakeVaultRepository, isNew: Boolean = false) {
        EditScreen(
            remember { EditViewModel(repo, accounts, events, EditTarget.Existing("1")) },
            isNew = isNew,
            online = true,
            navigation = EditNavigation({}, {}, {}),
        )
    }

    @Composable
    private fun PasskeySheet() {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
            SheetSurface(
                stringResource(R.string.passkey_save_title),
                footer = { PasskeyCreateActions(passkey, {}, {}) },
            ) { PasskeyCreateContent(passkey, {}, {}, {}) }
        }
    }

    private val revealPassword = {
        val label = app.getString(R.string.field_password)
        rule.onNodeWithContentDescription(app.getString(R.string.reveal, label)).performClick()
        rule.waitForIdle()
    }

    private val shots: List<Shot> = listOf(
        Shot("item") { Item() },
        Shot("item-revealed", before = revealPassword) { Item() },
        Shot("editor") { Editor(vault) },
        Shot("editor-tags", before = typeATag) { Editor(tagged) },
        Shot("item-tags") { Item(tagged) },
        Shot("editor-note") { Editor(note) },
        Shot("editor-card") { Editor(card) },
        Shot("editor-identity") { Editor(identity) },
        Shot("generator") {
            GeneratorScreen(remember { GeneratorViewModel(vault, FakeSettingsRepository()) }, clipboard, false, {}, {})
        },
        Shot("unlock") { UnlockForm(UnlockUiState(needsSecretKey = true, offerBiometric = true), { _, _ -> }, {}) },
        Shot("onboarding") { OnboardingScreen(remember { OnboardingViewModel(accounts) }, onDone = {}) },
        Shot("onboarding-type") {
            OnboardingScreen(
                remember { OnboardingViewModel(accounts).also { it.choose(OnboardingUiState.Mode.TYPE) } },
                onDone = {},
            )
        },
        Shot("onboarding-password") {
            OnboardingScreen(
                remember { OnboardingViewModel(accounts).also { it.choose(OnboardingUiState.Mode.PASSWORD) } },
                onDone = {},
            )
        },
        Shot("health") { Health() },
        Shot("health-filtered", before = {
            rule.onNode(hasText(app.getString(R.string.health_reused_title)) and hasClickAction()).performClick()
            rule.waitForIdle()
        }) { Health() },
        Shot("health-list", before = {
            rule.onNode(hasScrollAction()).performScrollToNode(hasText("Old forum"))
            rule.waitForIdle()
        }) { Health() },
        Shot("devices") { DevicesScreen(remember { DevicesViewModel(accounts, events) }, true, {}, {}) },
        Shot("autofill-setup") { AutofillSetupScreen(online = false, onBack = {}, onLock = {}) },
        Shot(
            "autofill-search",
            before = {
                rule.onNode(hasSetTextAction()).performTextInput("git")
                rule.waitForIdle()
                ShadowLooper.idleMainLooper(SETTLE_MS, TimeUnit.MILLISECONDS)
                rule.mainClock.advanceTimeBy(TYPING_MS)
            },
        ) {
            val repo = remember {
                FakeAutofillRepository().apply {
                    matchList = Outcome.Ok(listOf(AutofillMatch("m1", "GitHub", "sam@example.com", true)))
                }
            }
            AutofillSearchScreen(repo::search, CallerApp("com.example.app", "Example"), onConfirmed = { null })
        },
        Shot("wallet-confirm") {
            Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) {
                DialogSurface(
                    title = "Fill your identity in shop.example?",
                    confirm = DialogAction("Fill with documents", {}),
                    message = "The app calls itself Shop",
                    dismiss = DialogAction("Cancel", {}),
                    alternative = DialogAction("Fill without documents", {}),
                )
            }
        },
        Shot("passkey-sheet") { PasskeySheet() },
    )

    private fun save(name: String) {
        rule.waitForIdle()
        val view = rule.activity.window.decorView
        val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
        view.draw(Canvas(bitmap))
        val out = File(dir, "$name.png")
        out.parentFile?.mkdirs()
        out.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    private fun render(prefix: String, list: List<Shot>) {
        assumeTrue(dir != null)
        var dark by mutableStateOf(false)
        var index by mutableIntStateOf(0)
        rule.setContent {
            HavenTheme(darkTheme = dark) {
                Box(Modifier.fillMaxSize().background(HavenTheme.colors.pane)) {
                    key(index, dark) { list[index].content() }
                }
            }
        }
        for (theme in listOf(false, true)) {
            list.forEachIndexed { i, shot ->
                dark = theme
                index = i
                rule.mainClock.advanceTimeBy(SETTLE_MS)
                shot.before()
                rule.mainClock.advanceTimeBy(SETTLE_MS)
                save("$prefix-${if (theme) "dark" else "light"}-${shot.name}")
            }
        }
    }

    @Test
    fun everyRebuiltScreenInBothThemes() = render("screens", shots)

    /** A small phone: the editor's density and whether the passkey sheet's answers stay in view. */
    @Test
    @Config(qualifiers = "w360dp-h640dp-xxhdpi")
    fun onASmallPhone() = render(
        "screens-360",
        listOf(
            Shot("editor") { Editor(vault) },
            Shot("editor-tags", before = typeATag) { Editor(tagged) },
            Shot("passkey-sheet") { PasskeySheet() },
            Shot("health") { Health() },
            Shot("item") { Item() },
        ),
    )
}
