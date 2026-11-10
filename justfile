# Run the desktop app.
desktop:
    cargo run

# Build the Android app and install it on every connected adb device.
android:
    #!/usr/bin/env bash
    set -euo pipefail

    export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/opt/android-sdk}}"
    export ANDROID_SDK_ROOT="$ANDROID_HOME"
    export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/26.3.11579264}"

    rustup target add aarch64-linux-android x86_64-linux-android

    ./android/gradlew -p android assembleDebug

    apk="android/app/build/outputs/apk/debug/app-debug.apk"
    if [[ ! -f "$apk" ]]; then
        echo "APK not found: $apk" >&2
        exit 1
    fi

    mapfile -t devices < <(adb devices | awk 'NR > 1 && $2 == "device" { print $1 }')
    if (( ${#devices[@]} == 0 )); then
        echo "No connected adb devices." >&2
        exit 1
    fi

    failed=0
    for serial in "${devices[@]}"; do
        echo "Installing on $serial"
        if ! adb -s "$serial" install -r "$apk"; then
            failed=1
        fi
    done
    exit "$failed"
