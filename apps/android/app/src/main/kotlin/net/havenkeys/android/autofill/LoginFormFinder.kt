package net.havenkeys.android.autofill

/** The fields of one login, by [FieldFacts.index], in screen order. */
data class LoginForm(
    val usernames: List<Int>,
    val passwords: List<Int>,
    val otps: List<Int>,
    val webDomain: String?,
    val webScheme: String?,
    val focused: Int?,
    /** Agência, conta and their digits: a bank account's boxes, never a login's. */
    val bank: List<BankBox> = emptyList(),
) {
    val focusedOnBank: Boolean get() = bank.any { it.index == focused }

    /**
     * Where a login's username goes: the focused bank box (an old bank login
     * holds the agência or conta, spec 2026-11-03-bank-account §5.4) unless
     * it is a check digit's, else the username field.
     */
    val usernameTarget: Int?
        get() = when (bank.firstOrNull { it.index == focused }?.role) {
            null -> usernames.firstOrNull()
            BankRole.BRANCH_DIGIT, BankRole.ACCOUNT_DIGIT -> null
            else -> focused
        }
}

object LoginFormFinder {
    private val LOGIN_ROLES = setOf(FieldRole.USERNAME, FieldRole.PASSWORD, FieldRole.OTP)
    private val NEW_ROLES = setOf(FieldRole.NEW_PASSWORD, FieldRole.CONFIRM_PASSWORD)
    private val SIGN_IN = setOf(FieldRole.PASSWORD, FieldRole.NEW_PASSWORD, FieldRole.CONFIRM_PASSWORD, FieldRole.OTP)

    // Kotlin-side for now (spec 2026-11-14-android-email-username-focused-field
    // §3.3): what a lone email box is for when it is not a sign-in. The shared
    // negatives' "code" stays out: "we'll email you a code" is a sign-in.
    private val NOT_A_SIGN_IN = listOf(
        "newsletter", "subscribe", "subscription", "unsubscribe", "search", "query", "coupon", "promo",
        "comment", "feedback", "message", "invite", "referral", "gift", "buscar", "pesquisar", "cupom",
        "assinar", "inscreva", "inscrever", "novidades", "boletim",
    )

    /**
     * The login of the focused field's group ([FieldGroups.focusedGroup]),
     * [screen] being every field. A group with a password, a code or a bank
     * box is a login on its own. A group with only a username (an email
     * box, often) is a first step only when it has login context
     * (spec 2026-11-14-android-email-username-focused-field §3):
     * - nothing in its frame outside the group signs in, and its words do
     *   not say it is something else (a newsletter, a search, a coupon); or
     * - the whole screen pairs it with a password in another form (a
     *   username form apart from its password form).
     * Otherwise the focused box is not a login's (a newsletter's email next
     * to a sign-in) and there is no login to fill.
     */
    fun find(group: List<FieldFacts>, screen: List<FieldFacts>): LoginForm? {
        val form = find(group)
        val usernameOnly = form != null && form.usernames.isNotEmpty() &&
            form.passwords.isEmpty() && form.otps.isEmpty() && form.bank.isEmpty()
        val focused = group.firstOrNull { it.focused }
        return when {
            !usernameOnly -> form
            focused == null || !signInElsewhere(group, screen, focused) ->
                form.takeUnless { f -> group.any { it.index == f.usernames.first() && saysNotSignIn(it) } }
            else -> find(screen, useFocus = false)
                ?.takeIf { it.usernameTarget == focused.index && it.passwords.isNotEmpty() }
                ?.copy(focused = focused.index)
        }
    }

    /** A password or code field in [focused]'s frame that [group] (a part of [screen]) does not hold. */
    private fun signInElsewhere(group: List<FieldFacts>, screen: List<FieldFacts>, focused: FieldFacts): Boolean {
        if (group.size == screen.size) return false
        val inGroup = group.map { it.index }.toSet()
        val roles = loginRolesOf(screen)
        return screen.indices.any {
            screen[it].index !in inGroup && FieldGroups.sameFrame(screen[it], focused) && roles[it].role in SIGN_IN
        }
    }

    /**
     * Its words say the box is for something other than signing in (a
     * newsletter, a search, a coupon): a username alone in its group is
     * then no first step, however it is typed or hinted (an email box).
     */
    internal fun saysNotSignIn(f: FieldFacts): Boolean = hasAny(f.words.all, NOT_A_SIGN_IN)

    /**
     * Null when there is nothing M1 fills (no field, or only a sign-up form).
     * [useFocus] false: chosen as if nothing were focused.
     */
    fun find(fields: List<FieldFacts>, useFocus: Boolean = true): LoginForm? {
        val bank = BankFieldClassifier.roles(fields)
        val roles = loginRolesOf(fields, bank)
        fun positionsOf(role: FieldRole) = fields.indices.filter { roles[it].role == role }
        fun isBank(i: Int) = fields[i].index in bank
        val usernames = positionsOf(FieldRole.USERNAME)
        val passwords = loginPasswords(roles, usernames.isNotEmpty())
        val focused = fields.indexOfFirst { it.focused }.takeIf { it >= 0 && useFocus }
        val otps = positionsOf(FieldRole.OTP).ifEmpty {
            // A bank's one-character dígito box is no code.
            val start = splitCodeStart(fields, focused?.takeUnless(::isBank))
            listOfNotNull(start.takeIf { usernames.isEmpty() && passwords.isEmpty() })
        }
        val signUp = passwords.isEmpty() && otps.isEmpty() && fields.indices.any { roles[it].role in NEW_ROLES }
        // The field the user focused decides the frame when it is a login or
        // bank field; otherwise the password's, the code's, the username's, then a bank box's.
        val bankAt = fields.indices.filter(::isBank)
        val anchor = focused?.takeIf { roles[it].role in LOGIN_ROLES || isBank(it) }
            ?: listOf(passwords, otps, usernames, bankAt).firstNotNullOfOrNull { it.firstOrNull() }
        if (signUp || anchor == null) return null

        // Only fields in the anchor's frame: a field in another site's
        // iframe is never filled with this site's login.
        val frame = fields[anchor]
        fun inFrame(i: Int) = fields[i].webDomain == frame.webDomain && fields[i].webScheme == frame.webScheme

        val framePasswords = passwords.filter(::inFrame)
        val username =
            usernameOf(usernames.filter(::inFrame), framePasswords.firstOrNull(), focused, fields, roles, bank)

        return LoginForm(
            usernames = listOfNotNull(username).map { fields[it].index },
            passwords = framePasswords.map { fields[it].index },
            otps = otps.filter(::inFrame).map { fields[it].index },
            webDomain = frame.webDomain,
            webScheme = frame.webScheme,
            focused = focused?.takeIf(::inFrame)?.let { fields[it].index },
            bank = bankAt.filter(::inFrame).map { fields[it].index }.map { BankBox(it, bank.getValue(it)) },
        )
    }

    /**
     * The login's password fields. Sites put `new-password` on a login's
     * only password to stop browsers filling it: the only password field,
     * beside a username and with no confirmation, is that login's password.
     */
    private fun loginPasswords(roles: List<Classification>, hasUsername: Boolean): List<Int> {
        fun positionsOf(role: FieldRole) = roles.indices.filter { roles[it].role == role }
        val current = positionsOf(FieldRole.PASSWORD)
        val loneNew = positionsOf(FieldRole.NEW_PASSWORD).singleOrNull()?.takeIf {
            current.isEmpty() && hasUsername && positionsOf(FieldRole.CONFIRM_PASSWORD).isEmpty()
        }
        return current + listOfNotNull(loneNew)
    }

    /**
     * A code split one character per box, entered from the focused box:
     * the first box of its row, which takes the whole code. Some apps (Riot)
     * show Android only the focused box, so one box is enough.
     */
    private fun splitCodeStart(fields: List<FieldFacts>, focused: Int?): Int? {
        if (focused == null || !FieldClassifier.isCodeBox(fields[focused])) return null
        var start = focused
        while (start > 0 && fields[start - 1].let {
                FieldClassifier.isCodeBox(it) && it.webDomain == fields[focused].webDomain
            }
        ) {
            start--
        }
        return start
    }

    /**
     * The username before the password (any, without one): the focused one,
     * else the most confident, nearest the password on a tie; those in the
     * password's group ([FieldFacts.group]) first, so a newsletter's email
     * box is never the login's. Without a classified one, the nearest
     * visible text field before the password when nothing rules it out.
     */
    @Suppress("LongParameterList")
    private fun usernameOf(
        usernames: List<Int>,
        password: Int?,
        focused: Int?,
        fields: List<FieldFacts>,
        roles: List<Classification>,
        bank: Map<Int, BankRole>,
    ): Int? {
        val before = usernames.filter { password == null || it < password }
        val passwordGroup = password?.let { fields[it].group }
        val candidates = before.filter { passwordGroup == null || fields[it].group == passwordGroup }.ifEmpty { before }
        val classified = candidates.firstOrNull { it == focused }
            ?: candidates.maxWithOrNull(compareBy<Int> { roles[it].confidence }.thenBy { it })
        if (classified != null || password == null) return classified
        val frame = fields[password]
        val nearest = (password - 1 downTo 0).firstOrNull {
            val f = fields[it]
            f.webDomain == frame.webDomain && f.webScheme == frame.webScheme &&
                f.visible && f.enabled && FieldClassifier.isTextInput(f) && f.index !in bank
        }
        return nearest?.takeIf {
            roles[it].role == FieldRole.UNKNOWN && FieldClassifier.couldBeUsername(fields[it])
        }
    }
}
