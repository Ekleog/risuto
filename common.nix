rec {
  pkgsSrc = builtins.fetchTarball {
    # The following is for nixos-unstable on 2022-11-13
    url = "https://github.com/NixOS/nixpkgs/archive/cd00072eeb6ca71e6f30831385ce9d613508ad1d.tar.gz";
    sha256 = "1hmwmp73hba7kd89wfv3nir3xv760w31z4m31vs1mjinwmb6955v";
  };
  fenixOverlaySrc = builtins.fetchTarball {
    # The following is the latest version as of 2022-11-15
    url = "https://github.com/nix-community/fenix/archive/65fcbcc6dce1feb9c3f6f53bd1ce63c9976791cc.tar.gz";
    sha256 = "1w071s91whwhb5a15dg1plm4829xmf50g40fp1m0607lhvkw2wcx";
  };
  fenixOverlay = import (fenixOverlaySrc + "/overlay.nix");
  pkgs = import pkgsSrc {
    overlays = [
      fenixOverlay
    ];
  };
}
