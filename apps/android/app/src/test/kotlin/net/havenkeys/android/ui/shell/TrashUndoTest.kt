package net.havenkeys.android.ui.shell

import androidx.compose.runtime.remember
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.rememberToastState
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class TrashUndoTest {
    @get:Rule
    val rule = createComposeRule()

    private val restored = mutableListOf<String>()
    private var answer: Outcome<Unit> = Outcome.Ok(Unit)
    private var syncs = 0
    private val undo = TrashUndo(
        restore = { id ->
            restored += id
            answer
        },
        sync = { syncs++ },
    )

    private fun show() {
        rule.setKit {
            val toasts = rememberToastState()
            TrashUndoToast(remember { undo }, toasts)
            HavenScaffold(toastState = toasts) { HavenText("Shell") }
        }
    }

    @Test
    fun aTrashedItemIsOfferedOnceWithUndo() {
        show()
        rule.runOnIdle { undo.offer("id-1", "GitHub") }
        rule.onNodeWithText("Moved “GitHub” to Trash.").assertExists()
        assertNull(undo.pending)
        rule.onNodeWithText("Undo").performClick()
        rule.waitForIdle()
        assertEquals(listOf("id-1"), restored)
    }

    @Test
    fun aFailedUndoSaysWhy() {
        answer = Outcome.Failed("offline")
        show()
        rule.runOnIdle { undo.offer("id-1", "GitHub") }
        rule.onNodeWithText("Undo").performClick()
        rule.onNodeWithText("HavenKeys is offline — the vault is read-only until it reconnects.").assertExists()
    }

    @Test
    fun anUndoRefusedBecauseAnotherDeviceChangedItSyncsAndSaysSo() {
        answer = Outcome.Failed("item_changed_elsewhere")
        show()
        rule.runOnIdle { undo.offer("id-1", "GitHub") }
        rule.onNodeWithText("Undo").performClick()
        rule.onNodeWithText("This item changed on another device.").assertExists()
        assertEquals(1, syncs)
    }

    @Test
    fun anUndoThatFailsOtherwiseDoesNotSync() {
        answer = Outcome.Failed("offline")
        show()
        rule.runOnIdle { undo.offer("id-1", "GitHub") }
        rule.onNodeWithText("Undo").performClick()
        rule.waitForIdle()
        assertEquals(0, syncs)
    }

    @Test
    fun anItemDeletedForGoodIsOnlyReported() {
        show()
        rule.runOnIdle { undo.deletedForGood("id-1", "GitHub") }
        rule.onNodeWithText("Deleted “GitHub”.").assertExists()
        rule.onNodeWithText("Undo").assertDoesNotExist()
    }
}
