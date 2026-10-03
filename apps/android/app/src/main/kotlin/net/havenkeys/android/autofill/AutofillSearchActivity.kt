package net.havenkeys.android.autofill

import android.content.pm.PackageManager
import android.os.Bundle
import android.view.View
import android.view.WindowManager
import android.widget.Toast
import androidx.activity.compose.setContent
import androidx.fragment.app.FragmentActivity
import net.havenkeys.android.HavenApp
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.AutofillMatch

/**
 * "Search HavenKeys…" (spec §7.2): the user picks a login for an app that
 * nothing binds yet. Only after they confirm "Use <login> in <app>?" does
 * Rust bind it and fill. Not exported: only our PendingIntent reaches it.
 */
class AutofillSearchActivity : FragmentActivity() {
    private val container get() = (application as HavenApp).container

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.setFlags(WindowManager.LayoutParams.FLAG_SECURE, WindowManager.LayoutParams.FLAG_SECURE)
        // The master password and the Secret Key are typed here: no autofill
        // service may read them or offer to save them (CLAUDE.md §9).
        window.decorView.importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
        // A fill is confirmed here: a tap that passed through another app's
        // overlay is not the user's (tapjacking).
        window.decorView.filterTouchesWhenObscured = true
        val tapped = tappedRequest()?.takeIf { it.routed is Routed.Login } ?: return cancel()
        val packageName = tapped.screen.packageName
        val app = CallerApp(packageName, appLabel(packageName))
        unlockThen(container) {
            setContent {
                HavenTheme {
                    AutofillSearchScreen(
                        search = container.autofillRepository::search,
                        app = app,
                        onConfirmed = { match -> bindAndFill(tapped, match) },
                    )
                }
            }
        }
    }

    // Rust records this pick as a use; recording it here too would count it twice.
    private suspend fun bindAndFill(tapped: TappedRequest, match: AutofillMatch): String? =
        when (val r = container.autofillRepository.bindAndFill(match.id, tapped.target)) {
            is Outcome.Failed -> r.code
            is Outcome.Ok -> {
                val dataset = tapped.factory(this).loginDataset(r.value.values)
                if (dataset != null) {
                    if (!r.value.saved) Toast.makeText(this, R.string.autofill_filled_once, Toast.LENGTH_LONG).show()
                    finishWith(dataset)
                }
                if (dataset == null) NOTHING_TO_FILL else null
            }
        }

    /** Chosen by the app itself, so only ever shown beside its package name. */
    private fun appLabel(packageName: String): String? = try {
        packageManager.getApplicationLabel(packageManager.getApplicationInfo(packageName, 0)).toString()
    } catch (@Suppress("SwallowedException") e: PackageManager.NameNotFoundException) {
        null
    }

    private companion object {
        const val NOTHING_TO_FILL = "not_found"
    }
}
