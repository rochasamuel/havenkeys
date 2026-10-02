package net.havenkeys.android.ui.edit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
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
    private fun login(present: Boolean = true) = ItemEdit(
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
    fun theStateNamesNoValue() {
        val editor = EditorState(login())
        editor.load("password", "hunter2")
        editor.type("password", "hunter3")
        assertFalse(editor.toString().contains("hunter"))
    }
}
