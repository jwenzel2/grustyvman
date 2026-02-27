use glib::prelude::*;
use glib::subclass::prelude::*;
use glib::Properties;
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::VmObject)]
    pub struct VmObject {
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
        vcpus: RefCell<u32>,
        #[property(get, set)]
        memory_kib: RefCell<u64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VmObject {
        const NAME: &'static str = "GrustyvmanVmObject";
        type Type = super::VmObject;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for VmObject {}
}

glib::wrapper! {
    pub struct VmObject(ObjectSubclass<imp::VmObject>);
}

impl VmObject {
    pub fn new(info: &crate::backend::types::VmInfo) -> Self {
        glib::Object::builder()
            .property("name", &info.name)
            .property("uuid", &info.uuid)
            .property("state", info.state.as_str())
            .property("state-css", info.state.css_class())
            .property("subtitle", &info.subtitle())
            .property("vcpus", info.vcpus)
            .property("memory-kib", info.memory_kib)
            .build()
    }

    pub fn update_from(&self, info: &crate::backend::types::VmInfo) {
        self.set_name(info.name.clone());
        self.set_state(info.state.as_str().to_string());
        self.set_state_css(info.state.css_class().to_string());
        self.set_subtitle(info.subtitle());
        self.set_vcpus(info.vcpus);
        self.set_memory_kib(info.memory_kib);
    }
}
