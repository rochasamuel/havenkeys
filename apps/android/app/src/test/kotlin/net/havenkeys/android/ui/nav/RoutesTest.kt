package net.havenkeys.android.ui.nav

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.havenkeys_mobile.ItemKind

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

    @Test
    fun anItemRouteCarriesOnlyTheId() {
        assertEquals("item/{id}", Routes.ITEM)
        assertEquals("item/0b6f6c1e-5d1a-4a8e-9a43-2f0f3c1b7d10", Routes.item("0b6f6c1e-5d1a-4a8e-9a43-2f0f3c1b7d10"))
    }

    @Test
    fun editorRoutesCarryOnlyAnIdOrAKind() {
        assertEquals("edit/abc", Routes.edit("abc"))
        assertEquals("new/login", Routes.new(ItemKind.LOGIN))
        assertEquals(ItemKind.SECURE_NOTE, creatableKind("note"))
        assertEquals(ItemKind.CARD, creatableKind("card"))
        assertNull("the identity is never created", creatableKind("identity"))
        assertNull(creatableKind("../vault"))
    }
}
