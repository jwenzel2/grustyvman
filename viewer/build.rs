fn main() {
    // Rerun whenever the C helper changes.
    println!("cargo:rerun-if-changed=src/spice_helpers.c");

    // Probe but suppress automatic cargo metadata output — we'll emit it ourselves
    // *after* compiling the static C helper, so the dynamic libs appear after the
    // static archive in the linker command.  On Fedora/RHEL with --as-needed this
    // is required: the linker only pulls in a DSO when it first encounters a
    // reference to it, so DSOs must come *after* the object that needs them.
    let spice_gtk = pkg_config::Config::new()
        .cargo_metadata(false)
        .probe("spice-client-gtk-3.0")
        .expect(
            "spice-client-gtk-3.0 not found.\n\
             Install it with:  dnf install spice-gtk3-devel\n\
             or:               apt install libspice-client-gtk-3.0-dev",
        );

    // 1. Compile the C helper into a static archive (libspice_helpers.a).
    let mut build = cc::Build::new();
    build.file("src/spice_helpers.c");
    for path in &spice_gtk.include_paths {
        build.include(path);
    }
    build.compile("spice_helpers");
    // cc::Build::compile() emits:
    //   cargo:rustc-link-lib=static=spice_helpers
    //   cargo:rustc-link-search=native=<out_dir>
    // These appear at the current position in the build-script output order.

    // 2. Now emit the dynamic libraries so they appear *after* the static archive.
    //    The linker resolves undefined symbols in spice_helpers.a against these DSOs.
    for path in &spice_gtk.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
    for lib in &spice_gtk.libs {
        println!("cargo:rustc-link-lib={lib}");
    }
}
