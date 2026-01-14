# Maintainer: Your Name <you@example.com>

pkgname=escucha
pkgver=0.1.0
pkgrel=1
pkgdesc="Escucha hold-to-dictate Whisper daemon and GUI"
arch=("any")
url="https://github.com/somewhatjustin/escucha"
license=("MIT")
depends=(
  "python"
  "python-evdev"
  "python-requests"
  "python-faster-whisper"
  "alsa-utils"
)
optdepends=(
  "ydotool: paste on GNOME Wayland"
  "wtype: paste on Wayland"
  "xdotool: paste on X11"
  "wl-clipboard: clipboard paste on Wayland"
  "tk: GUI troubleshooting tool"
)
install="${pkgname}.install"
source=("${pkgname}-${pkgver}.tar.gz::https://github.com/somewhatjustin/escucha/archive/refs/tags/v${pkgver}.tar.gz")
sha256sums=("SKIP")

package() {
  local site_packages
  site_packages=$(python - <<'PY'
import sysconfig
print(sysconfig.get_paths()["purelib"])
PY
  )

  install -Dm755 /dev/stdin "$pkgdir/usr/bin/escucha" <<'PY'
#!/usr/bin/env python
from escucha.app import main

if __name__ == "__main__":
    main()
PY

  install -d "$pkgdir${site_packages}/escucha"
  install -m644 escucha/*.py "$pkgdir${site_packages}/escucha/"

  install -Dm644 systemd/escucha.service \
    "$pkgdir/usr/lib/systemd/user/escucha.service"
  install -Dm644 README.md "$pkgdir/usr/share/doc/${pkgname}/README.md"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/${pkgname}/LICENSE"
}
