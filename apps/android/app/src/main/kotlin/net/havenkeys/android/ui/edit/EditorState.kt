package net.havenkeys.android.ui.edit

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.havenkeys_mobile.Change
import uniffi.havenkeys_mobile.FieldChange
import uniffi.havenkeys_mobile.ItemDraft
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.MatchKind
import uniffi.havenkeys_mobile.Website

/** Rust's limits for an item's tags; the editor previews them, Rust stays the authority. */
internal const val MAX_TAGS = 20
private const val MAX_TAG_CHARS = 32
private val Whitespace = Regex("\\s+")

class WebsiteRow(url: String, match: MatchKind) {
    var url by mutableStateOf(url)
    var match by mutableStateOf(match)
}

/**
 * The draft of one edit: what the user typed and the values loaded to edit
 * them. Owned by the edit screen's `remember` (spec §9.4) — never a
 * ViewModel, never saved state — so the lock wipe and leaving the screen
 * drop it. A field the user did not change is not sent: Rust keeps it.
 */
@Suppress("TooManyFunctions") // the screen's whole draft API, specified by the plan
class EditorState(private val edit: ItemEdit) {
    val kind = edit.kind
    var title by mutableStateOf(edit.title)
    val websites = mutableStateListOf<WebsiteRow>().apply {
        edit.websites.forEach { add(WebsiteRow(it.url, it.matchKind)) }
    }

    /** The item's tags as Rust will store them: normalised, unique, A–Z. */
    val tags = mutableStateListOf<String>().apply { addAll(edit.tags) }

    private val loaded = mutableStateMapOf<String, String>()
    private val typed = mutableStateMapOf<String, String>()
    private val opened = mutableStateMapOf<String, Boolean>()
    private val removed = mutableStateMapOf<String, Boolean>()

    init {
        edit.fields.forEach { f -> f.value?.let { loaded[f.key] = it } }
    }

    /** A current value read for editing; shown, and not a change. */
    fun load(key: String, value: String) {
        loaded[key] = value
    }

    fun shown(key: String): String = typed[key] ?: loaded[key].orEmpty()

    fun type(key: String, text: String) {
        removed.remove(key)
        if (text == loaded[key].orEmpty()) typed.remove(key) else typed[key] = text
    }

    /** A hidden value the user chose to change: shown as a text field from now on. */
    fun open(key: String) {
        opened[key] = true
    }

    fun isOpen(key: String): Boolean = opened[key] == true

    fun remove(key: String) {
        typed.remove(key)
        removed[key] = true
    }

    fun undo(key: String) {
        typed.remove(key)
        removed.remove(key)
        opened.remove(key)
    }

    fun isRemoved(key: String): Boolean = removed[key] == true

    fun addWebsite() {
        websites.add(WebsiteRow("", MatchKind.DOMAIN))
    }

    /**
     * Adds [raw] as Rust will store it (trimmed, spaces collapsed, lower
     * case); false if that is empty, too long, has a comma or a control
     * character, is already here, or the item has its 20 tags.
     */
    fun addTag(raw: String): Boolean {
        val tag = raw.trim().split(Whitespace).joinToString(" ").lowercase()
        val ok = tag.isNotEmpty() &&
            tag.codePointCount(0, tag.length) <= MAX_TAG_CHARS &&
            tag.none { it == ',' || it.isISOControl() } &&
            tag !in tags &&
            tags.size < MAX_TAGS
        if (ok) {
            tags.add(tag)
            tags.sort()
        }
        return ok
    }

    fun removeTag(tag: String) {
        tags.remove(tag)
    }

    val dirty: Boolean
        get() = title != edit.title || websitesOf() != edit.websites || tags.toList() != edit.tags ||
            changes().isNotEmpty()

    /** Carries the revision the edit was opened at: Rust refuses it if the item changed since. */
    fun toDraft(): ItemDraft = ItemDraft(kind, title.trim(), websitesOf(), changes(), edit.revision, tags.toList())

    private fun websitesOf(): List<Website> =
        websites.filter { it.url.isNotBlank() }.map { Website(it.url.trim(), it.match) }

    private fun changes(): List<FieldChange> = edit.fields.mapNotNull { f ->
        val change = when {
            removed[f.key] == true -> Change.Remove
            else -> typed[f.key]?.let { text ->
                when {
                    // Spaces typed where nothing was shown are not a value.
                    text.isBlank() && loaded[f.key] == null -> null
                    text.isNotEmpty() -> Change.Replace(text)
                    // Erasing a value that was shown removes it; an empty
                    // field that never showed one (a code's "Replace") keeps it.
                    loaded[f.key] != null -> Change.Remove
                    else -> null
                }
            }
        }
        change?.let { FieldChange(f.key, it) }
    }

    override fun toString() = "EditorState(…)"
}
