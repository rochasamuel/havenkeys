package net.havenkeys.android.autofill

/** Which kind of fill a request is for. */
sealed interface Routed {
    /** [form] null: nothing to fill, but a form to save (a sign-up). */
    data class Login(val form: LoginForm?, val save: SaveForm?) : Routed

    data class Card(val form: CardForm) : Routed

    /** [save]: a sign-up's new password, saved as a login. */
    data class Identity(val form: IdentityForm, val save: SaveForm?) : Routed
}

/**
 * A request goes to cards when the focused field is a card field; to the
 * identity when the focused field is in an identity group that is not a
 * sign-in (no password or code there); otherwise to logins, as in M1.
 */
object FormRouter {
    fun route(fields: List<FieldFacts>): Routed? {
        val card = CardFormFinder.find(fields)
        val login = LoginFormFinder.find(fields)
        val save = SaveFormFinder.find(fields, login)
        val identity = if (card == null) IdentityFormFinder.find(fields) else null
        return when {
            card != null -> Routed.Card(card)
            identity != null && !signInOwnsFocus(fields, login) -> Routed.Identity(identity, save)
            login != null || save != null -> Routed.Login(login, save)
            else -> null
        }
    }

    /** A sign-in (a password or a code) holds the focused field, or nothing is focused. */
    private fun signInOwnsFocus(fields: List<FieldFacts>, login: LoginForm?): Boolean {
        val signIn = login != null && (login.passwords.isNotEmpty() || login.otps.isNotEmpty())
        val focused = fields.firstOrNull { it.focused }?.index
        val loginFields = login?.let { it.usernames + it.passwords + it.otps }.orEmpty()
        return signIn && (focused == null || focused in loginFields)
    }
}
