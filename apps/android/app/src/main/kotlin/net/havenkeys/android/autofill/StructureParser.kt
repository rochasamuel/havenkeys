package net.havenkeys.android.autofill

import android.app.assist.AssistStructure
import android.app.assist.AssistStructure.ViewNode
import android.view.View
import android.view.autofill.AutofillId
import android.view.autofill.AutofillValue
import java.time.Instant
import java.time.ZoneId

/**
 * A screen's fillable fields; `ids[i]` is the AutofillId of `fields[i]`.
 * [pageDomain]/[pageScheme]: the outermost web domain, the tab's page (a
 * browser's frames carry their own).
 */
data class ParsedScreen(
    val packageName: String,
    val fields: List<FieldFacts>,
    val ids: List<AutofillId>,
    val pageDomain: String? = null,
    val pageScheme: String? = null,
)

/**
 * The tab's page (domain, scheme) from the first field inside a page: a
 * browser's own field (its address bar) can come first and has none.
 */
internal fun pageOf(fieldPages: List<Pair<String?, String?>>): Pair<String?, String?>? =
    fieldPages.firstOrNull { it.first != null }

/**
 * Reads an AssistStructure into [FieldFacts]. A thin traversal: every
 * decision is in [FieldClassifier] and [LoginFormFinder]. The structure is
 * built by the app or page being filled, so everything is bounded (fields,
 * nodes, string lengths, attributes, options) and the walk never recurses.
 * A field's value is read only to tell whether it is empty, and, for a save
 * the user confirmed, from the saved fields.
 */
class StructureParser {
    fun parse(structure: AssistStructure): ParsedScreen {
        val roots = (0 until structure.windowNodeCount).map { structure.getWindowNodeAt(it).rootViewNode }
        val top = Frame(domain = null, scheme = null, visible = true, pageDomain = null, pageScheme = null)
        val found = cap(
            roots = roots.map { it to top },
            childCount = { it.childCount },
            childAt = { node, i -> node.getChildAt(i) },
            context = ::frameOf,
            accept = { it.autofillId != null && it.autofillType in FILLABLE_TYPES },
        )
        val page = pageOf(found.map { (_, frame) -> frame.pageDomain to frame.pageScheme })
        return ParsedScreen(
            packageName = structure.activityComponent?.packageName.orEmpty(),
            fields = found.mapIndexed { index, (node, frame) -> factsOf(index, node, frame) },
            ids = found.map { (node, _) -> requireNotNull(node.autofillId) },
            pageDomain = page?.first,
            pageScheme = page?.second,
        )
    }

    /**
     * The typed text of the [wanted] fields only, for a save the user just
     * confirmed. Bounded like [parse]; any other field's value is never read.
     */
    fun textOf(structure: AssistStructure, wanted: Set<AutofillId>): Map<AutofillId, String> {
        if (wanted.isEmpty()) return emptyMap()
        val roots = (0 until structure.windowNodeCount).map { structure.getWindowNodeAt(it).rootViewNode to Unit }
        val found = cap(
            roots = roots,
            childCount = { it.childCount },
            childAt = { node, i -> node.getChildAt(i) },
            context = { _, parent -> parent },
            accept = { it.autofillId in wanted },
        )
        return found.mapNotNull { (node, _) ->
            val id = node.autofillId ?: return@mapNotNull null
            val text = node.autofillValue?.let { valueText(node, it) }
            text?.let { id to it }
        }.toMap()
    }

    /**
     * What a node inherits: its frame's web domain and scheme, the outermost
     * domain (the tab's page), and whether an ancestor hides it.
     */
    private data class Frame(
        val domain: String?,
        val scheme: String?,
        val visible: Boolean,
        val pageDomain: String?,
        val pageScheme: String?,
    )

    // The domain and scheme are recorded as given (not trimmed): Rust
    // validates them, and a shortened domain could name another site.
    private fun frameOf(node: ViewNode, parent: Frame): Frame {
        val isPage = parent.pageDomain == null && node.webDomain != null
        return Frame(
            domain = node.webDomain ?: parent.domain,
            scheme = if (node.webDomain != null) node.webScheme else parent.scheme,
            visible = parent.visible && node.visibility == View.VISIBLE,
            pageDomain = if (isPage) node.webDomain else parent.pageDomain,
            pageScheme = if (isPage) node.webScheme else parent.pageScheme,
        )
    }

    private fun factsOf(index: Int, node: ViewNode, frame: Frame) = FieldFacts(
        index = index,
        autofillHints = node.autofillHints.orEmpty().take(MAX_HINTS).mapNotNull { it?.take(MAX_TEXT) },
        inputType = node.inputType,
        idEntry = node.idEntry?.take(MAX_TEXT),
        hint = node.hint?.take(MAX_TEXT),
        contentDescription = node.contentDescription?.take(MAX_TEXT)?.toString(),
        htmlTag = node.htmlInfo?.tag?.take(MAX_TEXT),
        htmlAttributes = htmlAttributesOf(node),
        visible = frame.visible,
        enabled = node.isEnabled,
        focused = node.isFocused,
        webDomain = frame.domain,
        webScheme = frame.scheme,
        maxTextLength = node.maxTextLength,
        autofillType = node.autofillType,
        isEmpty = isEmpty(node),
        options = node.autofillOptions.orEmpty().take(MAX_OPTIONS).map { it.toString().take(MAX_TEXT) },
    )

    // Read only to tell an empty field from a filled one; the value is not kept.
    private fun isEmpty(node: ViewNode): Boolean {
        val value = node.autofillValue
        return when {
            value == null -> true
            value.isText -> value.textValue.isEmpty()
            value.isList -> value.listValue <= 0
            else -> !value.isDate
        }
    }

    private fun valueText(node: ViewNode, value: AutofillValue): String? = when {
        value.isText -> value.textValue.toString().take(MAX_VALUE)
        value.isList -> node.autofillOptions?.getOrNull(value.listValue)?.toString()?.take(MAX_TEXT)
        value.isDate -> Instant.ofEpochMilli(value.dateValue).atZone(ZoneId.systemDefault()).toLocalDate().toString()
        else -> null
    }

    // Only the attributes the classifier reads: never `value` or anything
    // else that could carry what was typed.
    private fun htmlAttributesOf(node: ViewNode): Map<String, String> {
        val attributes = node.htmlInfo?.attributes ?: return emptyMap()
        val kept = HashMap<String, String>()
        for (pair in attributes.take(MAX_ATTRIBUTES_READ)) {
            val name = pair?.first?.lowercase()
            val value = pair?.second
            if (name != null && value != null && name in HTML_ATTRIBUTES) kept.putIfAbsent(name, value.take(MAX_TEXT))
        }
        return kept
    }

    companion object {
        const val MAX_FIELDS = 500
        const val MAX_NODES = 20_000
        const val MAX_OPTIONS = 200
        private val FILLABLE_TYPES = setOf(View.AUTOFILL_TYPE_TEXT, View.AUTOFILL_TYPE_LIST, View.AUTOFILL_TYPE_DATE)
        private const val MAX_TEXT = 200
        // Rust validates the real limits.
        private const val MAX_VALUE = 4_096
        private const val MAX_HINTS = 16
        private const val MAX_ATTRIBUTES_READ = 64
        private val HTML_ATTRIBUTES = setOf(
            "type", "name", "id", "autocomplete", "placeholder", "aria-label", "label", "inputmode", "maxlength",
            "title", "pattern",
        )

        /**
         * Depth-first in view order with an explicit stack (a hostile tree's
         * depth cannot overflow ours). Stops after [MAX_FIELDS] accepted nodes
         * or [MAX_NODES] visited ones; returns each accepted node with its
         * inherited context.
         */
        internal fun <N, C> cap(
            roots: List<Pair<N, C>>,
            childCount: (N) -> Int,
            childAt: (N, Int) -> N,
            context: (N, C) -> C,
            accept: (N) -> Boolean,
        ): List<Pair<N, C>> {
            val found = ArrayList<Pair<N, C>>()
            val stack = ArrayDeque<Pair<N, C>>()
            // Every node visited was pushed first, so this bounds the walk
            // and the stack, however many children one node claims.
            var pushed = 0
            fun push(nodes: List<Pair<N, C>>) {
                val room = minOf(nodes.size, MAX_NODES - pushed)
                for (i in room - 1 downTo 0) stack.addLast(nodes[i])
                pushed += room
            }
            push(roots)
            while (stack.isNotEmpty() && found.size < MAX_FIELDS) {
                val (node, parent) = stack.removeLast()
                val here = context(node, parent)
                if (accept(node)) found += node to here
                val room = minOf(childCount(node), MAX_NODES - pushed)
                push(List(room) { childAt(node, it) to here })
            }
            return found
        }
    }
}
