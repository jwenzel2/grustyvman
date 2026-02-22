%global debug_package %{nil}

Name:           grustyvman
Version:        1.0
Release:        1%{?dist}
Summary:        GTK4/Libadwaita virtual machine manager for QEMU/KVM via libvirt

License:        MIT
URL:            https://github.com/jwenzel2/grustyvman
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  rust
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  pkg-config
BuildRequires:  gtk4-devel
BuildRequires:  libadwaita-devel
BuildRequires:  glib2-devel
BuildRequires:  libvirt-devel
BuildRequires:  spice-gtk3-devel

Requires:       gtk4
Requires:       libadwaita
Requires:       libvirt-libs
Requires:       spice-gtk3

%description
Grustyvman is a GTK4/Libadwaita desktop application for managing QEMU/KVM
virtual machines via libvirt. It provides a graphical interface for creating,
configuring, and monitoring virtual machines, with an embedded SPICE console
viewer (grustyvman-viewer) for direct VM access including auto-resize,
clipboard sharing, and key injection.

%prep
%autosetup

%build
cargo build --release --workspace

%install
install -Dm755 target/release/grustyvman \
    %{buildroot}%{_bindir}/grustyvman
install -Dm755 target/release/grustyvman-viewer \
    %{buildroot}%{_bindir}/grustyvman-viewer

install -Dm644 packaging/grustyvman.desktop \
    %{buildroot}%{_datadir}/applications/grustyvman.desktop
install -Dm644 packaging/grustyvman-viewer.desktop \
    %{buildroot}%{_datadir}/applications/grustyvman-viewer.desktop

install -Dm644 icon.png \
    %{buildroot}%{_datadir}/icons/hicolor/1024x1024/apps/grustyvman.png

%post
/bin/touch --no-create %{_datadir}/icons/hicolor &>/dev/null || :
/usr/bin/gtk-update-icon-cache %{_datadir}/icons/hicolor &>/dev/null || :
/usr/bin/update-desktop-database -q %{_datadir}/applications &>/dev/null || :

%postun
if [ $1 -eq 0 ] ; then
    /bin/touch --no-create %{_datadir}/icons/hicolor &>/dev/null || :
    /usr/bin/gtk-update-icon-cache %{_datadir}/icons/hicolor &>/dev/null || :
    /usr/bin/update-desktop-database -q %{_datadir}/applications &>/dev/null || :
fi

%files
%{_bindir}/grustyvman
%{_bindir}/grustyvman-viewer
%{_datadir}/applications/grustyvman.desktop
%{_datadir}/applications/grustyvman-viewer.desktop
%{_datadir}/icons/hicolor/1024x1024/apps/grustyvman.png

%changelog
* %(date "+%a %b %d %Y") Jeremiah Wenzel <jeremiah@grustyvman> - 1.0-1
- Initial release
