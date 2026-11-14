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
 *
 * Logins and the identity are found in the focused field's group only
 * ([FieldGroups.focusedGroup]): a screen with a sign-in and a newsletter or
 * search form answers for the form the user is in. Cards keep the whole
 * screen (a checkout's boxes sit in several processor frames).
 */
object FormRouter {
    fun route(fields: List<FieldFacts>): Routed? {
        val card = CardFormFinder.find(fields)
        val group = FieldGroups.focusedGroup(fields)
        val login = LoginFormFinder.find(group, fields)
        val save = SaveFormFinder.find(group, login)
        val identity = if (card == null) identityOf(group, login) else null
        return when {
            card != null -> Routed.Card(card)
            identity != null && !signInOwnsFocus(group, login) && !loneLoginBox(group, identity, login) ->
                Routed.Identity(identity, save)
            login != null || save != null -> Routed.Login(login, save)
            else -> null
        }
    }

    /**
     * The identity group; or, with no login, a focused email box alone
     * (a newsletter's): without login context an email box is the
     * identity's (spec 2026-11-14-android-email-username-focused-field §3).
     */
    private fun identityOf(group: List<FieldFacts>, login: LoginForm?): IdentityForm? =
        IdentityFormFinder.find(group) ?: if (login == null) IdentityFormFinder.find(group, loneEmail = true) else null

    /**
     * A lone identity box the login finder takes as the username: gov.br's
     * one-CPF first step is a sign-in, not a form for the identity.
     */
    private fun loneLoginBox(fields: List<FieldFacts>, identity: IdentityForm, login: LoginForm?): Boolean {
        val focused = fields.firstOrNull { it.focused }?.index
        return identity.fields.size == 1 && login != null && focused in login.usernames
    }

    /** A sign-in (a password or a code) holds the focused field, or nothing is focused. */
    private fun signInOwnsFocus(fields: List<FieldFacts>, login: LoginForm?): Boolean {
        val signIn = login != null && (login.passwords.isNotEmpty() || login.otps.isNotEmpty())
        val focused = fields.firstOrNull { it.focused }?.index
        val loginFields = login?.let { it.usernames + it.passwords + it.otps }.orEmpty()
        return signIn && (focused == null || focused in loginFields)
    }
}
