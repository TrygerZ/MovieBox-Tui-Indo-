class MovieboxTui < Formula
  VERSION = "0.1.13"
  MACOS_SHA256 = "33fd8177fcfb244df3971287b0aa9c584a302493ef7b11eee8d6f4de607614db"
  LINUX_X64_SHA256 = "7caa0b96e91a060d9677135de742e47a26a6b5e58547ce8ebfd25882a1520beb"
  LINUX_ARM64_SHA256 = "e007faf963de0408c62f12900bf704b26c2ba49be2e3ac081697490939267c84"

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
