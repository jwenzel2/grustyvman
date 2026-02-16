use glib::prelude::*;
use glib::subclass::prelude::*;
use glib::Properties;
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::NetworkObject)]
    pub struct NetworkObject {
        #[property(get, set)]
        name: RefCell<String>,
        #[property(get, set)]
        uuid: RefCell<String>,
        #[property(get, set)]
        state: RefCell<String>,
        #[property(get, set)]
        state_css: RefCell<String>,
        #[property(get, set)]
        subtitle: RefCell<String>,
        #[property(get, set)]
        active: RefCell<bool>,
        #[property(get, set)]
        autostart: RefCell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NetworkObject {
        const NAME: &'static str = "GrustyvmanNetworkObject";
        type Type = super::NetworkObject;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for NetworkObject {}
}

glib::wrapper! {
    pub struct NetworkObject(ObjectSubclass<imp::NetworkObject>);
}

impl NetworkObject {
    pub fn new(info: &crate::backend::types::VirtNetworkInfo) -> Self {
        glib::Object::builder()
            .property("name", &info.name)
            .property("uuid", &info.uuid)
            .property("state", info.state.label())
            .property("state-css", info.state.css_class())
            .property("subtitle", &info.subtitle())
            .property("active", info.active)
            .property("autostart", info.autostart)
            .build()
    }

    pub fn update_from(&self, info: &crate::backend::types::VirtNetworkInfo) {
        self.set_name(info.name.clone());
        self.set_state(info.state.label().to_string());
        self.set_state_css(info.state.css_class().to_string());
        self.set_subtitle(info.subtitle());
        self.set_active(info.active);
        self.set_autostart(info.autostart);
    }
}
