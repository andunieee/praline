import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
}

// Shared signing material lives outside the repository.
val signingProperties = Properties().apply {
    val file = file("/home/andunie/android-release/signing.properties")
    require(file.exists()) { "Missing signing config: ${file.path}" }
    file.inputStream().use { load(it) }
}

android {
    namespace = "dev.caskade.wallet"
    compileSdk {
        version = release(36) {
            minorApiLevel = 1
        }
    }

    defaultConfig {
        applicationId = "dev.caskade.wallet"
        minSdk = 31
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    signingConfigs {
        create("release") {
            storeFile = file(signingProperties.getProperty("storeFile"))
            storePassword = signingProperties.getProperty("storePassword")
            keyAlias = signingProperties.getProperty("keyAlias")
            keyPassword = signingProperties.getProperty("keyPassword")
        }
    }

    buildTypes {
        debug {
            signingConfig = signingConfigs.getByName("release")
        }
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        viewBinding = true
    }
}

tasks.register<Exec>("buildRustLibrary") {
    workingDir("../..")
    val androidHome = System.getenv("ANDROID_HOME") ?: System.getenv("ANDROID_SDK_ROOT") ?: ""
    environment("ANDROID_HOME", androidHome)
    environment("ANDROID_JAR", "$androidHome/platforms/android-36/android.jar")
    commandLine("cargo", "ndk",
        "-o", "android/app/src/main/jniLibs/",
        "-t", "arm64-v8a",
        "-t", "x86_64-linux-android",
        "--platform", "31",
        "build", "--lib", "--release")
}

tasks.named("preBuild") {
    dependsOn("buildRustLibrary")
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.material)
    implementation(libs.androidx.constraintlayout)
    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
    implementation(libs.androidx.startup.runtime)
}
