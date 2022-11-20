
let
  sources = import ./sources.nix;
in
rec {
  pkgsSrc = builtins.fetchTarball {
    # The following is for nixos-unstable on 2022-11-13
    # Note: this has a patch fixing android emulator, see https://github.com/NixOS/nixpkgs/pull/202088
    # So we don't use the niv version
    url = "https://github.com/Ekleog/nixpkgs/archive/d0e691c9fee72b55fa4ecfe88e72cdd696e08100.tar.gz";
    sha256 = "1q5cs9r82j0i8v2swryzad9nlvxv3k6bw2d6p6dblbqgikcp07if";
  };
  fenixOverlay = import (sources.fenix + "/overlay.nix");
  pkgs = import pkgsSrc {
    config = {
      allowUnfreePredicate = pkg: pkg.name == "androidsdk";
      android_sdk.accept_license = true;
    };
    overlays = [
      fenixOverlay
    ];
  };
}
