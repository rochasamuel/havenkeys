package net.havenkeys.android.data

import kotlinx.coroutines.test.runTest
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.settings
import org.junit.Assert.assertEquals
import org.junit.Test

class ClipboardSecondsTest {
    @Test
    fun theSettingDecides() = runTest {
        val repo = FakeSettingsRepository().apply { current = Outcome.Ok(settings().copy(clipboardClearSeconds = 45u)) }
        assertEquals(45, repo.clipboardClearSeconds())
    }

    @Test
    fun unreadableSettingsFallBackToThirtySeconds() = runTest {
        val repo = FakeSettingsRepository().apply { current = Outcome.Failed("locked") }
        assertEquals(30, repo.clipboardClearSeconds())
    }
}
