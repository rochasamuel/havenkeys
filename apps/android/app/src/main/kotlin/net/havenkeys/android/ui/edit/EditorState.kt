package net.havenkeys.android.ui.edit

import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
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

/**
 * Unicode's White_Space, as Rust's `char::is_whitespace`: Kotlin's set plus
 * NEL, less the four separators U+001C–U+001F (control characters to Rust).
 * Not a regex: Android's ICU patterns do not take the Unicode-classes flag.
 */
private fun Char.isRustWhitespace(): Boolean = this == '\u0085' || (isWhitespace() && this !in '\u001C'..'\u001F')

/** [raw] as Rust stores a tag: white space trimmed and collapsed to one space, case kept. */
internal fun tagForm(raw: String): String =
    buildString { raw.forEach { append(if (it.isRustWhitespace()) ' ' else it) } }
        .split(' ')
        .filter { it.isNotEmpty() }
        .joinToString(" ")

/** What two tags are compared by, as Rust's `tags::key`: the same tag whatever its case. */
internal fun tagKey(tag: String): String = tag.lowercase()

/** Tags A–Z without case, then by spelling, as Rust sorts an item's tags. */
private val TagOrder: Comparator<String> = compareBy<String> { tagKey(it) }.thenBy { it }

/** Why Rust would refuse a typed tag, for the editor to say (never echoing it). */
internal enum class TagRefusal { TooLong, NotAllowed }

/** Why Rust would refuse [raw] as a tag; null when it would take it, or when it is blank. */
internal fun tagRefusal(raw: String): TagRefusal? {
    val tag = tagForm(raw)
    return when {
        tag.isEmpty() -> null
        tag.any { it == ',' || it.isISOControl() } -> TagRefusal.NotAllowed
        tag.codePointCount(0, tag.length) > MAX_TAG_CHARS -> TagRefusal.TooLong
        else -> null
    }
}

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
class EditorState(private val edit: ItemEdit, vaultTags: List<String> = emptyList()) {
    val kind = edit.kind
    var title by mutableStateOf(edit.title)
    val websites = mutableStateListOf<WebsiteRow>().apply {
        edit.websites.forEach { add(WebsiteRow(it.url, it.matchKind)) }
    }

    /** The item's tags as Rust will store them: normalised, unique without case, A–Z. */
    val tags = mutableStateListOf<String>().apply { addAll(edit.tags) }

    /** The other items' spelling of each tag, by [tagKey]: a typed tag takes it, as Rust would. */
    private val spellings = vaultTags.associateBy(::tagKey)

    /** The Add tag field: text typed but not yet a tag. Draft state, never saved. */
    internal val tagField = TextFieldState()

    /** Why the typed tag was not added; cleared once the text changes to one Rust would take. */
    internal var tagRefused by mutableStateOf<TagRefusal?>(null)
        private set

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
     * Adds [raw] as Rust will store it (trimmed, spaces collapsed, in the
     * spelling the vault already uses for it, else as typed); false if that
     * is empty, too long, has a comma or a control character, is already
     * here in any case, or the item has its 20 tags.
     */
    fun addTag(raw: String): Boolean {
        val typed = tagForm(raw)
        val key = tagKey(typed)
        val ok = typed.isNotEmpty() &&
            tagRefusal(raw) == null &&
            tags.none { tagKey(it) == key } &&
            tags.size < MAX_TAGS
        if (ok) {
            tags.add(spellings[key] ?: typed)
            tags.sortWith(TagOrder)
        }
        return ok
    }

    fun removeTag(tag: String) {
        tags.remove(tag)
    }

    /**
     * The Add tag field changed. A comma ends a tag, as on the desktop: each
     * one before it is added, except one Rust would refuse, which stays in
     * the field (commas kept) with [tagRefused] saying why.
     */
    internal fun tagTextChanged() {
        val now = tagField.text.toString()
        tagRefused = null
        if (',' !in now) return
        val parts = now.split(',')
        val kept = parts.dropLast(1).filter { part ->
            val why = tagRefusal(part)
            if (why == null) addTag(part) else if (tagRefused == null) tagRefused = why
            why != null
        }
        val rest = (kept + parts.last()).joinToString(",")
        if (rest != now) tagField.setTextAndPlaceCursorAtEnd(rest)
    }

    /**
     * Adds what is typed in the Add tag field. False when Rust would refuse
     * it: the text stays to be corrected and [tagRefused] says why, so a
     * Save must wait. A tag the item has, or a 21st, just empties the field.
     */
    internal fun commitTypedTag(): Boolean {
        val typed = tagField.text.toString()
        tagRefusal(typed)?.let {
            tagRefused = it
            return false
        }
        if (tagForm(typed).isNotEmpty()) {
            addTag(typed)
            tagField.clearText()
        }
        return true
    }

    /** A typed tag not yet added counts as a change: Back asks before it is lost. */
    val dirty: Boolean
        get() = title != edit.title || websitesOf() != edit.websites || tags.toList() != edit.tags ||
            tagForm(tagField.text.toString()).isNotEmpty() || changes().isNotEmpty()

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
