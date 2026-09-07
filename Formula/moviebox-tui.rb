class MovieboxTui < Formula
  VERSION = "0.1.18"
  MACOS_SHA256 = "27f0aa8d3d0452343c5fc26431b33c4ba32a7849675acb84c273868c6b7efa25"
  LINUX_X64_SHA256 = "d29d42c2102edd04a8185bb766710992c2aef805502431ed01800ea303e3420c"
  LINUX_ARM64_SHA256 = "8d2bb658dcb21b6503e67321d535cf79579f42480ff20202a86ee91aab55c350"

  desc "Stream movies, shows, anime, and live TV from your terminal"
  homepage "https://github.com/mesamirh/MovieBox-Tui"
  version VERSION
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    url "https://github.com/mesamirh/MovieBox-Tui/releases/download/v#{VERSION}/MovieBox_macOS_Universal.tar.gz"
    sha256 MACOS_SHA256
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/mesamirh/MovieBox-Tui/releases/download/v#{VERSION}/MovieBox_Linux_arm64.tar.gz"
      sha256 LINUX_ARM64_SHA256
    else
      url "https://github.com/mesamirh/MovieBox-Tui/releases/download/v#{VERSION}/MovieBox_Linux_x64.tar.gz"
      sha256 LINUX_X64_SHA256
    end
  end

  def install
    bin.install "moviebox-tui"
  end

  test do
    system "#{bin}/moviebox-tui", "--version"
  end
end
