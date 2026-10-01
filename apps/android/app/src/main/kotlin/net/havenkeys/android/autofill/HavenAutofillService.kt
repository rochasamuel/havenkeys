package net.havenkeys.android.autofill

import android.os.Build
import android.os.CancellationSignal
import android.service.autofill.AutofillService
import android.service.autofill.FillCallback
import android.service.autofill.FillRequest
import android.service.autofill.FillResponse
import android.service.autofill.SaveCallback
import android.service.autofill.SaveRequest
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.havenkeys.android.HavenApp
import uniffi.havenkeys_mobile.TargetFacts

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
            val response = withContext(Dispatchers.Default) {
                val screen = StructureParser().parse(structure)
                val form = LoginFormFinder.find(screen.fields)
                // No activity component: nobody to check the caller against.
                if (form == null || screen.packageName.isEmpty()) return@withContext null
                val target = TargetFacts(
                    screen.packageName,
                    CallerIdentity(packageManager).certDigests(screen.packageName),
                    form.webDomain,
                    form.webScheme,
                )
                val plan = FillPlanner.plan(form, target, container.events.unlocked.value, container.autofillRepository)
                DatasetFactory(this@HavenAutofillService, screen, form, inlineRequest).response(plan)
            }
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

    /** Saving logins comes in M2: no response asks for a save, so this is never expected. */
    override fun onSaveRequest(request: SaveRequest, callback: SaveCallback) {
        callback.onFailure(null)
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private companion object {
        // Android waits about five seconds for a fill response.
        const val FILL_BUDGET_MS = 4_000L
    }
}
