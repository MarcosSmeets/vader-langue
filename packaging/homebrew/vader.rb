# Fórmula Homebrew (template). Depois de publicar os binários numa release do GitHub,
# preencha os `sha256` (use `shasum -a 256 <arquivo>`) e troque SEU-USUARIO.
# Instalação pelo usuário:  brew install SEU-USUARIO/tap/vader
#   (com um tap próprio: github.com/SEU-USUARIO/homebrew-tap, este arquivo em Formula/)
class Vader < Formula
  desc "The Vader programming language compiler"
  homepage "https://github.com/MarcosSmeets/vader-langue"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/MarcosSmeets/vader-langue/releases/download/v0.1.0/vader-macos-arm64"
      sha256 "PREENCHER_SHA256_MACOS_ARM64"
    end
    on_intel do
      url "https://github.com/MarcosSmeets/vader-langue/releases/download/v0.1.0/vader-macos-x86_64"
      sha256 "PREENCHER_SHA256_MACOS_X86_64"
    end
  end

  on_linux do
    url "https://github.com/MarcosSmeets/vader-langue/releases/download/v0.1.0/vader-linux-x86_64"
    sha256 "PREENCHER_SHA256_LINUX_X86_64"
  end

  def install
    bin.install Dir["vader*"].first => "vader"
  end

  test do
    assert_match "vader", shell_output("#{bin}/vader version")
  end
end
