with import ./common.nix;

let
  androidBuildToolsVersion = "30.0.3";
  androidPkgs = pkgs.androidenv.composeAndroidPackages {
    buildToolsVersions = [ androidBuildToolsVersion ];
    platformVersions = [ "30" ];

    includeEmulator = true;
    emulatorVersion = "31.3.9";
    includeSystemImages = true;
    abiVersions = [ "x86_64" ];
  };
in
pkgs.stdenv.mkDerivation {
  name = "risuto";
  buildInputs = (
    (with pkgs; [
      androidPkgs.androidsdk
      cacert
      gradle
      jdk8_headless
      mdbook
      nodePackages.cordova
      nodePackages.npm
      nodePackages.sass
      openssl
      pkgconfig
      rust-analyzer-nightly
      sqlx-cli
      trunk

      (fenix.combine (with fenix; [
        minimal.cargo
        minimal.rustc
        targets.wasm32-unknown-unknown.latest.rust-std
      ]))
    ])
  );
  ANDROID_SDK_ROOT = "${androidPkgs.androidsdk}/libexec/android-sdk";
  GRADLE_OPTS = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${androidPkgs.androidsdk}/libexec/android-sdk/build-tools/${androidBuildToolsVersion}/aapt2";
}
