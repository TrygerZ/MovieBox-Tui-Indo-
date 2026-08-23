class MovieboxTui < Formula
  VERSION = "0.1.13"
  MACOS_SHA256 = "8ec5f7aec486fa8b3566fa0bcb160cdb4018218c2eb4427a106a8b3f3ce82d58"
  LINUX_X64_SHA256 = "acf62dc2cc02b8a61eb04c1e7933c6b335a587447808e29ea088cbfd77c52872"
  LINUX_ARM64_SHA256 = "6ad96b3e374c82eacc41a28ca9c42a71a2af2480fa2cc0e4d050946369fa8ece"

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
