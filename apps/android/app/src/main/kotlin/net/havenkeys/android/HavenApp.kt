package net.havenkeys.android

import android.app.Application
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import net.havenkeys.android.data.NativeTls
import net.havenkeys.android.security.SecretKeyCipher

class HavenApp : Application() {
    lateinit var container: AppContainer
        private set

    val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private var foregroundSync: Job? = null

    override fun onCreate() {
        super.onCreate()
        NativeTls.init(this)
        container = AppContainer(this, SecretKeyCipher())
        registerReceiver(
            object : BroadcastReceiver() {
                override fun onReceive(context: Context, intent: Intent) {
                    container.vaultRepository.screenTurnedOff()
                }
            },
            IntentFilter(Intent.ACTION_SCREEN_OFF),
        )
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) {
                container.vaultRepository.tick()
                foregroundSync = appScope.launch {
                    while (isActive) {
                        container.accountRepository.syncIfDue()
                        delay(SYNC_CHECK_MS)
                    }
                }
            }

            override fun onStop(owner: LifecycleOwner) {
                foregroundSync?.cancel()
            }
        })
    }

    private companion object {
        const val SYNC_CHECK_MS = 30_000L
    }
}
