package net.havenkeys.android.autofill

import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import android.os.Build
import java.security.MessageDigest

/**
 * The caller's signing certificates, as Android reports them. Any failure
 * (the package is not visible to us, or the lookup fails) gives no
 * certificates, and Rust refuses a target without one.
 */
class CallerIdentity(private val pm: PackageManager) {
    @Suppress("SwallowedException", "TooGenericExceptionCaught")
    fun certDigests(packageName: String): List<ByteArray> = try {
        packageInfo(packageName).signingInfo?.apkContentsSigners.orEmpty().map {
            MessageDigest.getInstance("SHA-256").digest(it.toByteArray())
        }
    } catch (e: PackageManager.NameNotFoundException) {
        emptyList()
    } catch (e: RuntimeException) {
        emptyList()
    }

    private fun packageInfo(packageName: String): PackageInfo =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            pm.getPackageInfo(
                packageName,
                PackageManager.PackageInfoFlags.of(PackageManager.GET_SIGNING_CERTIFICATES.toLong()),
            )
        } else {
            pm.getPackageInfo(packageName, PackageManager.GET_SIGNING_CERTIFICATES)
        }
}
