package net.havenkeys.android.autofill

import android.app.assist.AssistStructure
import android.app.assist.AssistStructure.ViewNode
import android.view.View
import android.view.autofill.AutofillId

/** A screen's fillable text fields; `ids[i]` is the AutofillId of `fields[i]`. */
data class ParsedScreen(val packageName: String, val fields: List<FieldFacts>, val ids: List<AutofillId>)

/**
 * Reads an AssistStructure into [FieldFacts]. A thin traversal: every
 * decision is in [FieldClassifier] and [LoginFormFinder]. The structure is
 * built by the app or page being filled, so everything is bounded (fields,
 * nodes, string lengths, attributes) and the walk never recurses.
 */
class StructureParser {
    fun parse(structure: AssistStructure): ParsedScreen {
        val roots = (0 until structure.windowNodeCount).map { structure.getWindowNodeAt(it).rootViewNode }
        val top = Frame(domain = null, scheme = null, visible = true)
        val found = cap(
            roots = roots.map { it to top },
            childCount = { it.childCount },
            childAt = { node, i -> node.getChildAt(i) },
            context = ::frameOf,
            accept = { it.autofillId != null && it.autofillType == View.AUTOFILL_TYPE_TEXT },
        )
        return ParsedScreen(
            packageName = structure.activityComponent?.packageName.orEmpty(),
            fields = found.mapIndexed { index, (node, frame) -> factsOf(index, node, frame) },
            ids = found.map { (node, _) -> requireNotNull(node.autofillId) },
        )
    }

    /** What a node inherits: its frame's web domain and scheme, and whether an ancestor hides it. */
    private data class Frame(val domain: String?, val scheme: String?, val visible: Boolean)

    // The domain and scheme are recorded as given (not trimmed): Rust
    // validates them, and a shortened domain could name another site.
    private fun frameOf(node: ViewNode, parent: Frame) = Frame(
        domain = node.webDomain ?: parent.domain,
        scheme = if (node.webDomain != null) node.webScheme else parent.scheme,
        visible = parent.visible && node.visibility == View.VISIBLE,
    )

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
    )

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
        private const val MAX_TEXT = 200
        private const val MAX_HINTS = 16
        private const val MAX_ATTRIBUTES_READ = 64
        private val HTML_ATTRIBUTES = setOf(
            "type", "name", "id", "autocomplete", "placeholder", "aria-label", "label", "inputmode", "maxlength",
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
