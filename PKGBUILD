# Maintainer: Justin <justin@example.com>
pkgname=escucha
pkgver=0.2.7
pkgrel=2
pkgdesc="Hold-to-talk speech-to-text for Linux"
arch=('x86_64')
url="https://github.com/SomewhatJustin/escucha"
license=('MIT')
depends=('alsa-utils' 'qt6-base' 'qt6-declarative' 'ydotool' 'wl-clipboard')
optdepends=(
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
            '393c5f8f95a744356e12c99111ef294e441214c2114697af58460f0f37d6bf6a')

build() {
    cd "$srcdir/$pkgname-$pkgver" || return 1
    cargo clean
    # Arch hardening/linker flags can break cxx-qt/whisper static native links.
    unset CFLAGS CXXFLAGS CPPFLAGS LDFLAGS RUSTFLAGS CARGO_ENCODED_RUSTFLAGS
    cargo build --release --locked
}

check() {
    cd "$srcdir/$pkgname-$pkgver" || return 1
    unset CFLAGS CXXFLAGS CPPFLAGS LDFLAGS RUSTFLAGS CARGO_ENCODED_RUSTFLAGS
    cargo test --release --locked
}

package() {
    cd "$srcdir/$pkgname-$pkgver" || return 1
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
    install -Dm644 "systemd/$pkgname.service" "$pkgdir/usr/lib/systemd/user/$pkgname.service"
    install -Dm644 "systemd/ydotoold.service" "$pkgdir/usr/lib/systemd/user/ydotoold.service"
    install -Dm644 "assets/io.github.escucha.svg" \
        "$pkgdir/usr/share/icons/hicolor/scalable/apps/io.github.escucha.svg"
    install -Dm644 "$srcdir/io.github.escucha.desktop" \
        "$pkgdir/usr/share/applications/io.github.escucha.desktop"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
