# Maintainer: Justin <justin@example.com>
pkgname=escucha
pkgver=0.1.0
pkgrel=2
pkgdesc="Hold-to-talk speech-to-text for Linux"
arch=('x86_64')
url="https://github.com/SomewhatJustin/escucha"
license=('MIT')
depends=('alsa-utils' 'qt6-base' 'qt6-declarative')
optdepends=(
    'ydotool: Wayland/KDE automatic pasting'
    'wl-clipboard: Wayland clipboard support'
    'wtype: Wayland virtual-keyboard paste support'
    'xdotool: X11 paste/key simulation'
    'xclip: X11 clipboard support'
    'curl: model download on first run'
)
makedepends=('cargo' 'clang' 'cmake' 'git' 'pkgconf' 'qt6-base' 'qt6-declarative' 'qt6-tools' 'rust')
source=(
    "$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz"
    "io.github.escucha.desktop"
)
sha256sums=('SKIP'
            '803d4f1d1075d8ffe8cdf87c22454115f82042866daa0905396d03219b541829')

build() {
    cd "$pkgname-$pkgver"
    cargo build --release --locked
}

check() {
    cd "$pkgname-$pkgver"
    cargo test --release --locked
}

package() {
    cd "$pkgname-$pkgver"
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
    install -Dm644 "systemd/$pkgname.service" "$pkgdir/usr/lib/systemd/user/$pkgname.service"
    install -Dm644 "$srcdir/io.github.escucha.desktop" \
        "$pkgdir/usr/share/applications/io.github.escucha.desktop"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
