package net.havenkeys.android.ui.shell

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

    private val screens = ShellScreens(
        home = { HavenText("Home root") },
        items = { _, onCategory -> HavenButton("Open logins", onClick = { onCategory(Category.LOGINS) }) },
        category = { _, category, onBack -> HavenButton("List ${category.arg}", onClick = onBack) },
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
}
