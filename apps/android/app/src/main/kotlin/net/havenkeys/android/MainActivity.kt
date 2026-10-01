package net.havenkeys.android

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.fragment.app.FragmentActivity
import net.havenkeys.android.ui.nav.HavenNavHost
import net.havenkeys.android.ui.theme.HavenTheme

class MainActivity : FragmentActivity() {
    private val container get() = (application as HavenApp).container

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.setFlags(WindowManager.LayoutParams.FLAG_SECURE, WindowManager.LayoutParams.FLAG_SECURE)
        enableEdgeToEdge()
        setContent { HavenTheme { HavenNavHost(container) } }
    }

    override fun onUserInteraction() {
        super.onUserInteraction()
        container.vaultRepository.touch()
    }
}
