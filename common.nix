rec {
  pkgsSrc = builtins.fetchTarball {
    # The following is for nixos-unstable on 2022-11-13
    # Note: this has a patch fixing android emulator, see https://github.com/NixOS/nixpkgs/pull/202088
    url = "https://github.com/Ekleog/nixpkgs/archive/d0e691c9fee72b55fa4ecfe88e72cdd696e08100.tar.gz";
    sha256 = "1q5cs9r82j0i8v2swryzad9nlvxv3k6bw2d6p6dblbqgikcp07if";
  };
  fenixOverlaySrc = builtins.fetchTarball {
    # The following is the latest version as of 2022-11-15
    url = "https://github.com/nix-community/fenix/archive/65fcbcc6dce1feb9c3f6f53bd1ce63c9976791cc.tar.gz";
    sha256 = "1w071s91whwhb5a15dg1plm4829xmf50g40fp1m0607lhvkw2wcx";
  };
  fenixOverlay = import (fenixOverlaySrc + "/overlay.nix");
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
