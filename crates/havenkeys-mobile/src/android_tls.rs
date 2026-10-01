//! rustls-platform-verifier needs the app's Context once, before the first
//! TLS connection (spec §6.3). Called from `HavenApp.onCreate` through a
//! plain JNI method, outside UniFFI, because UniFFI has no JNI types.

use jni::errors::LogErrorAndDefault;
use jni::objects::{JClass, JObject};
use jni::EnvUnowned;

#[no_mangle]
pub extern "system" fn Java_net_havenkeys_android_data_NativeTls_init<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    context: JObject<'caller>,
) {
    // A failure leaves the bundled roots in use (transport.rs), which still
    // verify every certificate; only user-installed CAs are missed.
    env.with_env(|env| rustls_platform_verifier::android::init_with_env(env, context))
        .resolve::<LogErrorAndDefault>()
}
