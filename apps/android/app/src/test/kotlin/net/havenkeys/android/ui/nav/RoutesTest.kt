package net.havenkeys.android.ui.nav

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class RoutesTest {
    @Test
    fun aRestoredVaultScreenGivesWayToUnlockWhenLocked() {
        assertEquals(Routes.UNLOCK, routeToForce(Routes.VAULT, Start.UNLOCK))
        assertEquals(Routes.UNLOCK, routeToForce(Routes.ITEM, Start.UNLOCK))
        assertEquals(Routes.UNLOCK, routeToForce(Routes.SETTINGS, Start.UNLOCK))
    }

    @Test
    fun aRestoredScreenGivesWayToOnboardingWithoutAVault() {
        assertEquals(Routes.ONBOARDING, routeToForce(Routes.VAULT, Start.ONBOARDING))
        assertEquals(Routes.ONBOARDING, routeToForce(Routes.UNLOCK, Start.ONBOARDING))
    }

    @Test
    fun theStartScreenItselfStays() {
        assertNull(routeToForce(Routes.UNLOCK, Start.UNLOCK))
        assertNull(routeToForce(Routes.ONBOARDING, Start.ONBOARDING))
    }

    @Test
    fun anUnlockedVaultKeepsTheRestoredScreen() {
        assertNull(routeToForce(Routes.ITEM, Start.VAULT))
        assertNull(routeToForce(Routes.VAULT, Start.VAULT))
    }
}
