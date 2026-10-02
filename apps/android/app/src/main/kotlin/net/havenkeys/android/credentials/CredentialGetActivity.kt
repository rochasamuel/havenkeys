package net.havenkeys.android.credentials

import android.os.Build
import android.os.Bundle
import androidx.annotation.RequiresApi
import androidx.fragment.app.FragmentActivity
import net.havenkeys.android.autofill.cancel
import net.havenkeys.android.security.hardenWindow

/** Stub until its task replaces it. */
@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
class CredentialGetActivity : FragmentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        hardenWindow()
        cancel()
    }
}
