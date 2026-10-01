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
        val passwords = positionsOf(FieldRole.PASSWORD)
        val otps = positionsOf(FieldRole.OTP)
        val usernames = positionsOf(FieldRole.USERNAME)
        val signUp = passwords.isEmpty() && otps.isEmpty() && fields.indices.any { roles[it].role in NEW_ROLES }
        val focused = fields.indexOfFirst { it.focused }.takeIf { it >= 0 }
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
