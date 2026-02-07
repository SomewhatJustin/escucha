# Maintainer: Justin <justin@example.com>
pkgname=escucha
pkgver=0.2.0
pkgrel=1
pkgdesc="Hold-to-talk speech-to-text for Linux"
arch=('x86_64')
url="https://github.com/SomewhatJustin/escucha"
license=('MIT')
depends=('alsa-utils' 'wl-clipboard' 'ydotool' 'gtk4' 'libadwaita')
makedepends=('rust' 'cargo')
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

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
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
