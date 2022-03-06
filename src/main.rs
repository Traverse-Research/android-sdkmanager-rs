use android_sdkmanager::{download_and_extract_packages, HostOs, MatchType};
use anyhow::Result;

const HELP: &str = "\
android-sdkmanager-rs is a lightweight android sdk and ndk package installer.

USAGE:
  cargo android-sdkmanager [OPTIONS] [INPUT]
FLAGS:
  -h, --help            Prints help information
OPTIONS:
  --sdk_path PATH       Sets the SDK install path
ARGS:
  <INPUT>               A list of all android packages, if left empty we'll install the packages required for cargo-apk to work
";

#[derive(Debug)]
struct AppArgs {
    sdk_path: String,
    packages: Vec<String>,
}

fn parse_args() -> Result<AppArgs, pico_args::Error> {
    let mut args: Vec<_> = std::env::args_os().collect();
    args.remove(0);

    // when invoked as cargo tool
    if args[0] == "android-sdkmanager" {
        args.remove(0);
    }

    let mut pargs = pico_args::Arguments::from_vec(args);

    // Help has a higher priority and should be handled separately.
    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP);
        std::process::exit(0);
    }

    let mut args = AppArgs {
        sdk_path: pargs.value_from_str("--sdk_path")?,
        packages: vec![],
    };

    let packages = pargs.finish();
    args.packages = packages
        .iter()
        .map(|os_str| os_str.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    Ok(args)
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let full = true;

    let install_dir = &args.sdk_path;

    let _ = std::fs::remove_dir_all(install_dir);
    let _ = std::fs::create_dir_all(install_dir);

    let packages_borrowed = args.packages.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let packages_borrowed = if packages_borrowed.is_empty() {
        &[
            "ndk;23.1.7779620",
            "platforms;android-31",
            "build-tools;31.0.0",
            "platform-tools",
        ]
    } else {
        packages_borrowed.as_slice()
    };

    #[cfg(target_os = "windows")]
    let host_os = HostOs::Windows;
    #[cfg(target_os = "linux")]
    let host_os = HostOs::Linux;
    #[cfg(target_os = "macos")]
    let host_os = HostOs::MacOs;

    download_and_extract_packages(
        install_dir,
        host_os,
        packages_borrowed,
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

    Ok(())
}

/*
'lib/arm64-v8a/libc++_shared.so'...
'lib/arm64-v8a/libVkLayer_khronos_validation.so'...
*/

// - add aarch64-linux-android to rust-toolchain.toml
// - ANDROID_SDK_ROOT and ANDROID_NDK_ROOT in .config/cargo.toml
// - automatically install cargo-apk
