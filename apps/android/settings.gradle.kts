pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // The Kotlin half of rustls-platform-verifier is published only here
        // (its README); nothing else may resolve from this repository. This
        // branch is mutable: gradle/verification-metadata.xml pins the AAR's
        // checksum, and every dependency bump must regenerate that file.
        exclusiveContent {
            forRepository {
                maven("https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/")
            }
            filter { includeModule("org.rustls", "rustls-platform-verifier") }
        }
    }
}

rootProject.name = "HavenKeys"
include(":app")
