package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.havenkeys_mobile.CardChoice

class WalletRowsTest {
    @Test
    fun aCardRowShowsTheLastFourAndTheExpiryOnly() {
        val visa = CardChoice("c1", "Visa", "visa", "1111", "2033-04")
        assertEquals("•••• 1111 · 04/33", cardSubtitle(visa, expired = false, expiredWord = "Expired"))
        assertEquals("•••• 1111 · 04/33 · Expired", cardSubtitle(visa, expired = true, expiredWord = "Expired"))
        assertEquals("", cardSubtitle(visa.copy(last4 = null, expiry = null), false, "Expired"))
    }

    @Test
    fun aMalformedExpiryIsNotShown() {
        assertEquals("04/33", shortExpiry("2033-04"))
        assertNull(shortExpiry("33-4"))
    }
}

class LoginSaveFallbackTest {
    private val save = SaveForm(null, null, 0, null, null)

    @Test
    fun walletAnswerWinsAndIsNotReplaced() {
        assertEquals("wallet", withLoginSaveFallback("wallet", save) { "login" })
    }

    @Test
    fun aSignUpWithoutAWalletAnswerFallsBackToTheLoginSave() {
        assertEquals("login", withLoginSaveFallback<String>(null, save) { "login" })
    }

    @Test
    fun noSaveFormMeansNoFallback() {
        assertNull(withLoginSaveFallback<String>(null, null) { "login" })
    }
}
