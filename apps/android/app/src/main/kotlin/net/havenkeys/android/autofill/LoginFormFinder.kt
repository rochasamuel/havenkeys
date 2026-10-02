package net.havenkeys.android.autofill

/** The fields of one login, by [FieldFacts.index], in screen order. */
data class LoginForm(
    val usernames: List<Int>,
    val passwords: List<Int>,
    val otps: List<Int>,
    val webDomain: String?,
    val webScheme: String?,
    val focused: Int?,
)

object LoginFormFinder {
    private val LOGIN_ROLES = setOf(FieldRole.USERNAME, FieldRole.PASSWORD, FieldRole.OTP)
    private val NEW_ROLES = setOf(FieldRole.NEW_PASSWORD, FieldRole.CONFIRM_PASSWORD)

    /** Null when there is nothing M1 fills (no field, or only a sign-up form). */
    fun find(fields: List<FieldFacts>): LoginForm? {
        val roles = fields.map { FieldClassifier.classify(it) }
        fun positionsOf(role: FieldRole) = fields.indices.filter { roles[it].role == role }
        val usernames = positionsOf(FieldRole.USERNAME)
        val passwords = loginPasswords(roles, usernames.isNotEmpty())
        val focused = fields.indexOfFirst { it.focused }.takeIf { it >= 0 }
        val otps = positionsOf(FieldRole.OTP).ifEmpty {
            listOfNotNull(splitCodeStart(fields, focused).takeIf { usernames.isEmpty() && passwords.isEmpty() })
        }
        val signUp = passwords.isEmpty() && otps.isEmpty() && fields.indices.any { roles[it].role in NEW_ROLES }
        // The field the user focused decides the frame when it is a login
        // field; otherwise the password's, the code's, then the username's.
        val anchor = focused?.takeIf { roles[it].role in LOGIN_ROLES }
            ?: passwords.firstOrNull() ?: otps.firstOrNull() ?: usernames.firstOrNull()
        if (signUp || anchor == null) return null

        // Only fields in the anchor's frame: a field in another site's
        // iframe is never filled with this site's login.
        val frame = fields[anchor]
        fun inFrame(i: Int) = fields[i].webDomain == frame.webDomain && fields[i].webScheme == frame.webScheme

        val framePasswords = passwords.filter(::inFrame)
        val username = usernameOf(usernames.filter(::inFrame), framePasswords.firstOrNull(), focused, fields, roles)

        return LoginForm(
            usernames = listOfNotNull(username).map { fields[it].index },
            passwords = framePasswords.map { fields[it].index },
            otps = otps.filter(::inFrame).map { fields[it].index },
            webDomain = frame.webDomain,
            webScheme = frame.webScheme,
            focused = focused?.takeIf(::inFrame)?.let { fields[it].index },
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
     * else the most confident, nearest the password on a tie. Without a
     * classified one, the nearest visible text field before the password
     * when nothing rules it out.
     */
    private fun usernameOf(
        usernames: List<Int>,
        password: Int?,
        focused: Int?,
        fields: List<FieldFacts>,
        roles: List<Classification>,
    ): Int? {
        val candidates = usernames.filter { password == null || it < password }
        val classified = candidates.firstOrNull { it == focused }
            ?: candidates.maxWithOrNull(compareBy<Int> { roles[it].confidence }.thenBy { it })
        if (classified != null || password == null) return classified
        val frame = fields[password]
        val nearest = (password - 1 downTo 0).firstOrNull {
            val f = fields[it]
            f.webDomain == frame.webDomain && f.webScheme == frame.webScheme &&
                f.visible && f.enabled && FieldClassifier.isTextInput(f)
        }
        return nearest?.takeIf {
            roles[it].role == FieldRole.UNKNOWN && FieldClassifier.couldBeUsername(fields[it])
        }
    }
}
