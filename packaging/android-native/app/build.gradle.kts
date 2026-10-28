// Native (egui) GridCraft host. Kotlin only wraps the activity (MainActivity.kt). The Rust library is built by
// build-native.sh into app/src/main/jniLibs/<abi>/libgridcraft_android.so.
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "ai.storyteller.gridcraft.nativeapp"
    compileSdk = 35
    ndkVersion = "27.2.12479018"

    defaultConfig {
        applicationId = "ai.storyteller.gridcraft.native"
        minSdk = 29
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    implementation("androidx.games:games-activity:4.0.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
}

// games-activity pulls in kotlin-stdlib-jdk7/jdk8 1.6.21 beside stdlib 1.8.22; the duplicates break the build.
configurations.all {
    exclude(group = "org.jetbrains.kotlin", module = "kotlin-stdlib-jdk7")
    exclude(group = "org.jetbrains.kotlin", module = "kotlin-stdlib-jdk8")
}
