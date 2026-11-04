package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The extension's bank.test.ts, case for case. */
class BankFieldClassifierTest {
    private fun box(
        label: String,
        vararg attrs: Pair<String, String>,
        enabled: Boolean = true,
        domain: String? = null,
    ) =
        field(
            html = buildMap {
                put("id", "x")
                if (label.isNotEmpty()) put("label", label)
                putAll(attrs)
            },
            enabled = enabled,
            domain = domain,
        )

    private fun facts(branch: Boolean = false, account: Boolean = false, prev: BankRole? = null) =
        BankGroupFacts(groupHasBranch = branch, groupHasAccount = account, prevRole = prev)

    private fun role(f: FieldFacts, g: BankGroupFacts = facts()) = BankFieldClassifier.roleOf(f, g)

    /** The roles of [fields], in order, as [BankFieldClassifier.roles] reads them. */
    private fun roles(fields: List<FieldFacts>): List<BankRole> {
        val found = BankFieldClassifier.roles(fields)
        return fields.mapNotNull { found[it.index] }
    }

    @Test
    fun contaWithoutBranchIsNotBank() {
        val conta = box("Conta", "inputmode" to "numeric", "maxlength" to "7")
        assertNull(role(conta))
        assertEquals(BankRole.ACCOUNT, role(conta, facts(branch = true)))
    }

    @Test
    fun entrarNaSuaContaIsNotBank() {
        assertNull(role(box("Entrar na sua conta"), facts(branch = true)))
        assertNull(role(box("Criar conta"), facts(branch = true)))
    }

    @Test
    fun agenciaDeViagemIsNotBank() {
        for (label in listOf("Agência de viagem", "Agência de turismo", "Travel agency")) {
            assertNull(label, role(box(label, "maxlength" to "6"), facts(account = true)))
        }
    }

    @Test
    fun textAgBoxNeedsAccountNeighbour() {
        assertNull(role(box("Ag", "type" to "text")))
        assertEquals(BankRole.BRANCH, role(box("Ag", "type" to "text"), facts(account = true)))
    }

    @Test
    fun aNumericBranchBoxStandsAlone() {
        // Sicoob's cooperativa, BB's 6-long box.
        assertEquals(BankRole.BRANCH, role(box("Digite sua cooperativa", "type" to "number", "maxlength" to "4")))
        assertEquals(BankRole.BRANCH, role(box("Agência", "name" to "dependenciaOrigem", "maxlength" to "6")))
    }

    @Test
    fun shortWordsMatchWholeWordsOnly() {
        assertNull(role(box("Page", "maxlength" to "4"), facts(account = true)))
        assertNull(role(box("Advogado"), facts(prev = BankRole.ACCOUNT)))
        assertNull(role(box("Digital"), facts(prev = BankRole.ACCOUNT)))
    }

    @Test
    fun oneCharBoxAfterAccountIsAccountDigit() {
        assertEquals(
            BankRole.ACCOUNT_DIGIT,
            role(box("", "maxlength" to "1"), facts(branch = true, prev = BankRole.ACCOUNT)),
        )
        assertEquals(BankRole.BRANCH_DIGIT, role(box("DV"), facts(account = true, prev = BankRole.BRANCH)))
        assertNull(role(box("", "maxlength" to "1"), facts(branch = true)))
    }

    @Test
    fun passwordAndOtpBoxesAreNeverBank() {
        val g = facts(branch = true, account = true, prev = BankRole.ACCOUNT)
        assertNull(role(box("Agência", "type" to "password", "maxlength" to "4"), g))
        assertNull(role(box("Dígito", "type" to "password", "maxlength" to "1"), g))
        assertNull(role(box("Agência", "autocomplete" to "one-time-code", "maxlength" to "4"), g))
        assertNull(role(box("Conta", "type" to "hidden"), g))
        assertNull(role(box("Conta", enabled = false), g))
        assertNull(role(box("Conta", "readonly" to ""), g))
        assertNull(role(box("Conta", "autocomplete" to "username"), g))
        assertNull(role(box("Número da conta do cartão", "autocomplete" to "cc-number"), g))
        // Android's own: a native password box, and a field the app names by an autofill hint.
        assertNull(role(field(hint = "Agência", inputType = password, maxTextLength = 4), g))
        assertNull(role(field(hint = "Conta", hints = listOf("username")), g))
    }

    @Test
    fun itausFormGivesNeitherBoxAUsername() {
        val attrs = arrayOf("type" to "text", "inputmode" to "numeric", "autocomplete" to "off")
        val fields = listOf(
            box("Agência", "id" to "idl-login-sensibility-input-agency", "maxlength" to "4", *attrs),
            box("Conta", "id" to "idl-login-sensibility-input-account", "maxlength" to "7", *attrs),
        )
        assertEquals(listOf(BankRole.BRANCH, BankRole.ACCOUNT), roles(fields))
        assertTrue(loginRolesOf(fields).none { it.role == FieldRole.USERNAME })
    }

    @Test
    fun bradescosDigitBoxAndFactsPerFrame() {
        // Android sees no forms: the extension's second form is another frame here.
        val fields = listOf(
            box("Agência:", "name" to "AGN", "type" to "number", "maxlength" to "4"),
            box("Conta:", "name" to "CTA", "type" to "number", "maxlength" to "7"),
            box("Dígito:", "name" to "DIGCTA", "type" to "number", "maxlength" to "1"),
            box("Conta", "maxlength" to "7", domain = "other.example"),
        )
        assertEquals(listOf(BankRole.BRANCH, BankRole.ACCOUNT, BankRole.ACCOUNT_DIGIT), roles(fields))
    }

    @Test
    fun aSignInFormSayingContaKeepsItsUsername() {
        val user = box("Usuário ou número da conta", "type" to "text")
        val pw = box("Senha", "type" to "password")
        assertEquals(emptyList<BankRole>(), roles(listOf(user, pw)))
        assertEquals(listOf(user.index), LoginFormFinder.find(listOf(user, pw))?.usernames)
    }

    @Test
    fun junkAutocompleteTokensAreNeutralRealFieldNamesRefuse() {
        for (t in listOf("nope", "false", "none", "chrome-off")) {
            val conta = box("Conta", "autocomplete" to t, "maxlength" to "7")
            assertEquals(t, BankRole.ACCOUNT, role(conta, facts(branch = true)))
        }
        assertNull(role(box("Conta", "autocomplete" to "tel-national", "maxlength" to "7"), facts(branch = true)))
        assertNull(role(box("Conta", "autocomplete" to "one-time-code"), facts(branch = true)))
        val fields = listOf(
            box("Agência", "type" to "text", "maxlength" to "4", "autocomplete" to "nope"),
            box("Conta", "type" to "text", "maxlength" to "7", "autocomplete" to "nope"),
        )
        assertEquals(listOf(BankRole.BRANCH, BankRole.ACCOUNT), roles(fields))
    }

    @Test
    fun aCheckboxBetweenContaAndDigitoBreaksNeitherPath() {
        val fields = listOf(
            box("Agência", "type" to "text", "maxlength" to "4"),
            box("Conta", "type" to "text", "maxlength" to "7"),
            box("Lembrar", "type" to "checkbox"),
            box("Dígito", "type" to "text", "maxlength" to "1"),
            box("Senha", "type" to "password"),
        )
        assertEquals(listOf(BankRole.BRANCH, BankRole.ACCOUNT, BankRole.ACCOUNT_DIGIT), roles(fields))
        val login = requireNotNull(LoginFormFinder.find(fields))
        assertEquals(emptyList<Int>(), login.usernames)
        assertEquals(listOf(fields[4].index), login.passwords)
    }

    @Test
    fun bothPathsAgreeOnTheBradescoForm() {
        val fields = listOf(
            box("Agência:", "type" to "number", "maxlength" to "4"),
            box("Conta:", "type" to "number", "maxlength" to "7"),
            box("Dígito:", "type" to "number", "maxlength" to "1"),
        )
        assertEquals(3, roles(fields).size)
        assertTrue(loginRolesOf(fields).all { it.role == FieldRole.UNKNOWN })
    }

    @Test
    fun theFirstBoxOfASplitOneTimeCodeIsNotADigit() {
        val fields = listOf(
            box("Agência", "type" to "text", "maxlength" to "4"),
            box("Conta", "type" to "text", "maxlength" to "7"),
        ) + List(6) { field(html = mapOf("type" to "text", "maxlength" to "1")) }
        assertEquals(listOf(BankRole.BRANCH, BankRole.ACCOUNT), roles(fields))
    }

    @Test
    fun aFocusedAgenciaOnACheckoutRoutesToTheBankAccount() {
        // TETO's donation page: the identity, a card and a bank account in one form.
        val fields = listOf(
            box("Nome", "name" to "person[name]"),
            box("Número do Cartão", "name" to "person[payment_attributes][card_number]", "type" to "tel"),
            box("Agência", "name" to "person[payment_attributes][bank_agency]").copy(focused = true),
            box("Conta", "name" to "person[payment_attributes][bank_account]"),
        )
        val routed = FormRouter.route(fields) as Routed.Login
        val form = requireNotNull(routed.form)
        assertEquals(listOf(BankRole.BRANCH, BankRole.ACCOUNT), form.bank.map { it.role })
        assertEquals(fields[2].index, form.usernameTarget)
    }

    @Test
    fun aTelAgenciaIsNoIdentityPhone() {
        val fields = listOf(
            box("Agência", "type" to "tel", "maxlength" to "4").copy(focused = true),
            box("Conta", "type" to "tel", "maxlength" to "7"),
        )
        assertNull(IdentityFormFinder.find(fields))
    }
}
