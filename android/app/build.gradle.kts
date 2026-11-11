plugins {
    id("com.android.application")
}

// The Android app is the very same web deck as the GitHub Pages site:
// docs/ (HTML + JS + the Rust DSP core compiled to WebAssembly) is bundled
// as assets and served from a secure https://appassets.androidplatform.net origin.
val bundleWeb by tasks.registering(Copy::class) {
    from(rootProject.file("../docs"))
    into(layout.buildDirectory.dir("generated/web/www"))
}

android {
    namespace = "com.sloev.spiralvision"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.sloev.spiralvision"
        minSdk = 26
        targetSdk = 35
        versionCode = (System.getenv("VERSION_CODE") ?: "1").toInt()
        versionName = System.getenv("VERSION_NAME") ?: "0.2.0"
    }

    signingConfigs {
        create("release") {
            // scripts/package-android.sh always provides a keystore
            // (your release key from secrets, or a throwaway key).
            val ks = System.getenv("ANDROID_KEYSTORE")
            if (ks != null) {
                storeFile = file(ks)
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS")
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            if (System.getenv("ANDROID_KEYSTORE") != null) signingConfig = signingConfigs.getByName("release")
        }
    }

    sourceSets["main"].assets.srcDir(layout.buildDirectory.dir("generated/web"))

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

tasks.named("preBuild") { dependsOn(bundleWeb) }

dependencies {
    implementation("androidx.webkit:webkit:1.12.1")
}
