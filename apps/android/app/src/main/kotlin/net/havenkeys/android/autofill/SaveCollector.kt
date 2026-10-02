package net.havenkeys.android.autofill

import uniffi.havenkeys_mobile.TargetFacts

/** One fill context's form as the user submitted it. Holds typed values: never logged, never kept. */
class SubmittedForm(val target: TargetFacts, val username: String?, val password: String?, val current: String?) {
    override fun toString() = "SubmittedForm(…)"
}

class SubmittedLogin(val target: TargetFacts, val username: String?, val password: String, val current: String?) {
    override fun toString() = "SubmittedLogin(…)"
}

object SaveCollector {
    /**
     * The login a save request carries, from its contexts oldest first: the
     * newest password, and the username from its own screen or, for a
     * sign-in split over two screens, from an earlier one of the same app
     * and site.
     */
    fun collect(forms: List<SubmittedForm>): SubmittedLogin? {
        val at = forms.indexOfLast { !it.password.isNullOrEmpty() }
        if (at < 0) return null
        val last = forms[at]
        val earlier = forms.subList(0, at).lastOrNull {
            sameSite(it.target, last.target) && !it.username.isNullOrBlank()
        }
        val username = last.username?.takeIf { it.isNotBlank() } ?: earlier?.username
        val current = last.current?.takeIf { it.isNotEmpty() }
        return SubmittedLogin(last.target, username, requireNotNull(last.password), current)
    }

    private fun sameSite(a: TargetFacts, b: TargetFacts) =
        a.packageName == b.packageName && a.webDomain == b.webDomain && a.webScheme == b.webScheme
}
