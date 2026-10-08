package net.havenkeys.android.screens

import android.graphics.Bitmap
import android.graphics.Canvas
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import java.io.File
import java.util.concurrent.TimeUnit
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.home.HomeScreen
import net.havenkeys.android.ui.home.HomeViewModel
import net.havenkeys.android.ui.items.CategoryScreen
import net.havenkeys.android.ui.items.ItemListViewModel
import net.havenkeys.android.ui.items.ItemsScreen
import net.havenkeys.android.ui.items.TagScreen
import net.havenkeys.android.ui.kit.SheetSurface
import net.havenkeys.android.ui.search.SearchScreen
import net.havenkeys.android.ui.search.SearchViewModel
import net.havenkeys.android.ui.settings.SettingsActions
import net.havenkeys.android.ui.settings.SettingsNavigation
import net.havenkeys.android.ui.settings.SettingsScreen
import net.havenkeys.android.ui.settings.SettingsViewModel
import net.havenkeys.android.ui.shell.AddTiles
import net.havenkeys.android.ui.shell.ShellNavigation
import net.havenkeys.android.ui.shell.ShellScreen
import net.havenkeys.android.ui.shell.ShellScreens
import net.havenkeys.android.ui.shell.ShellViewModel
import net.havenkeys.android.ui.shell.tilesFor
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowLooper
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.ViewField

private const val SETTLE_MS = 2_000L

/** One rendered screen: its name, its content, and what the test does on it before the shot. */
private class Shot(val name: String, val before: () -> Unit = {}, val content: @Composable () -> Unit)

/**
 * Renders the stage 3 screens with fake data for design review
 * (apps/android/.impeccable/review). Runs only when the build passes a directory:
 * `./gradlew testGithubDebugUnitTest --tests '*ShellScreenshots*' -PscreensDir=.impeccable/review`.
 * The window is drawn directly (captureToImage waits for a frame Robolectric never commits).
 * Sheets open in their own window, so the add sheet's surface is drawn in place.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class ShellScreenshots {
    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    private val dir: File? = System.getProperty("havenkeys.screens.dir")?.takeIf { it.isNotBlank() }?.let(::File)

    private fun summary(
        id: String,
        kind: ItemKind,
        title: String,
        sub: String? = null,
        totp: Boolean = false,
        passkey: Boolean = false,
    ) = ItemSummary(id, kind, title, sub, null, totp, passkey, 0, 0, tags = emptyList())

    private val identity = summary("9", ItemKind.IDENTITY, "Sam Rocha")
    private val sample = listOf(
        summary("1", ItemKind.LOGIN, "GitHub", "sam@example.com", totp = true, passkey = true),
        summary("2", ItemKind.LOGIN, "Banco do Brasil", "sam.rocha"),
        summary("3", ItemKind.SECURE_NOTE, "Wi-Fi at home"),
        summary("4", ItemKind.CARD, "Visa", "•••• 4242"),
        summary("5", ItemKind.LOGIN, "Amazon", "sam@example.com"),
        identity,
    )
    private val events = VaultEventsHub()
    private val accounts = FakeAccountRepository()
    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(sample)
        recent = Outcome.Ok(sample.take(4))
        frequent = Outcome.Ok(listOf(sample[0], sample[4]))
        view = Outcome.Ok(
            ItemView(
                identity,
                listOf("first_name", "email", "city", "cpf").map {
                    ViewField("identity.$it", it, FieldKind.TEXT, null)
                },
            ),
        )
        searches += listOf("bank", "github")
    }

    /** The same vault with tags, for the Items tab's Tags group and a tag's list. */
    private val taggedVault = FakeVaultRepository().apply {
        val tags = mapOf("1" to listOf("Dev", "Work"), "2" to listOf("Finance"), "5" to listOf("Shopping", "Work"))
        items = Outcome.Ok(sample.map { it.copy(tags = tags[it.id].orEmpty()) })
        recent = vault.recent
        view = vault.view
    }

    /** A Home whose Recently added failed to load and whose Frequently used is empty. */
    private val failingVault = FakeVaultRepository().apply {
        items = Outcome.Ok(sample)
        recent = Outcome.Failed("network")
        view = vault.view
    }

    @Composable
    private fun Shell(home: FakeVaultRepository = vault) {
        ShellScreen(
            remember { ShellViewModel(home, accounts, events) },
            ShellScreens(
                home = { p ->
                    HomeScreen(
                        remember { HomeViewModel(home, accounts, events) },
                        onOpen = { _, _ -> },
                        onHealth = {},
                        contentPadding = p,
                    )
                },
                items = { p, open, openTag ->
                    ItemsScreen(remember { ItemListViewModel(home, accounts, events) }, open, openTag, p)
                },
                category = { p, c, _ ->
                    CategoryScreen(remember { ItemListViewModel(home, accounts, events) }, c, { _, _ -> }, p)
                },
                tag = { p, name, back ->
                    TagScreen(remember { ItemListViewModel(home, accounts, events) }, name, { _, _ -> }, back, p)
                },
                settings = { p ->
                    SettingsScreen(
                        remember {
                            SettingsViewModel(FakeSettingsRepository(), accounts, home, biometricEnrolled = { true })
                        },
                        online = false,
                        actions = SettingsActions(true, {}, { Outcome.Ok(Unit) }, { true }),
                        navigation = SettingsNavigation({}, {}, {}),
                        contentPadding = p,
                    )
                },
            ),
            ShellNavigation({}, {}, {}),
        )
    }

    private fun tap(text: String) = rule.onNodeWithText(text).performClick()

    /** Types a query, then lets the ViewModel's typing pause (main looper time, not the compose clock) pass. */
    private fun typeQuery() {
        rule.onNode(hasSetTextAction()).performTextInput("a")
        rule.waitForIdle()
        ShadowLooper.idleMainLooper(SETTLE_MS, TimeUnit.MILLISECONDS)
    }

    private val categoryShot: Shot
        get() = Shot("category", before = { tap("Items"); rule.mainClock.advanceTimeBy(SETTLE_MS); tap("Logins") }) {
            Shell()
        }

    private val tagShot: Shot
        get() = Shot("tag", before = { tap("Items"); rule.mainClock.advanceTimeBy(SETTLE_MS); tap("Work") }) {
            Shell(taggedVault)
        }

    private val shots: List<Shot> = listOf(
        Shot("home") { Shell() },
        Shot("items", before = { tap("Items") }) { Shell() },
        Shot("items-tags", before = { tap("Items") }) { Shell(taggedVault) },
        categoryShot,
        tagShot,
        Shot("settings", before = { tap("Settings") }) { Shell() },
        Shot("home-error") { Shell(failingVault) },
        Shot("search") {
            SearchScreen(remember { SearchViewModel(vault, events) }, onOpen = { _, _ -> }, onCancel = {})
        },
        Shot("search-results", before = ::typeQuery) {
            SearchScreen(remember { SearchViewModel(vault, events) }, onOpen = { _, _ -> }, onCancel = {})
        },
        Shot("add-offline") {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
                SheetSurface("New item") { AddTiles(tilesFor(online = false), onPick = {}) }
            }
        },
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
    fun everyNewScreenInBothThemes() = render("shell", shots)

    /** The narrowest common phone, offline: the top bar's tightest case. */
    @Test
    @Config(qualifiers = "w360dp-h780dp-xxhdpi")
    fun theShellOnANarrowPhone() = render("shell-360", listOf(Shot("home") { Shell() }, categoryShot))

    /** The narrowest common phone, offline, in the longer language: the top bar's worst case. */
    @Test
    @Config(qualifiers = "pt-rBR-w360dp-h780dp-xxhdpi")
    fun theShellOnANarrowPhoneInPortuguese() = render("shell-360-pt", listOf(Shot("home") { Shell() }))
}
