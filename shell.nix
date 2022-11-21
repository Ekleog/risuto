let
  pkgs = import ./nix;

  risuto-app-env = pkgs.npmlock2nix.node_modules {
    src = ./risuto-app;
    ELECTRON_SKIP_BINARY_DOWNLOAD = 1;
    ELECTRON_OVERRIDE_DIST_PATH = "${pkgs.electron}/bin";
  };

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
      electron_14
      gradle
      jdk8_headless
      mdbook
      niv
      nodejs
      nodePackages.cordova
      nodePackages.npm
      nodePackages.sass
      openssl
      p7zip
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

  NODE_PATH = "${risuto-app-env}/node_modules";
  ELECTRON_OVERRIDE_DIST_PATH = "${pkgs.electron}/bin";
  USE_SYSTEM_7ZA = "true";

  ANDROID_SDK_ROOT = "${androidPkgs.androidsdk}/libexec/android-sdk";
  GRADLE_OPTS = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${androidPkgs.androidsdk}/libexec/android-sdk/build-tools/${androidBuildToolsVersion}/aapt2";
}
