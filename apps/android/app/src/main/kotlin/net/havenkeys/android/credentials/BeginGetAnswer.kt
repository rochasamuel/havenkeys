package net.havenkeys.android.credentials

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.credentials.provider.AuthenticationAction
import androidx.credentials.provider.BeginGetCredentialOption
import androidx.credentials.provider.BeginGetCredentialRequest
import androidx.credentials.provider.BeginGetCredentialResponse
import androidx.credentials.provider.BeginGetPasswordOption
import androidx.credentials.provider.BeginGetPublicKeyCredentialOption
import androidx.credentials.provider.CredentialEntry
import androidx.credentials.provider.PasswordCredentialEntry
import androidx.credentials.provider.PublicKeyCredentialEntry
import java.util.concurrent.atomic.AtomicInteger
import net.havenkeys.android.R
import net.havenkeys.android.data.CredentialRepository

/** Each entry needs its own PendingIntent: Android tells them apart by request code. */
private val requestCodes = AtomicInteger()

/**
 * Android's sheet for one Begin request. The extras only name what was
 * listed; the activity they open asks Rust again, for the caller Android
 * attaches then, and Rust refuses anything that does not match it.
 */
@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
internal suspend fun answerBeginGet(
    context: Context,
    request: BeginGetCredentialRequest,
    unlocked: Boolean,
    repo: CredentialRepository,
): BeginGetCredentialResponse {
    val caller = request.callingAppInfo?.toCaller() ?: return BeginGetCredentialResponse()
    val options = request.beginGetCredentialOptions
    val asked = options.mapIndexedNotNull { index, option ->
        when (option) {
            is BeginGetPublicKeyCredentialOption -> Asked.Passkey(index, option.requestJson)
            is BeginGetPasswordOption -> Asked.Password(index)
            else -> null
        }
    }
    val offers = CredentialPlanner.plan(asked, caller, unlocked, repo)
    return if (offers == listOf(CredentialOffer.Unlock)) {
        val unlock = pendingIntent(context, Intent(context, CredentialUnlockActivity::class.java))
        BeginGetCredentialResponse(
            authenticationActions = listOf(AuthenticationAction(context.getString(R.string.autofill_unlock), unlock)),
        )
    } else {
        BeginGetCredentialResponse(credentialEntries = entriesFor(context, offers, options))
    }
}

@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
private fun entriesFor(
    context: Context,
    offers: List<CredentialOffer>,
    options: List<BeginGetCredentialOption>,
): List<CredentialEntry> =
    offers.mapNotNull { offer ->
        when (offer) {
            is CredentialOffer.Passkey -> PublicKeyCredentialEntry(
                context = context,
                username = offer.offer.userName.ifBlank { offer.offer.title },
                pendingIntent = pendingIntent(
                    context,
                    getIntent(context, CredentialExtras.KIND_PASSKEY, offer.offer.itemId, offer.offer.title)
                        .putExtra(CredentialExtras.CREDENTIAL_ID, offer.offer.credentialId),
                ),
                beginGetPublicKeyCredentialOption = options[offer.index] as BeginGetPublicKeyCredentialOption,
                displayName = offer.offer.title,
            )
            is CredentialOffer.Password -> PasswordCredentialEntry(
                context = context,
                username = offer.match.username.orEmpty(),
                pendingIntent = pendingIntent(
                    context,
                    getIntent(context, CredentialExtras.KIND_PASSWORD, offer.match.id, offer.match.title),
                ),
                beginGetPasswordOption = options[offer.index] as BeginGetPasswordOption,
                displayName = offer.match.title,
            )
            CredentialOffer.Unlock -> null
        }
    }

private fun getIntent(context: Context, kind: String, itemId: String, title: String) =
    Intent(context, CredentialGetActivity::class.java)
        .putExtra(CredentialExtras.KIND, kind)
        .putExtra(CredentialExtras.ITEM_ID, itemId)
        .putExtra(CredentialExtras.TITLE, title)

/**
 * Mutable, as the provider API requires (Android adds the final request),
 * and explicit: only our own non-exported activity can receive it.
 */
internal fun pendingIntent(context: Context, intent: Intent): PendingIntent = PendingIntent.getActivity(
    context,
    requestCodes.incrementAndGet(),
    intent,
    PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
)
