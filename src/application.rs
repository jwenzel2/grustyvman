use glib::subclass::prelude::*;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;

use crate::config;
use crate::ui::window::Window;

mod imp {
    use super::*;
    use gio::subclass::prelude::ApplicationImpl;
    use gtk::subclass::prelude::GtkApplicationImpl;
    use adw::subclass::prelude::AdwApplicationImpl;

    #[derive(Debug, Default)]
    pub struct GrustyvmanApplication;

    #[glib::object_subclass]
    impl ObjectSubclass for GrustyvmanApplication {
        const NAME: &'static str = "GrustyvmanApplication";
        type Type = super::GrustyvmanApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for GrustyvmanApplication {}

    impl ApplicationImpl for GrustyvmanApplication {
        fn activate(&self) {
            let app = self.obj();
            let window = Window::new(app.upcast_ref());
            window.present();
        }
    }

    impl GtkApplicationImpl for GrustyvmanApplication {}
    impl AdwApplicationImpl for GrustyvmanApplication {}
}

glib::wrapper! {
    pub struct GrustyvmanApplication(ObjectSubclass<imp::GrustyvmanApplication>)
        @extends adw::Application, gtk::Application, gio::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl GrustyvmanApplication {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", config::APP_ID)
            .build()
    }
}
