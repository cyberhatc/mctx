# Homebrew formula for mctx.
#
# Installed via the tap:  brew install cyberhatc/mctx/mctx
# (tap repo: github.com/cyberhatc/homebrew-mctx — this file is its mctx.rb)
class Mctx < Formula
  desc "Terminal notepad for .mctx AI agent memory files"
  homepage "https://github.com/cyberhatc/mctx"
  url "https://github.com/cyberhatc/mctx/archive/refs/tags/v1.1.0.tar.gz"
  # sha256 updated by scripts/update-brew-sha.sh after each tag
  sha256 "PENDING_TAG_SHA"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--root", libexec, "--path", "apps/mctx-notepad"
    bin.install libexec/"bin/mctx"
    man1.install "man/mctx.1"
  end

  test do
    system "#{bin}/mctx", "--help"
  end
end
