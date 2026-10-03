package net.havenkeys.android.catalogue

import android.os.Bundle
import android.view.View
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge

/**
 * The kit catalogue (debug builds only). It holds no vault data, so unlike
 * every release activity it leaves FLAG_SECURE off: the catalogue is
 * screenshotted for design review. The kit's own windows (sheet, dialog,
 * menu) still set it, which is why the catalogue also draws them inline.
 */
class KitCatalogueActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.decorView.importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
        window.decorView.filterTouchesWhenObscured = true
        enableEdgeToEdge()
        setContent { KitCatalogue() }
    }
}
