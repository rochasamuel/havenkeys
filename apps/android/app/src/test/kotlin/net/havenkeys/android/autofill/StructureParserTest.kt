package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Test

class StructureParserTest {
    private class Node(val name: String, val children: List<Node> = emptyList())

    private fun walk(vararg roots: Node, accept: (Node) -> Boolean = { true }) =
        StructureParser.cap(
            roots = roots.map { it to "" },
            childCount = { it.children.size },
            childAt = { n, i -> n.children[i] },
            context = { n, parent -> parent + "/" + n.name },
            accept = accept,
        )

    @Test
    fun visitsInViewOrderWithInheritedContext() {
        val tree = Node("a", listOf(Node("b", listOf(Node("c"))), Node("d")))
        val got = walk(tree, Node("e"))
        assertEquals(listOf("a", "b", "c", "d", "e"), got.map { it.first.name })
        assertEquals("/a/b/c", got[2].second)
    }

    @Test
    fun thousandsOfInputsAreCappedAtMaxFields() {
        val page = Node("body", List(5_000) { Node("input$it") })
        val got = walk(page) { it.name.startsWith("input") }
        assertEquals(StructureParser.MAX_FIELDS, got.size)
        assertEquals("input0", got.first().first.name)
    }

    @Test
    fun aDeepTreeNeitherOverflowsTheStackNorWalksForever() {
        var deep = Node("leaf")
        repeat(200_000) { deep = Node("div", listOf(deep)) }
        var visited = 0
        val got = StructureParser.cap(
            roots = listOf(deep to 0),
            childCount = { it.children.size },
            childAt = { n, i -> n.children[i] },
            context = { _, depth -> depth + 1 },
            accept = { visited++; false },
        )
        assertEquals(0, got.size)
        assertEquals(StructureParser.MAX_NODES, visited)
    }

    @Test
    fun aNodeClaimingMillionsOfChildrenIsReadOnlyUpToTheNodeCap() {
        var read = 0
        val got = StructureParser.cap(
            roots = listOf(Node("root") to Unit),
            childCount = { if (it.name == "root") Int.MAX_VALUE else 0 },
            childAt = { _, _ -> read++; Node("div") },
            context = { _, _ -> },
            accept = { false },
        )
        assertEquals(0, got.size)
        assertEquals(StructureParser.MAX_NODES - 1, read)
    }
}
