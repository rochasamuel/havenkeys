package net.havenkeys.android.ui.edit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.havenkeys_mobile.Change
import uniffi.havenkeys_mobile.EditField
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.MatchKind
import uniffi.havenkeys_mobile.Website

class EditorStateTest {
    private fun login(present: Boolean = true, tags: List<String> = emptyList()) = ItemEdit(
        ItemKind.LOGIN,
        "GitHub",
        listOf(Website("https://github.com", MatchKind.DOMAIN)),
        listOf(
            EditField("username", FieldKind.TEXT, present, if (present) "octo" else null),
            EditField("password", FieldKind.SECRET, present, null),
            EditField("totp", FieldKind.TOTP, present, null),
            EditField("notes", FieldKind.SECRET, present, null),
        ),
        false,
        true,
        7L,
        tags = tags,
    )

    private fun EditorState.changes() = toDraft().changes.associate { it.key to it.change }

    @Test
    fun anUntouchedEditorSendsNoChange() {
        val editor = EditorState(login())
        assertFalse(editor.dirty)
        val draft = editor.toDraft()
        assertEquals("GitHub", draft.title)
        assertEquals(listOf(Website("https://github.com", MatchKind.DOMAIN)), draft.websites)
        assertTrue(draft.changes.isEmpty())
        assertEquals("octo", editor.shown("username"))
    }

    @Test
    fun theDraftCarriesTheRevisionItWasOpenedAt() {
        assertEquals(7L, EditorState(login()).toDraft().baseRevision)
        assertEquals(null, EditorState(login().copy(revision = null)).toDraft().baseRevision)
    }

    @Test
    fun spacesInAnEmptyCodeAreNotSent() {
        val editor = EditorState(login())
        editor.open("totp")
        editor.type("totp", "   ")
        assertTrue(editor.changes().isEmpty())
        assertFalse(editor.dirty)
    }

    @Test
    fun typedTextReplacesAndTypingItBackSendsNothing() {
        val editor = EditorState(login())
        editor.type("username", "ana")
        assertEquals(Change.Replace("ana"), editor.changes()["username"])
        assertTrue(editor.dirty)
        editor.type("username", "octo")
        assertTrue(editor.changes().isEmpty())
    }

    @Test
    fun anOpenedFieldLeftAsItWasIsNotSent() {
        val editor = EditorState(login())
        editor.load("password", "hunter2")
        editor.open("password")
        assertEquals("hunter2", editor.shown("password"))
        assertTrue(editor.changes().isEmpty())
        editor.type("password", "hunter3")
        assertEquals(Change.Replace("hunter3"), editor.changes()["password"])
    }

    @Test
    fun erasingALoadedValueRemovesIt() {
        val editor = EditorState(login())
        editor.load("notes", "keep this")
        editor.open("notes")
        editor.type("notes", "")
        assertEquals(Change.Remove, editor.changes()["notes"])
    }

    @Test
    fun replacingACodeWithNothingKeepsIt() {
        val editor = EditorState(login())
        editor.open("totp")
        assertEquals("", editor.shown("totp"))
        editor.type("totp", "JB")
        editor.type("totp", "")
        assertTrue(editor.changes().isEmpty())
        editor.type("totp", "JBSWY3DPEHPK3PXP")
        assertEquals(Change.Replace("JBSWY3DPEHPK3PXP"), editor.changes()["totp"])
    }

    @Test
    fun removeAndUndo() {
        val editor = EditorState(login())
        editor.remove("totp")
        assertTrue(editor.isRemoved("totp"))
        assertEquals(Change.Remove, editor.changes()["totp"])
        editor.undo("totp")
        assertFalse(editor.isRemoved("totp"))
        assertTrue(editor.changes().isEmpty())
    }

    @Test
    fun aNewItemSendsWhatWasTypedOnly() {
        val editor = EditorState(login(present = false).copy(title = "", websites = emptyList()))
        editor.title = "  Bank  "
        editor.type("password", "s3cret")
        editor.type("notes", "")
        editor.addWebsite()
        editor.websites[0].url = " bank.example "
        editor.addWebsite()
        val draft = editor.toDraft()
        assertEquals("Bank", draft.title)
        assertEquals(listOf(Website("bank.example", MatchKind.DOMAIN)), draft.websites)
        assertEquals(mapOf("password" to Change.Replace("s3cret")), editor.changes())
    }

    @Test
    fun websitesAndTitleMakeItDirty() {
        val editor = EditorState(login())
        editor.websites[0].match = MatchKind.ORIGIN
        assertTrue(editor.dirty)
        val other = EditorState(login())
        other.title = "GitHub (work)"
        assertTrue(other.dirty)
    }

    @Test
    fun addTagNormalisesAndRefusesDuplicatesAndCommas() {
        val state = EditorState(login(tags = listOf("work")))
        assertTrue(state.addTag("  Prod   Server "))
        assertFalse(state.addTag("WORK"))
        assertFalse(state.addTag("a,b"))
        assertFalse(state.addTag("   "))
        assertFalse(state.addTag("x".repeat(33)))
        assertTrue(state.addTag("x".repeat(32)))
        assertEquals(listOf("Prod Server", "work", "x".repeat(32)), state.toDraft().tags)
        assertTrue(state.dirty)
    }

    @Test
    fun aTagKeepsItsCase() {
        val state = EditorState(login())
        assertTrue(state.addTag("  Dev   Team "))
        assertEquals(listOf("Dev Team"), state.toDraft().tags)
        assertEquals("Dev Team", tagForm(" Dev\u00A0Team"))
    }

    @Test
    fun aTagTheItemHasInAnotherCaseIsAQuietDuplicate() {
        val state = EditorState(login(tags = listOf("Work")))
        state.tagField.edit { replace(0, length, "work") }
        assertTrue(state.commitTypedTag())
        assertNull(state.tagRefused)
        assertEquals("", state.tagField.text.toString())
        assertEquals(listOf("Work"), state.toDraft().tags)
        assertFalse(state.dirty)
    }

    @Test
    fun aTagTheVaultHasTakesTheVaultsSpelling() {
        val state = EditorState(login(), vaultTags = listOf("Work", "Dev Team"))
        assertTrue(state.addTag("work"))
        assertTrue(state.addTag("dev  team"))
        assertFalse(state.addTag("WORK"))
        assertEquals(listOf("Dev Team", "Work"), state.toDraft().tags)
    }

    /** The only item tagged "work" may become "Work": its own tags are not the vault's spelling. */
    @Test
    fun theOnlyItemWithATagCanChangeItsCase() {
        val state = EditorState(login(tags = listOf("work")), vaultTags = emptyList())
        state.removeTag("work")
        assertTrue(state.addTag("Work"))
        assertEquals(listOf("Work"), state.toDraft().tags)
        assertTrue(state.dirty)
    }

    @Test
    fun tagsSortWithoutCase() {
        val state = EditorState(login(tags = listOf("alpha")))
        assertTrue(state.addTag("Beta"))
        assertTrue(state.addTag("Zed"))
        assertTrue(state.addTag("charlie"))
        assertEquals(listOf("alpha", "Beta", "charlie", "Zed"), state.tags.toList())
    }

    @Test
    fun unicodeWhiteSpaceIsTrimmedAndCollapsedAsRustDoes() {
        val state = EditorState(login())
        assertTrue(state.addTag("\u00A0Side\u2003\u00A0Project\u3000"))
        assertEquals(listOf("Side Project"), state.tags.toList())
        assertFalse(state.addTag("\u00A0\u2003"))
        assertTrue(state.addTag("a\u0085b"))
        assertTrue("a b" in state.tags)
    }

    @Test
    fun tagRefusalNamesWhyRustWouldRefuseATag() {
        assertEquals(TagRefusal.TooLong, tagRefusal("x".repeat(33)))
        assertEquals(TagRefusal.NotAllowed, tagRefusal("a\u0007b"))
        assertEquals(TagRefusal.NotAllowed, tagRefusal("a,b"))
        assertNull(tagRefusal("x".repeat(32)))
        assertNull(tagRefusal("  Work "))
        assertNull(tagRefusal("   "))
    }

    @Test
    fun aTypedTagIsAddedOnCommitAndARefusedOneHoldsTheSave() {
        val state = EditorState(login())
        state.tagField.edit { replace(0, length, "x".repeat(33)) }
        assertTrue(state.dirty)
        assertFalse(state.commitTypedTag())
        assertEquals(TagRefusal.TooLong, state.tagRefused)
        assertEquals("x".repeat(33), state.tagField.text.toString())
        state.tagField.edit { replace(0, length, " Work ") }
        state.tagTextChanged()
        assertNull(state.tagRefused)
        assertTrue(state.commitTypedTag())
        assertEquals("", state.tagField.text.toString())
        assertEquals(listOf("Work"), state.toDraft().tags)
    }

    @Test
    fun aCommaListKeepsItsRefusedPartsInTheField() {
        val state = EditorState(login())
        state.tagField.edit { replace(0, length, "a\u0007b,prod,ops") }
        state.tagTextChanged()
        assertEquals(listOf("prod"), state.tags.toList())
        assertEquals("a\u0007b,ops", state.tagField.text.toString())
        assertEquals(TagRefusal.NotAllowed, state.tagRefused)
    }

    @Test
    fun theTwentyFirstTagIsRefused() {
        val state = EditorState(login(tags = (1..20).map { "t%02d".format(it) }))
        assertFalse(state.addTag("more"))
        assertEquals(20, state.tags.size)
    }

    @Test
    fun removingATagMarksTheDraftDirty() {
        val state = EditorState(login(tags = listOf("work")))
        assertFalse(state.dirty)
        state.removeTag("work")
        assertEquals(emptyList<String>(), state.toDraft().tags)
        assertTrue(state.dirty)
    }

    @Test
    fun theStateNamesNoValue() {
        val editor = EditorState(login())
        editor.load("password", "hunter2")
        editor.type("password", "hunter3")
        assertFalse(editor.toString().contains("hunter"))
    }
}
