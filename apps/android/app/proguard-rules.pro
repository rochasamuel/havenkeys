# UniFFI calls into Kotlin through JNA by name.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class uniffi.havenkeys_mobile.** { *; }
# rustls-platform-verifier's Kotlin half is called from Rust over JNI.
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }
# Our own JNI entry point.
-keep class net.havenkeys.android.data.NativeTls { *; }
-dontwarn java.awt.**
