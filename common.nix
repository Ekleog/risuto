rec {
  pkgsSrc = builtins.fetchTarball {
    # The following is for nixos-unstable on 2022-11-13
    url = "https://github.com/NixOS/nixpkgs/archive/cd00072eeb6ca71e6f30831385ce9d613508ad1d.tar.gz";
    sha256 = "1hmwmp73hba7kd89wfv3nir3xv760w31z4m31vs1mjinwmb6955v";
  };
  rustOverlaySrc = builtins.fetchTarball {
    # The following is the latest version as of 2022-11-13
    url = "https://github.com/mozilla/nixpkgs-mozilla/archive/80627b282705101e7b38e19ca6e8df105031b072.tar.gz";
    sha256 = "11g9lppm53f5aq7a0fnwh5hivdhn2p1wmhwgmz1052x10hfqjrah";
  };
  rustOverlay = import rustOverlaySrc;
  pkgs = import pkgsSrc {
    overlays = [
      rustOverlay
    ];
  };
  rustNightlyChannel = pkgs.rustChannelOf {
    date = "2022-03-15";
    channel = "nightly";
    sha256 = "0wgn87di2bz901iv2gspg935qgyzc3c2fg5jszckxl4q47jzvd8b";
  };
}
