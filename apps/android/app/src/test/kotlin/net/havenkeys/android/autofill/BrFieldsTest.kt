package net.havenkeys.android.autofill

import android.view.View
import org.json.JSONObject
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The shared Brazilian field fixture (packages/field-words/fixtures, spec
 * 2026-10-19-shared-field-words-design §6): each real field, on its own,
 * must get the role the fixture names from each classifier. The
 * extension's br-fields.test.ts runs the same records. A known gap must
 * still fail, so a fix forces removing its entry.
 */
@RunWith(RobolectricTestRunner::class)
class BrFieldsTest {
    private val fixture = JSONObject(
        requireNotNull(javaClass.classLoader?.getResource("br-fields.json")).readText(),
    )

    @Test
    fun everyRecordGetsItsRoleOnAndroid() {
        val gaps = fixture.getJSONArray("known_gaps").let { a ->
            (0 until a.length()).map { a.getJSONObject(it) }
                .filter { it.getString("platform") == "android" }
                .map { key(it, it.getString("classifier")) }
                .toSet()
        }
        val records = fixture.getJSONArray("records")
        val wrong = mutableListOf<String>()
        for (i in 0 until records.length()) {
            val r = records.getJSONObject(i)
            val f = facts(r)
            val g = bankFacts(r)
            for (c in listOf("login", "identity", "card", "bank")) {
                val expected = r.getString(c)
                if (expected == "skip") continue
                val got = actual(f, g, c)
                val isGap = key(r, c) in gaps
                if ((got == expected) == isGap) {
                    wrong += JSONObject()
                        .put("site", r.getString("site")).put("id", r.getString("id"))
                        .put("label_text", r.getString("label_text")).put("classifier", c)
                        .put("expected", expected).put("got", got).put("gap", isGap).toString()
                }
            }
        }
        val message = "Mismatches (gap=true: a known gap now passes, remove it):\n${wrong.joinToString("\n")}"
        assertTrue(message, wrong.isEmpty())
    }

    /** A record (or a gap naming one) and a classifier. */
    private fun key(r: JSONObject, classifier: String) =
        "${r.getString("site")}|${r.getString("id")}|${r.getString("label_text")}|$classifier"

    private fun facts(r: JSONObject): FieldFacts {
        val select = r.getString("tag") == "select"
        val html = buildMap {
            for (a in listOf("type", "name", "id", "placeholder", "autocomplete", "inputmode", "maxlength")) {
                val v = r.getString(a)
                if (v.isNotEmpty() && !(select && a == "type")) put(a, v)
            }
            r.getString("aria_label").takeIf { it.isNotEmpty() }?.let { put("aria-label", it) }
            r.getString("label_text").takeIf { it.isNotEmpty() }?.let { put("label", it) }
        }
        return field(
            html = html,
            tag = r.getString("tag"),
            autofillType = if (select) View.AUTOFILL_TYPE_LIST else View.AUTOFILL_TYPE_TEXT,
        )
    }

    /** The bank group facts the record's page has (none, by default). */
    private fun bankFacts(r: JSONObject): BankGroupFacts {
        val c = r.optJSONObject("context")
        val prev = c?.optString("prevRole")?.takeIf { it.isNotEmpty() && it != "null" }
        return BankGroupFacts(
            groupHasBranch = c?.optBoolean("groupHasBranch") ?: false,
            groupHasAccount = c?.optBoolean("groupHasAccount") ?: false,
            prevRole = prev?.let { p -> BankRole.entries.single { bankName(it) == p } },
        )
    }

    private fun actual(f: FieldFacts, g: BankGroupFacts, classifier: String): String = when (classifier) {
        "card" -> CardFieldClassifier.kindOf(f, strong = false)?.kind?.name?.let(::camel) ?: "none"
        "identity" -> IdentityFieldClassifier.roleOf(f)?.role?.name?.let(::camel) ?: "none"
        "bank" -> BankFieldClassifier.roleOf(f, g)?.let(::bankName) ?: "none"
        else -> loginRole(f, g)
    }

    /** The login classifier refuses a bank box, as [loginRolesOf] does. */
    private fun loginRole(f: FieldFacts, g: BankGroupFacts): String =
        if (BankFieldClassifier.roleOf(f, g) != null) "none" else when (FieldClassifier.classify(f).role) {
            FieldRole.USERNAME -> "username"
            FieldRole.PASSWORD -> "password"
            FieldRole.NEW_PASSWORD -> "new-password"
            FieldRole.CONFIRM_PASSWORD -> "confirmation-password"
            FieldRole.OTP -> "otp"
            FieldRole.UNKNOWN -> "none"
        }

    /** `BRANCH_DIGIT` → `bankBranchDigit`, the fixture's names. */
    private fun bankName(role: BankRole) = camel("BANK_${role.name}")

    /** `CARDHOLDER_NAME` → `cardholderName`, the fixture's (protocol's) names. */
    private fun camel(name: String) = name.lowercase().split('_')
        .mapIndexed { i, w -> if (i == 0) w else w.replaceFirstChar(Char::uppercase) }.joinToString("")
}
