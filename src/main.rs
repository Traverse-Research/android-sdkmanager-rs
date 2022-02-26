use sdkmanager::{download_and_extract_packages, HostOs, MatchType};

fn main() {
    let full = false;
    let install_dir = if full {
        "./vendor-full/breda-android-sdk/"
    } else {
        "./vendor-linux/breda-android-sdk/"
    };

    let _ = std::fs::remove_dir_all(install_dir);
    let _ = std::fs::create_dir_all(install_dir);

    download_and_extract_packages(
        install_dir,
        HostOs::Windows,
        &[
            "ndk;23.1.7779620",
            "platforms;android-31",
            "build-tools;31.0.0",
            "platform-tools",
        ],
        if full {
            None
        } else {
            Some(&[
                MatchType::EntireStem("aapt"),
                MatchType::EntireStem("zipalign"),
                MatchType::EntireStem("apksigner"),
                MatchType::EntireStem("adb"),
                MatchType::EntireName("android.jar"),
                MatchType::EntireName("source.properties"),
                MatchType::EntireName("platforms.mk"),
                MatchType::Partial("clang"),
                MatchType::EntireStem("ar"),
                MatchType::Partial("-ar"),
                MatchType::EntireStem("readelf"),
                // platform specific
                MatchType::EntireName("libwinpthread-1.dll"),
                MatchType::EntireStem("lld"),
                // to build native code
                MatchType::EntireFolder("sysroot"),
                // Test
                MatchType::EntireFolder("cxx-stl"),
                // C:\Users\Jasper\traverse\android-sdkmanager-rs\vendor-full\breda-android-sdk\ndk\23.1.7779620\toolchains\llvm\prebuilt\windows-x86_64\lib64\clang\12.0.8\include\stddef.h
                MatchType::EntireFolder("build-tools"),
                MatchType::EntireFolder("lib64"),
                MatchType::EntireFolder("libc++_shared.so"),
                MatchType::EntireFolder("libVkLayer_khronos_validation.so"),
            ])
        },
    );
}

/*
'lib/arm64-v8a/libc++_shared.so'...
'lib/arm64-v8a/libVkLayer_khronos_validation.so'...
*/

// - add aarch64-linux-android to rust-toolchain.toml
// - ANDROID_SDK_ROOT and ANDROID_NDK_ROOT in .config/cargo.toml
// - automatically install cargo-apk
