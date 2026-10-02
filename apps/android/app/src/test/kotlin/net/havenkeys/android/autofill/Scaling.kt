package net.havenkeys.android.autofill

import org.junit.Assert.assertTrue

/**
 * CLAUDE.md attack 7: a page with thousands of inputs must not slow the
 * phone down. A fixed millisecond budget fails at random on shared CI
 * runners, so this checks the shape instead: ten times the fields may cost
 * at most [MAX_RATIO] times as much (linear is 10, quadratic 100), and the
 * large page stays far under Android's fill deadline.
 */
fun assertScalesLinearly(page: (Int) -> List<FieldFacts>, work: (List<FieldFacts>) -> Unit) {
    // Warm the JIT on both sizes before measuring.
    repeat(WARM_UP) {
        work(page(SMALL))
        work(page(LARGE))
    }
    val small = fastestNanos(page, SMALL, work)
    val large = fastestNanos(page, LARGE, work)
    val ratio = large.toDouble() / small.coerceAtLeast(1)
    assertTrue("$LARGE fields cost ${"%.1f".format(ratio)}× $SMALL", ratio < MAX_RATIO)
    assertTrue("$LARGE fields took ${large / NANOS_PER_MS} ms", large / NANOS_PER_MS < CEILING_MS)
}

/** The fastest of a few runs, each on fresh fields as every request has, so a GC pause does not decide. */
private fun fastestNanos(page: (Int) -> List<FieldFacts>, size: Int, work: (List<FieldFacts>) -> Unit): Long =
    List(RUNS) {
        val fields = page(size)
        val start = System.nanoTime()
        work(fields)
        System.nanoTime() - start
    }.min()

private const val SMALL = 1_000
private const val LARGE = 10_000
private const val RUNS = 5
private const val WARM_UP = 2
private const val MAX_RATIO = 25.0
private const val CEILING_MS = 2_000
private const val NANOS_PER_MS = 1_000_000
