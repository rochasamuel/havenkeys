package net.havenkeys.android.ui.nav

import androidx.compose.foundation.layout.Box
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

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
            }
        }
        rule.waitForIdle()
    }

    private fun entries(route: String) = nav.currentBackStack.value.count { it.destination.route == route }

    @Test
    fun twoQuickTapsOnThePillOpenOneSearch() {
        rule.runOnIdle {
            repeat(2) { nav.ifSettled { nav.navigate(Routes.SEARCH) { launchSingleTop = true } } }
        }
        rule.waitForIdle()
        assertEquals(1, entries(Routes.SEARCH))
    }

    @Test
    fun aSecondTapDuringAPushIsIgnoredEvenWithoutSingleTop() {
        rule.runOnIdle {
            repeat(2) { nav.ifSettled { nav.navigate(Routes.GENERATOR) } }
        }
        rule.waitForIdle()
        assertEquals(1, entries(Routes.GENERATOR))
    }

    @Test
    fun aTapOnASettledScreenNavigates() {
        rule.runOnIdle { nav.ifSettled { nav.navigate(Routes.GENERATOR) } }
        rule.waitForIdle()
        assertEquals(1, entries(Routes.GENERATOR))
    }
}
