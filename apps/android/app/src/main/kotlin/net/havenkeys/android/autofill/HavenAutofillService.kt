package net.havenkeys.android.autofill

import android.app.assist.AssistStructure
import android.content.Context
import android.os.Build
import android.os.CancellationSignal
import android.service.autofill.AutofillService
import android.service.autofill.FillCallback
import android.service.autofill.FillEventHistory
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
import net.havenkeys.android.data.AutofillRepository
import net.havenkeys.android.data.Outcome
import uniffi.havenkeys_mobile.SaveLogin
import uniffi.havenkeys_mobile.SaveResult

/**
 * Android's entry point for filling (spec §7). Parses the screen, then asks
 * Rust what to offer. Using autofill is not using the app: nothing here
 * touches the idle timer.
 */
class HavenAutofillService : AutofillService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate + swallowUncaught)
    private val picks = DatasetIds.Ledger()
    private val container get() = (application as HavenApp).container

    override fun onFillRequest(request: FillRequest, cancellation: CancellationSignal, callback: FillCallback) {
        val answered = AtomicBoolean(false)
        fun answer(response: FillResponse?) {
            if (answered.compareAndSet(false, true)) {
                if (response != null) picks.responded()
                callback.onSuccess(response)
            }
        }
        val structure = request.fillContexts.lastOrNull()?.structure ?: return answer(null)
        val inlineRequest =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) request.inlineSuggestionsRequest else null
        val work = scope.launch {
            recordPickedRows()
            val response = withContext(Dispatchers.Default) { guarded(null) { respond(structure, inlineRequest) } }
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

    /**
     * Rows picked since the last request: Android reports them here and
     * nowhere else. Direct rows only (they carry an item id); confirmed rows
     * counted in AutofillAuthActivity. Activity is a convenience: nothing
     * here may fail or delay the fill.
     */
    // FillEventHistory is deprecated from API 34 with no replacement.
    @Suppress("DEPRECATION")
    private suspend fun recordPickedRows() = guarded(Unit) {
        val events = fillEventHistory?.events.orEmpty().map { event ->
            val kind = when (event.type) {
                FillEventHistory.Event.TYPE_DATASET_SELECTED -> DatasetIds.Kind.SELECTED
                FillEventHistory.Event.TYPE_DATASET_AUTHENTICATION_SELECTED -> DatasetIds.Kind.AUTHENTICATION_SELECTED
                else -> DatasetIds.Kind.OTHER
            }
            DatasetIds.Picked(kind, event.datasetId)
        }
        val fresh = picks.fresh(events)
        if (container.events.unlocked.value) {
            fresh.forEach { container.autofillRepository.recordUse(it) }
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
            is Routed.Login -> loginResponse(screen, routed.form, routed.save, unlocked, repo, inlineRequest)
            else -> {
                val target = packageManager.targetOf(screen, screen.pageDomain, screen.pageScheme)
                val direct = unlocked && !repo.confirmBeforeFilling()
                val plan = WalletPlanner.plan(routed, target, unlocked, repo, direct)
                val wallet = WalletDatasets(this, screen, routed, inlineRequest).response(plan)
                // A sign-up form still gets "Save password?" when the wallet offers nothing.
                val save = (routed as? Routed.Identity)?.save
                withLoginSaveFallback(wallet, save) { loginResponse(screen, null, it, unlocked, repo, inlineRequest) }
            }
        }
    }

    /**
     * The user confirmed Android's save sheet, for a login or (the response's
     * client state says so) a card. Values are read here, for the saved fields
     * only, and go straight to Rust. Not app use: no `touch()`.
     */
    override fun onSaveRequest(request: SaveRequest, callback: SaveCallback) {
        val answered = AtomicBoolean(false)
        fun answer(message: String?): Boolean {
            if (!answered.compareAndSet(false, true)) return false
            if (message == null) callback.onSuccess() else callback.onFailure(message)
            return true
        }
        val structures = request.fillContexts.map { it.structure }
        val card = request.clientState?.getString(DatasetFactory.EXTRA_SAVE_KIND) == DatasetFactory.SAVE_KIND_CARD
        val work = scope.launch {
            val outcome = withContext(Dispatchers.Default) {
                guarded(Outcome.Failed("internal")) { if (card) saveCard(structures) else saveLogin(structures) }
            }
            val message = saveMessage(outcome, card)?.let(::getString)
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

    private suspend fun saveLogin(structures: List<AssistStructure>): Outcome<SaveResult>? {
        val reader = SaveRequestReader(packageManager)
        val login = SaveCollector.collect(reader.read(structures)) ?: return null
        return container.autofillRepository.save(
            login.target,
            SaveLogin(login.username, login.password, login.current, reader.appTitle(login.target.packageName)),
        )
    }

    /** The card's values are read here, for the card fields only, and go straight to Rust. */
    private suspend fun saveCard(structures: List<AssistStructure>): Outcome<SaveResult>? =
        CardSaveReader(packageManager).read(structures)?.let {
            container.autofillRepository.saveCard(it.target, it.frame, it.card)
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
internal fun saveMessage(outcome: Outcome<SaveResult>?, card: Boolean = false): Int? = when (outcome) {
    null, is Outcome.Ok -> null
    is Outcome.Failed -> when (outcome.code) {
        "offline" -> if (card) R.string.autofill_card_save_offline else R.string.autofill_save_offline
        "locked" -> if (card) R.string.autofill_card_save_locked else R.string.autofill_save_locked
        else -> if (card) R.string.autofill_card_save_failed else R.string.autofill_save_failed
    }
}

/** The wallet's answer, or, when there is none and a login save exists, the login-save answer. */
internal inline fun <T : Any> withLoginSaveFallback(wallet: T?, save: SaveForm?, fallback: (SaveForm) -> T?): T? =
    wallet ?: save?.let(fallback)

/** The M1 login answer: [form]'s rows and, with [save], "Save password?". */
@Suppress("LongParameterList")
internal suspend fun Context.loginResponse(
    screen: ParsedScreen,
    form: LoginForm?,
    save: SaveForm?,
    unlocked: Boolean,
    repo: AutofillRepository,
    inlineRequest: InlineSuggestionsRequest?,
): FillResponse? {
    val target = packageManager.targetOf(screen, form?.webDomain ?: save?.webDomain, form?.webScheme ?: save?.webScheme)
    val plan = FillPlanner.plan(form, target, unlocked, repo, saveable = save != null)
    return DatasetFactory(this, screen, form, save, inlineRequest).response(plan)
}
