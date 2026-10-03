plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.detekt)
}

// Release signing comes only from the environment (the release workflow's
// secrets, or a local export). Without all four values the release build
// stays unsigned; it is never signed with the debug key.
val releaseSigning: Map<String, String>? = listOf(
    "HAVENKEYS_KEYSTORE_FILE",
    "HAVENKEYS_KEYSTORE_PASSWORD",
    "HAVENKEYS_KEY_ALIAS",
    "HAVENKEYS_KEY_PASSWORD",
).associateWith { providers.environmentVariable(it).orNull.orEmpty() }
    .takeIf { values -> values.values.all { it.isNotEmpty() } }

android {
    namespace = "net.havenkeys.android"
    compileSdk = 36
    // The NDK that strips the Rust library; keep in step with CI (android.yml).
    ndkVersion = "30.0.16248370"

    defaultConfig {
        applicationId = "net.havenkeys.android"
        minSdk = 28
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    flavorDimensions += "distribution"
    productFlavors {
        create("github") { dimension = "distribution" }
        create("play") { dimension = "distribution" }
    }

    signingConfigs {
        if (releaseSigning != null) {
            create("release") {
                storeFile = file(releaseSigning.getValue("HAVENKEYS_KEYSTORE_FILE"))
                storePassword = releaseSigning.getValue("HAVENKEYS_KEYSTORE_PASSWORD")
                keyAlias = releaseSigning.getValue("HAVENKEYS_KEY_ALIAS")
                keyPassword = releaseSigning.getValue("HAVENKEYS_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.findByName("release")
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures { compose = true }
    packaging { jniLibs { useLegacyPackaging = false } }

    testOptions {
        // The ui/kit component tests run on the JVM under Robolectric and
        // need the merged resources (strings, fonts).
        unitTests {
            isIncludeAndroidResources = true
            // -PscreensDir=.impeccable/review renders the kit catalogue to PNGs
            // (CatalogueScreenshots); without it that test skips itself.
            val screensDir = providers.gradleProperty("screensDir").orNull
            all { test ->
                if (screensDir != null) {
                    test.systemProperty("havenkeys.screens.dir", rootProject.file(screensDir).absolutePath)
                    test.outputs.upToDateWhen { false }
                }
            }
        }
    }
}

kotlin { jvmToolchain(17) }

// The Kotlin half of rustls-platform-verifier must be the version of the
// Rust crate in Cargo.lock, or the JNI calls between them may break.
val rustlsPlatformVerifierVersion: String = providers
    .fileContents(rootProject.layout.projectDirectory.file("../../Cargo.lock"))
    .asText
    .map { lock ->
        val versions = Regex("""name = "rustls-platform-verifier-android"\s+version = "([^"]+)"""")
            .findAll(lock).map { it.groupValues[1] }.toList()
        versions.singleOrNull()
            ?: error("expected one rustls-platform-verifier-android in Cargo.lock, found ${versions.size}")
    }
    .get()

detekt {
    buildUponDefaultConfig = true
    config.setFrom(files("detekt.yml"))
    source.setFrom("src/main/kotlin", "src/test/kotlin", "src/debug/kotlin", "src/testDebug/kotlin")
}

tasks.withType<io.gitlab.arturbosch.detekt.Detekt>().configureEach {
    // The generated bindings are not ours to lint.
    exclude("**/uniffi/**")
}

// detekt's ForbiddenMethodCall needs type resolution, which detekt 1.23 has
// no task for under AGP's built-in Kotlin; this text scan enforces the rule.
val forbidLogging by tasks.registering {
    description = "Fails on any logging call: HavenKeys never logs (CLAUDE.md §40)."
    val sources = fileTree("src") {
        include("**/*.kt")
        exclude("**/uniffi/**")
    }
    inputs.files(sources)
    doLast {
        val forbidden = Regex(
            """\bandroid\.util\.Log\b|\bLog\.(v|d|i|w|e|wtf|println)\s*\(|\bprintln\s*\(|\bprint\s*\(|""" +
                """\.printStackTrace\s*\(|\bSystem\.(out|err)\b""",
        )
        val hits = sources.flatMap { file ->
            file.readLines().mapIndexedNotNull { i, line ->
                if (forbidden.containsMatchIn(line)) "${file.path}:${i + 1}" else null
            }
        }
        if (hits.isNotEmpty()) {
            throw GradleException("Logging is forbidden (CLAUDE.md §40):\n" + hits.joinToString("\n"))
        }
    }
}
// Spec 2026-10-03 §5: the theme, the kit and the catalogue are built on
// foundation only. Stage 5 widens this to the whole app.
val forbidMaterialInKit by tasks.registering {
    description = "Fails on a Material import in the kit, the theme (but MaterialBridge.kt), the catalogue, " +
        "or the screens built from the kit (shell, Home, Items, search, navigation, Settings tab)."
    val sources = fileTree("src") {
        include(
            "**/ui/kit/**/*.kt",
            "**/ui/theme/**/*.kt",
            "**/catalogue/**/*.kt",
            "**/ui/shell/**/*.kt",
            "**/ui/home/**/*.kt",
            "**/ui/item/**/*.kt",
            "**/ui/edit/**/*.kt",
            "**/ui/generator/**/*.kt",
            "**/ui/items/**/*.kt",
            "**/ui/search/**/*.kt",
            "**/ui/nav/**/*.kt",
            "**/ui/unlock/**/*.kt",
            "**/ui/onboarding/**/*.kt",
            "**/ui/settings/**/*.kt",
            "**/ui/autofillsetup/**/*.kt",
            "**/autofill/**/*.kt",
            "**/credentials/**/*.kt",
            "**/ui/components/ScreenBar.kt",
            "**/ui/components/SecretText.kt",
        )
        exclude("**/ui/theme/MaterialBridge.kt")
    }
    inputs.files(sources)
    doLast {
        val material = Regex("""^\s*import\s+androidx\.compose\.material""")
        val hits = sources.flatMap { file ->
            file.readLines().mapIndexedNotNull { i, line ->
                if (material.containsMatchIn(line)) "${file.path}:${i + 1}" else null
            }
        }
        if (hits.isNotEmpty()) {
            throw GradleException("Material is not used in screens built from the HavenKeys kit (spec 2026-10-03 §5):\n" + hits.joinToString("\n"))
        }
    }
}
tasks.named("detekt") { dependsOn(forbidLogging, forbidMaterialInKit) }

// detekt 1.23 embeds the Kotlin compiler it was built with; the project's
// newer Kotlin must not replace it on detekt's own classpath.
configurations.matching { it.name == "detekt" }.configureEach {
    resolutionStrategy.eachDependency {
        if (requested.group == "org.jetbrains.kotlin") useVersion(io.gitlab.arturbosch.detekt.getSupportedKotlinVersion())
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime)
    implementation(libs.androidx.lifecycle.process)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.fragment)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.material3)
    implementation(libs.compose.icons)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.navigation.compose)
    implementation(libs.biometric)
    implementation(libs.autofill)
    implementation(libs.credentials)
    implementation(libs.camerax.core)
    implementation(libs.camerax.camera2)
    implementation(libs.camerax.lifecycle)
    implementation(libs.camerax.view)
    implementation("${libs.jna.get()}@aar")
    implementation(libs.rustls.platform.verifier) { version { strictly(rustlsPlatformVerifierVersion) } }
    implementation(libs.coroutines.android)
    debugImplementation(libs.compose.ui.tooling)
    debugImplementation(libs.compose.ui.test.manifest)
    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test)
    testImplementation(libs.robolectric)
    androidTestImplementation(platform(libs.compose.bom))
    androidTestImplementation(libs.compose.ui.test)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.junit)
}
