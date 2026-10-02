package net.havenkeys.android.autofill

/**
 * The fields a submitted form is saved from, by [FieldFacts.index]: the
 * password to save (a new one when the form sets one), the current password
 * of a change-password form, and the username. A username-only first step
 * has no password; its username waits for the next step.
 */
data class SaveForm(
    val username: Int?,
    val password: Int?,
    val current: Int?,
    val webDomain: String?,
    val webScheme: String?,
)

object SaveFormFinder {
    /**
     * [login] is what [LoginFormFinder] found on the same screen; when there
     * is one, only fields in its frame count.
     */
    fun find(fields: List<FieldFacts>, login: LoginForm?): SaveForm? {
        val roles = fields.map { FieldClassifier.classify(it) }
        fun inFrame(i: Int) = login == null ||
            (fields[i].webDomain == login.webDomain && fields[i].webScheme == login.webScheme)
        val news = fields.indices.filter { roles[it].role == FieldRole.NEW_PASSWORD && inFrame(it) }
        val loginPasswords = login?.passwords.orEmpty()
        // LoginFormFinder already took a lone new-password beside a username as the login's own.
        val loneNewIsLogin = news.size == 1 && fields[news[0]].index in loginPasswords
        return when {
            news.isNotEmpty() && !loneNewIsLogin -> newPassword(fields, roles, news.first())
            loginPasswords.isNotEmpty() -> SaveForm(
                login?.usernames?.firstOrNull(), loginPasswords.first(), null, login?.webDomain, login?.webScheme,
            )
            login != null && login.usernames.isNotEmpty() && login.otps.isEmpty() ->
                SaveForm(login.usernames.first(), null, null, login.webDomain, login.webScheme)
            else -> null
        }
    }

    private fun newPassword(fields: List<FieldFacts>, roles: List<Classification>, at: Int): SaveForm {
        val frame = fields[at]
        fun same(i: Int) = fields[i].webDomain == frame.webDomain && fields[i].webScheme == frame.webScheme
        val current = fields.indices.firstOrNull { same(it) && roles[it].role == FieldRole.PASSWORD }
        val username = fields.indices
            .filter { it < at && same(it) && roles[it].role == FieldRole.USERNAME }
            .maxWithOrNull(compareBy<Int> { roles[it].confidence }.thenBy { it })
        return SaveForm(
            username?.let { fields[it].index },
            frame.index,
            current?.let { fields[it].index },
            frame.webDomain,
            frame.webScheme,
        )
    }
}
