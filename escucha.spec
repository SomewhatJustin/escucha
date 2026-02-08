Name:           escucha
Version:        0.2.0
Release:        1%{?dist}
Summary:        Hold-to-talk speech-to-text for Linux

License:        MIT
URL:            https://github.com/SomewhatJustin/escucha
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  rust >= 1.75
BuildRequires:  cargo
BuildRequires:  gtk4-devel
BuildRequires:  libadwaita-devel

Requires:       alsa-utils
Requires:       wl-clipboard
Requires:       ydotool
Requires:       gtk4
Requires:       libadwaita

%description
Hold a key, speak, release. Escucha transcribes locally with Whisper.cpp
and pastes into the focused field. Works on both X11 and Wayland.

%prep
%autosetup

%build
cargo build --release

%install
install -Dm755 target/release/escucha %{buildroot}%{_bindir}/escucha
install -Dm644 systemd/escucha.service %{buildroot}%{_userunitdir}/escucha.service

%files
%license LICENSE
%doc README.md
%{_bindir}/escucha
%{_userunitdir}/escucha.service

%changelog
* Fri Feb 07 2025 Justin <justin@example.com> - 0.2.0-1
- Initial Rust rewrite
- Add GTK4 GUI with permission fixes
- Add preflight environment checks
