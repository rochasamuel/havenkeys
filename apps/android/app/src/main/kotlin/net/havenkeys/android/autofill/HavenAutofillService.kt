package net.havenkeys.android.autofill

import android.app.assist.AssistStructure
import android.os.Build
import android.os.CancellationSignal
import android.service.autofill.AutofillService
import android.service.autofill.FillCallback
import android.service.autofill.FillRequest
import android.service.autofill.FillResponse
import android.service.autofill.SaveCallback
import android.service.autofill.SaveRequest
import android.view.inputmethod.InlineSuggestionsRequest
import android.widget.Toast
import androidx.annotation.StringRes
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.havenkeys.android.HavenApp
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import uniffi.havenkeys_mobile.SaveLogin
import uniffi.havenkeys_mobile.SaveResult

/**
 * Android's entry point for filling (spec §7). Parses the screen, then asks
 * Rust what to offer. Using autofill is not using the app: nothing here
 * touches the idle timer.
 */
class HavenAutofillService : AutofillService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val container get() = (application as HavenApp).container

    override fun onFillRequest(request: FillRequest, cancellation: CancellationSignal, callback: FillCallback) {
        val answered = AtomicBoolean(false)
        fun answer(response: FillResponse?) {
            if (answered.compareAndSet(false, true)) callback.onSuccess(response)
        }
        val structure = request.fillContexts.lastOrNull()?.structure ?: return answer(null)
        val inlineRequest =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) request.inlineSuggestionsRequest else null
        val work = scope.launch {
            val response = withContext(Dispatchers.Default) { respond(structure, inlineRequest) }
            answer(response)
        }
        // Rust calls cannot be interrupted, so the deadline answers "nothing"
        // in their place and a late plan is dropped.
        val deadline = scope.launch {
            delay(FILL_BUDGET_MS)
            answer(null)
        }
        work.invokeOnCompletion { deadline.cancel() }
        cancellation.setOnCancelListener {
            answered.set(true)
            work.cancel()
            deadline.cancel()
        }
    }

    private suspend fun respond(structure: AssistStructure, inlineRequest: InlineSuggestionsRequest?): FillResponse? {
        val screen = StructureParser().parse(structure)
        val routed = FormRouter.route(screen.fields)
        // No activity component: nobody to check the caller against.
        if (routed == null || screen.packageName.isEmpty()) return null
        val unlocked = container.events.unlocked.value
        val repo = container.autofillRepository
        return when (routed) {
            is Routed.Login -> {
                val target = packageManager.targetOf(
                    screen,
                    routed.form?.webDomain ?: routed.save?.webDomain,
                    routed.form?.webScheme ?: routed.save?.webScheme,
                )
                val plan = FillPlanner.plan(routed.form, target, unlocked, repo, saveable = routed.save != null)
                DatasetFactory(this, screen, routed.form, routed.save, inlineRequest).response(plan)
            }
            else -> {
                val target = packageManager.targetOf(screen, screen.pageDomain, screen.pageScheme)
                val direct = unlocked && !repo.confirmBeforeFilling()
                val plan = WalletPlanner.plan(routed, target, unlocked, repo, direct)
                WalletDatasets(this, screen, routed, inlineRequest).response(plan)
            }
        }
    }

    /**
     * The user confirmed Android's save sheet. Values are read here, for the
     * saved fields only, and go straight to Rust. Not app use: no `touch()`.
     */
    override fun onSaveRequest(request: SaveRequest, callback: SaveCallback) {
        val answered = AtomicBoolean(false)
        fun answer(message: String?): Boolean {
            if (!answered.compareAndSet(false, true)) return false
            if (message == null) callback.onSuccess() else callback.onFailure(message)
            return true
        }
        val structures = request.fillContexts.map { it.structure }
        val work = scope.launch {
            val outcome = withContext(Dispatchers.Default) {
                val reader = SaveRequestReader(packageManager)
                val login = SaveCollector.collect(reader.read(structures)) ?: return@withContext null
                container.autofillRepository.save(
                    login.target,
                    SaveLogin(login.username, login.password, login.current, reader.appTitle(login.target.packageName)),
                )
            }
            val message = saveMessage(outcome)?.let(::getString)
            // Past the deadline Android no longer shows our answer: say it ourselves.
            if (!answer(message) && message != null) {
                Toast.makeText(applicationContext, message, Toast.LENGTH_LONG).show()
            }
        }
        val deadline = scope.launch {
            delay(SAVE_BUDGET_MS)
            answer(null)
        }
        work.invokeOnCompletion { deadline.cancel() }
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private companion object {
        // Android waits about five seconds for a fill response.
        const val FILL_BUDGET_MS = 4_000L
        const val SAVE_BUDGET_MS = 8_000L
    }
}

/** Null when saved or already saved; otherwise the message Android shows. */
@StringRes
internal fun saveMessage(outcome: Outcome<SaveResult>?): Int? = when (outcome) {
    null, is Outcome.Ok -> null
    is Outcome.Failed -> when (outcome.code) {
        "offline" -> R.string.autofill_save_offline
        "locked" -> R.string.autofill_save_locked
        else -> R.string.autofill_save_failed
    }
}
