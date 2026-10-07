package net.havenkeys.android.ui.shell

import android.os.Parcel
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.navigation.NavHostController
import androidx.navigation.compose.rememberNavController
import net.havenkeys.android.ui.items.Category
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ShellNavHostTest {
    @get:Rule
    val rule = createComposeRule()

    private lateinit var nav: NavHostController
    private var openCategory: ((Category) -> Unit)? = null
    private var openTag: ((String) -> Unit)? = null

    private val screens = ShellScreens(
        home = { HavenText("Home root") },
        items = { _, onCategory, onTag ->
            openCategory = onCategory
            openTag = onTag
            Column {
                HavenButton("Open logins", onClick = { onCategory(Category.LOGINS) })
                HavenButton("Open tag", onClick = { onTag("work/staging") })
            }
        },
        category = { _, category, onBack -> HavenButton("List ${category.arg}", onClick = onBack) },
        tag = { _, name, onBack -> HavenButton("Tag $name", onClick = onBack) },
        settings = { HavenText("Settings root") },
    )

    @Before
    fun show() {
        rule.setKit {
            nav = rememberNavController()
            ShellNavHost(nav, screens, PaddingValues())
        }
    }

    private fun select(tab: Tab) {
        rule.runOnIdle { nav.selectTab(tab) }
        rule.waitForIdle()
    }

    @Test
    fun itStartsOnHome() {
        rule.onNodeWithText("Home root").assertIsDisplayed()
    }

    @Test
    fun eachTabKeepsItsOwnStack() {
        select(Tab.ITEMS)
        rule.onNodeWithText("Open logins").performClick()
        rule.onNodeWithText("List logins").assertIsDisplayed()
        select(Tab.HOME)
        rule.onNodeWithText("Home root").assertIsDisplayed()
        select(Tab.ITEMS)
        rule.onNodeWithText("List logins").assertIsDisplayed()
    }

    @Test
    fun reselectingATabPopsItToItsRoot() {
        select(Tab.ITEMS)
        rule.onNodeWithText("Open logins").performClick()
        select(Tab.ITEMS)
        rule.onNodeWithText("Open logins").assertIsDisplayed()
        rule.onNodeWithText("List logins").assertDoesNotExist()
    }

    @Test
    fun backFromAnotherTabsRootGoesHome() {
        select(Tab.SETTINGS)
        rule.onNodeWithText("Settings root").assertIsDisplayed()
        rule.runOnIdle { nav.popBackStack() }
        rule.onNodeWithText("Home root").assertIsDisplayed()
    }

    @Test
    fun aCategoryListBacksOutToItems() {
        select(Tab.ITEMS)
        rule.onNodeWithText("Open logins").performClick()
        rule.onNodeWithText("List logins").performClick()
        rule.onNodeWithText("Open logins").assertIsDisplayed()
    }

    @Test
    fun aTagWithASlashOpensItsListAndBacksOutToItems() {
        select(Tab.ITEMS)
        rule.onNodeWithText("Open tag").performClick()
        rule.runOnIdle { assertEquals(ShellRoutes.TAG, nav.currentDestination?.route) }
        rule.onNodeWithText("Tag work/staging").performClick()
        rule.onNodeWithText("Open tag").assertIsDisplayed()
    }

    /**
     * Final review (tags): a tag's name was a route argument, so it landed in
     * the saved-state Bundle. Neither the route nor the saved back stacks
     * carry it now.
     */
    @Test
    fun aTagsNameIsInNoRouteAndNoSavedState() {
        val name = "secret-project"
        select(Tab.ITEMS)
        rule.runOnIdle { openTag!!(name) }
        rule.onNodeWithText("Tag $name").assertIsDisplayed()
        // Saved as a tab switch saves it, and as the activity's state is written.
        select(Tab.HOME)
        rule.runOnIdle {
            val routes = nav.currentBackStack.value.mapNotNull { it.destination.route }
            assertFalse(routes.any { name in it })
            val parcel = Parcel.obtain()
            try {
                parcel.writeBundle(nav.saveState())
                val bytes = parcel.marshall()
                for (encoded in listOf(Charsets.UTF_8, Charsets.UTF_16LE)) {
                    val needle = name.toByteArray(encoded)
                    assertFalse("the tag is in the saved state ($encoded)", bytes.contains(needle))
                }
            } finally {
                parcel.recycle()
            }
        }
        select(Tab.ITEMS)
        rule.onNodeWithText("Tag $name").assertIsDisplayed()
    }

    /** A tag list with no name in memory (after a process death) backs out to Items. */
    @Test
    fun aTagListWithNoNameBacksOut() {
        select(Tab.ITEMS)
        rule.runOnIdle { nav.navigate(ShellRoutes.TAG) }
        rule.waitForIdle()
        rule.onNodeWithText("Open logins").assertIsDisplayed()
    }

    @Test
    fun twoQuickTapsOnTwoTagsOpenTheSecond() {
        select(Tab.ITEMS)
        rule.runOnIdle {
            openTag!!("work")
            openTag!!("home")
        }
        rule.onNodeWithText("Tag home").assertIsDisplayed()
        rule.runOnIdle { nav.popBackStack() }
        rule.onNodeWithText("Tag work").assertIsDisplayed()
    }

    private fun ByteArray.contains(needle: ByteArray) =
        indices.any { i -> needle.indices.all { j -> getOrNull(i + j) == needle[j] } }

    @Test
    fun anUnknownCategoryOpensNothing() {
        select(Tab.ITEMS)
        rule.runOnIdle { nav.navigate("items/identity") }
        rule.waitForIdle()
        rule.onNodeWithText("Open logins").assertIsDisplayed()
    }

    @Test
    fun aCategoryListBelongsToItems() {
        select(Tab.ITEMS)
        rule.onNodeWithText("Open logins").performClick()
        rule.runOnIdle { assertEquals(Tab.ITEMS, nav.currentDestination?.tab()) }
    }

    /** Review (tags): a tag that encodes differently ("side project") was pushed twice by a double tap. */
    @Test
    fun twoQuickTapsOnATagWithASpaceOpenItOnce() {
        select(Tab.ITEMS)
        rule.runOnIdle { repeat(2) { openTag!!("side project") } }
        rule.waitForIdle()
        rule.onNodeWithText("Tag side project").assertIsDisplayed()
        rule.runOnIdle { nav.popBackStack() }
        rule.waitForIdle()
        rule.onNodeWithText("Open logins").assertIsDisplayed()
    }

    /** Final review (stage 4): a double tap on a category pushed its list twice. */
    @Test
    fun twoQuickTapsOnACategoryOpenItOnce() {
        select(Tab.ITEMS)
        rule.runOnIdle { repeat(2) { openCategory!!(Category.LOGINS) } }
        rule.waitForIdle()
        val lists = nav.currentBackStack.value.count { it.destination.route == ShellRoutes.CATEGORY }
        assertEquals(1, lists)
    }
}
