plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.mctx.app"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.mctx.app"
        minSdk = 24
        targetSdk = 34
        versionCode = 3
        versionName = "2.1.5"
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
