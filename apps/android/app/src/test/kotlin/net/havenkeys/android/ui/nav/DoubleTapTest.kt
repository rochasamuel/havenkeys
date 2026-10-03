package net.havenkeys.android.ui.nav

import androidx.compose.foundation.layout.Box
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Pushes made in one frame: the second lands while the first is still moving in (spec §7). */
@RunWith(RobolectricTestRunner::class)
class DoubleTapTest {
    @get:Rule
    val rule = createComposeRule()

    private lateinit var nav: NavHostController

    @Before
    fun show() {
        rule.setKit {
            nav = rememberNavController()
            NavHost(nav, startDestination = Routes.SHELL) {
                composable(Routes.SHELL) { Box {} }
                composable(Routes.SEARCH) { Box {} }
                composable(Routes.GENERATOR) { Box {} }
                composable(Routes.ITEM, arguments = listOf(navArgument(Routes.ITEM_ID) { type = NavType.StringType })) {
                    Box {}
                }
            }
        }
        rule.waitForIdle()
    }

    private fun stack() = nav.currentBackStack.value.mapNotNull { it.concreteRoute() }.filter { it != Routes.SHELL }

    @Test
    fun twoQuickTapsOnOneTargetOpenItOnce() {
        rule.runOnIdle { repeat(2) { nav.pushOnce(Routes.SEARCH) } }
        rule.runOnIdle { repeat(2) { nav.pushOnce(Routes.item("1")) } }
        rule.waitForIdle()
        assertEquals(listOf(Routes.SEARCH, "item/1"), stack())
    }

    @Test
    fun aTapOnAnotherTargetDuringAPushStillGoesThere() {
        rule.runOnIdle {
            nav.pushOnce(Routes.item("1"))
            nav.pushOnce(Routes.GENERATOR)
        }
        rule.waitForIdle()
        assertEquals(listOf("item/1", Routes.GENERATOR), stack())
    }

    @Test
    fun anotherItemIsAnotherTarget() {
        rule.runOnIdle {
            nav.pushOnce(Routes.item("1"))
            nav.pushOnce(Routes.item("2"))
        }
        rule.waitForIdle()
        assertEquals(listOf("item/1", "item/2"), stack())
    }

    @Test
    fun aRouteIsReadBackWithItsArguments() {
        rule.runOnIdle { nav.pushOnce(Routes.item("abc")) }
        rule.waitForIdle()
        assertEquals("item/abc", nav.currentBackStackEntry?.concreteRoute())
    }
}
