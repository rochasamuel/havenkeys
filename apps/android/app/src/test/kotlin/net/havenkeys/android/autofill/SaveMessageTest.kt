package net.havenkeys.android.autofill

import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.havenkeys_mobile.SaveResult

class SaveMessageTest {
    @Test
    fun onlyAFailureHasAMessage() {
        assertNull(saveMessage(null))
        assertNull(saveMessage(Outcome.Ok(SaveResult.ADDED)))
        assertNull(saveMessage(Outcome.Ok(SaveResult.UNCHANGED)))
        assertEquals(R.string.autofill_save_offline, saveMessage(Outcome.Failed("offline")))
        assertEquals(R.string.autofill_save_locked, saveMessage(Outcome.Failed("locked")))
        assertEquals(R.string.autofill_save_failed, saveMessage(Outcome.Failed("denied")))
    }
}
