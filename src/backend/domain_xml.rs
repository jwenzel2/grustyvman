use crate::backend::types::{
    BootDevice, CpuMode, CpuTune, DiskInfo, DomainDetails, FirmwareType, GraphicsInfo,
    GraphicsType, NetworkInfo, NewDiskParams, NewNetworkParams, SoundInfo, SoundModel, VcpuPin,
    VideoInfo, VideoModel,
};
use crate::error::AppError;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

#[derive(Debug, Clone)]
pub struct NewVmParams {
    pub name: String,
    pub vcpus: u32,
    pub memory_mib: u64,
    pub disk_size_gib: u64,
    pub iso_path: Option<String>,
    pub firmware: FirmwareType,
}

pub fn extract_interface_targets(xml: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut in_devices = false;
    let mut in_interface = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "devices" => in_devices = true,
                    "interface" if in_devices => in_interface = true,
                    "target" if in_interface => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"dev" {
                                let dev = String::from_utf8_lossy(&attr.value).to_string();
                                targets.push(dev);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "devices" => in_devices = false,
                    "interface" => in_interface = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    targets
}

pub fn generate_domain_xml(params: &NewVmParams, disk_path: &str) -> String {
    let memory_kib = params.memory_mib * 1024;

    let os_tag = match params.firmware {
        FirmwareType::Efi => r#"<os firmware="efi">"#,
        FirmwareType::Bios => "<os>",
    };

    let smm_feature = match params.firmware {
        FirmwareType::Efi => "\n    <smm state=\"on\"/>",
        FirmwareType::Bios => "",
    };

    let xml = format!(
        r#"<domain type="kvm">
  <name>{name}</name>
  <memory unit="KiB">{memory_kib}</memory>
  <vcpu placement="static">{vcpus}</vcpu>
  {os_tag}
    <type arch="x86_64" machine="q35">hvm</type>
    <boot dev="hd"/>
{cdrom_boot}  </os>
  <features>
    <acpi/>
    <apic/>{smm_feature}
  </features>
  <cpu mode="host-passthrough"/>
  <devices>
    <emulator>/usr/bin/qemu-system-x86_64</emulator>
    <disk type="file" device="disk">
      <driver name="qemu" type="qcow2"/>
      <source file="{disk_path}"/>
      <target dev="vda" bus="virtio"/>
    </disk>
{cdrom_device}    <interface type="network">
      <source network="default"/>
      <model type="virtio"/>
    </interface>
    <graphics type="spice" autoport="yes"/>
    <video>
      <model type="virtio"/>
    </video>
    <channel type="unix">
      <target type="virtio" name="org.qemu.guest_agent.0"/>
    </channel>
    <input type="tablet" bus="usb"/>
    <console type="pty"/>
  </devices>
</domain>"#,
        name = params.name,
        memory_kib = memory_kib,
        vcpus = params.vcpus,
        os_tag = os_tag,
        smm_feature = smm_feature,
        disk_path = disk_path,
        cdrom_boot = if params.iso_path.is_some() {
            "    <boot dev=\"cdrom\"/>\n"
        } else {
            ""
        },
        cdrom_device = if let Some(ref iso) = params.iso_path {
            format!(
                "    <disk type=\"file\" device=\"cdrom\">\n      <driver name=\"qemu\" type=\"raw\"/>\n      <source file=\"{iso}\"/>\n      <target dev=\"sda\" bus=\"sata\"/>\n      <readonly/>\n    </disk>\n"
            )
        } else {
            String::new()
        },
    );

    xml
}

pub fn create_vm(uri: &str, params: &NewVmParams) -> Result<(), AppError> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let images_dir = format!("{home}/.local/share/libvirt/images");
    std::fs::create_dir_all(&images_dir)?;

    let disk_path = format!("{images_dir}/{}.qcow2", params.name);

    let output = std::process::Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            &disk_path,
            &format!("{}G", params.disk_size_gib),
        ])
        .output()?;

    if !output.status.success() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "qemu-img failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        )));
    }

    let xml = generate_domain_xml(params, &disk_path);

    let conn = virt::connect::Connect::open(Some(uri))?;
    let domain = virt::domain::Domain::define_xml(&conn, &xml)?;
    drop(domain);

    Ok(())
}

pub fn parse_domain_xml(xml: &str) -> Result<DomainDetails, AppError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut details = DomainDetails {
        name: String::new(),
        uuid: String::new(),
        memory_kib: 0,
        vcpus: 0,
        os_type: String::new(),
        arch: String::new(),
        disks: Vec::new(),
        networks: Vec::new(),
        has_graphics: false,
        boot_order: Vec::new(),
        cpu_mode: CpuMode::HostPassthrough,
        cpu_model: None,
        firmware: FirmwareType::Bios,
        graphics: None,
        video: None,
        sound: None,
        cpu_tune: CpuTune::default(),
    };

    #[derive(Debug)]
    enum Context {
        None,
        Name,
        Uuid,
        Memory,
        Vcpu,
        OsType,
        CpuModel,
        Disk(DiskBuilder),
        Interface(InterfaceBuilder),
    }

    #[derive(Debug, Default)]
    struct DiskBuilder {
        target_dev: String,
        source_file: Option<String>,
        bus: String,
        device_type: String,
    }

    #[derive(Debug, Default)]
    struct InterfaceBuilder {
        mac_address: Option<String>,
        source_network: Option<String>,
        model_type: Option<String>,
    }

    let mut context = Context::None;
    let mut in_devices = false;
    let mut in_os = false;
    let mut in_cpu = false;
    let mut in_video = false;
    let mut in_cputune = false;
    let mut os_arch = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "name" if !in_devices => context = Context::Name,
                    "uuid" => context = Context::Uuid,
                    "memory" => context = Context::Memory,
                    "vcpu" => context = Context::Vcpu,
                    "os" => {
                        in_os = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"firmware" {
                                let val = String::from_utf8_lossy(&attr.value).to_string();
                                if val == "efi" {
                                    details.firmware = FirmwareType::Efi;
                                }
                            }
                        }
                    }
                    "loader" if in_os => {
                        // Legacy UEFI detection: <loader type="pflash">/usr/share/OVMF/OVMF_CODE.fd</loader>
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"type" {
                                let val = String::from_utf8_lossy(&attr.value).to_string();
                                if val == "pflash" {
                                    details.firmware = FirmwareType::Efi;
                                }
                            }
                        }
                    }
                    "type" if in_os && matches!(context, Context::None) => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"arch" {
                                os_arch = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        context = Context::OsType;
                    }
                    "boot" if in_os => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"dev" {
                                let dev = String::from_utf8_lossy(&attr.value).to_string();
                                if let Some(bd) = BootDevice::from_str(&dev) {
                                    details.boot_order.push(bd);
                                }
                            }
                        }
                    }
                    "cpu" if !in_devices => {
                        in_cpu = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"mode" {
                                let mode = String::from_utf8_lossy(&attr.value).to_string();
                                details.cpu_mode = CpuMode::from_str(&mode);
                            }
                        }
                    }
                    "model" if in_cpu && !in_devices => {
                        context = Context::CpuModel;
                    }
                    "cputune" => {
                        in_cputune = true;
                    }
                    "vcpupin" if in_cputune => {
                        let mut vcpu = 0u32;
                        let mut cpuset = String::new();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"vcpu" => {
                                    vcpu = String::from_utf8_lossy(&attr.value)
                                        .parse()
                                        .unwrap_or(0);
                                }
                                b"cpuset" => {
                                    cpuset =
                                        String::from_utf8_lossy(&attr.value).to_string();
                                }
                                _ => {}
                            }
                        }
                        if !cpuset.is_empty() {
                            details.cpu_tune.vcpu_pins.push(VcpuPin { vcpu, cpuset });
                        }
                    }
                    "emulatorpin" if in_cputune => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"cpuset" {
                                details.cpu_tune.emulatorpin =
                                    Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }
                    }
                    "devices" => {
                        in_devices = true;
                    }
                    "disk" if in_devices => {
                        let mut db = DiskBuilder::default();
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"device" {
                                db.device_type = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        context = Context::Disk(db);
                    }
                    "source" => {
                        match &mut context {
                            Context::Disk(ref mut db) => {
                                for attr in e.attributes().flatten() {
                                    if attr.key.as_ref() == b"file" {
                                        db.source_file = Some(
                                            String::from_utf8_lossy(&attr.value).to_string(),
                                        );
                                    }
                                }
                            }
                            Context::Interface(ref mut ib) => {
                                for attr in e.attributes().flatten() {
                                    if attr.key.as_ref() == b"network" {
                                        ib.source_network = Some(
                                            String::from_utf8_lossy(&attr.value).to_string(),
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    "target" if matches!(context, Context::Disk(_)) => {
                        if let Context::Disk(ref mut db) = context {
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"dev" => {
                                        db.target_dev =
                                            String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    b"bus" => {
                                        db.bus =
                                            String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    "interface" if in_devices => {
                        context = Context::Interface(InterfaceBuilder::default());
                    }
                    "mac" if matches!(context, Context::Interface(_)) => {
                        if let Context::Interface(ref mut ib) = context {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"address" {
                                    ib.mac_address =
                                        Some(String::from_utf8_lossy(&attr.value).to_string());
                                }
                            }
                        }
                    }
                    "model" if in_video && in_devices => {
                        let mut vtype = VideoModel::None;
                        let mut vram = None;
                        let mut heads = None;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"type" => {
                                    vtype = VideoModel::from_str(
                                        &String::from_utf8_lossy(&attr.value),
                                    );
                                }
                                b"vram" => {
                                    vram = String::from_utf8_lossy(&attr.value)
                                        .parse()
                                        .ok();
                                }
                                b"heads" => {
                                    heads = String::from_utf8_lossy(&attr.value)
                                        .parse()
                                        .ok();
                                }
                                _ => {}
                            }
                        }
                        details.video = Some(VideoInfo {
                            model: vtype,
                            vram,
                            heads,
                        });
                    }
                    "model" if matches!(context, Context::Interface(_)) => {
                        if let Context::Interface(ref mut ib) = context {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"type" {
                                    ib.model_type =
                                        Some(String::from_utf8_lossy(&attr.value).to_string());
                                }
                            }
                        }
                    }
                    "graphics" if in_devices => {
                        details.has_graphics = true;
                        let mut gtype = GraphicsType::None;
                        let mut port = None;
                        let mut autoport = false;
                        let mut listen_address = None;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"type" => {
                                    gtype = GraphicsType::from_str(
                                        &String::from_utf8_lossy(&attr.value),
                                    );
                                }
                                b"port" => {
                                    port = String::from_utf8_lossy(&attr.value)
                                        .parse()
                                        .ok();
                                }
                                b"autoport" => {
                                    autoport = String::from_utf8_lossy(&attr.value) == "yes";
                                }
                                b"listen" => {
                                    listen_address =
                                        Some(String::from_utf8_lossy(&attr.value).to_string());
                                }
                                _ => {}
                            }
                        }
                        details.graphics = Some(GraphicsInfo {
                            graphics_type: gtype,
                            port,
                            autoport,
                            listen_address,
                        });
                    }
                    "video" if in_devices => {
                        in_video = true;
                    }
                    "sound" if in_devices => {
                        let mut smodel = SoundModel::None;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"model" {
                                smodel = SoundModel::from_str(
                                    &String::from_utf8_lossy(&attr.value),
                                );
                            }
                        }
                        details.sound = Some(SoundInfo { model: smodel });
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                match context {
                    Context::Name => {
                        details.name = text;
                        context = Context::None;
                    }
                    Context::Uuid => {
                        details.uuid = text;
                        context = Context::None;
                    }
                    Context::Memory => {
                        details.memory_kib = text.parse().unwrap_or(0);
                        context = Context::None;
                    }
                    Context::Vcpu => {
                        details.vcpus = text.parse().unwrap_or(0);
                        context = Context::None;
                    }
                    Context::OsType => {
                        details.os_type = text;
                        details.arch = os_arch.clone();
                        context = Context::None;
                    }
                    Context::CpuModel => {
                        details.cpu_model = Some(text);
                        context = Context::None;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "os" => {
                        in_os = false;
                    }
                    "cpu" => {
                        in_cpu = false;
                    }
                    "cputune" => {
                        in_cputune = false;
                    }
                    "devices" => {
                        in_devices = false;
                    }
                    "video" => {
                        in_video = false;
                    }
                    "disk" => {
                        if let Context::Disk(db) = std::mem::replace(&mut context, Context::None) {
                            details.disks.push(DiskInfo {
                                target_dev: db.target_dev,
                                source_file: db.source_file,
                                bus: db.bus,
                                device_type: if db.device_type.is_empty() {
                                    "disk".to_string()
                                } else {
                                    db.device_type
                                },
                            });
                        }
                    }
                    "interface" => {
                        if let Context::Interface(ib) =
                            std::mem::replace(&mut context, Context::None)
                        {
                            details.networks.push(NetworkInfo {
                                mac_address: ib.mac_address,
                                source_network: ib.source_network,
                                model_type: ib.model_type,
                            });
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
            _ => {}
        }
    }

    Ok(details)
}

pub fn modify_graphics(xml: &str, graphics_type: GraphicsType) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut in_devices = false;
    let mut skip_graphics = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "devices" => {
                        in_devices = true;
                        result.push('<');
                        write_element(&mut result, e);
                        result.push('>');
                    }
                    "graphics" if in_devices => {
                        skip_graphics = true;
                        // Skip this element and its children
                        let mut depth = 1u32;
                        loop {
                            match reader.read_event() {
                                Ok(Event::Start(_)) => depth += 1,
                                Ok(Event::End(_)) => {
                                    depth -= 1;
                                    if depth == 0 { break; }
                                }
                                Ok(Event::Eof) => break,
                                _ => {}
                            }
                        }
                    }
                    _ => {
                        result.push('<');
                        write_element(&mut result, e);
                        result.push('>');
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "graphics" && in_devices {
                    // Skip existing empty graphics element
                    continue;
                }
                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "devices" {
                    // Insert new graphics before </devices>
                    if graphics_type != GraphicsType::None {
                        result.push_str(&format!(
                            r#"<graphics type="{}" autoport="yes"/>"#,
                            graphics_type.as_str()
                        ));
                    }
                    in_devices = false;
                }
                result.push_str(&format!("</{name}>"));
            }
            Ok(ref event) => {
                copy_event(&mut result, event);
                if matches!(event, Event::Eof) { break; }
            }
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
        }
    }

    let _ = skip_graphics;
    Ok(result)
}

pub fn modify_video(xml: &str, video_model: VideoModel) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut in_devices = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "devices" => {
                        in_devices = true;
                        result.push('<');
                        write_element(&mut result, e);
                        result.push('>');
                    }
                    "video" if in_devices => {
                        // Skip this element and its children
                        let mut depth = 1u32;
                        loop {
                            match reader.read_event() {
                                Ok(Event::Start(_)) => depth += 1,
                                Ok(Event::End(_)) => {
                                    depth -= 1;
                                    if depth == 0 { break; }
                                }
                                Ok(Event::Eof) => break,
                                _ => {}
                            }
                        }
                    }
                    _ => {
                        result.push('<');
                        write_element(&mut result, e);
                        result.push('>');
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "video" && in_devices {
                    continue;
                }
                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "devices" {
                    if video_model != VideoModel::None {
                        result.push_str(&format!(
                            r#"<video><model type="{}"/></video>"#,
                            video_model.as_str()
                        ));
                    }
                    in_devices = false;
                }
                result.push_str(&format!("</{name}>"));
            }
            Ok(ref event) => {
                copy_event(&mut result, event);
                if matches!(event, Event::Eof) { break; }
            }
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
        }
    }

    Ok(result)
}

pub fn modify_sound(xml: &str, sound_model: SoundModel) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut in_devices = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "devices" => {
                        in_devices = true;
                        result.push('<');
                        write_element(&mut result, e);
                        result.push('>');
                    }
                    "sound" if in_devices => {
                        // Skip this element and its children
                        let mut depth = 1u32;
                        loop {
                            match reader.read_event() {
                                Ok(Event::Start(_)) => depth += 1,
                                Ok(Event::End(_)) => {
                                    depth -= 1;
                                    if depth == 0 { break; }
                                }
                                Ok(Event::Eof) => break,
                                _ => {}
                            }
                        }
                    }
                    _ => {
                        result.push('<');
                        write_element(&mut result, e);
                        result.push('>');
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "sound" && in_devices {
                    continue;
                }
                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "devices" {
                    if sound_model != SoundModel::None {
                        result.push_str(&format!(
                            r#"<sound model="{}"/>"#,
                            sound_model.as_str()
                        ));
                    }
                    in_devices = false;
                }
                result.push_str(&format!("</{name}>"));
            }
            Ok(ref event) => {
                copy_event(&mut result, event);
                if matches!(event, Event::Eof) { break; }
            }
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
        }
    }

    Ok(result)
}

pub fn modify_domain_xml(xml: &str, new_vcpus: u32, new_memory_mib: u64) -> Result<String, AppError> {
    let new_memory_kib = new_memory_mib * 1024;
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut skip_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "vcpu" => {
                        result.push('<');
                        write_element(&mut result, e);
                        result.push('>');
                        skip_text = true;
                        result.push_str(&new_vcpus.to_string());
                    }
                    "memory" => {
                        result.push_str("<memory unit=\"KiB\">");
                        skip_text = true;
                        result.push_str(&new_memory_kib.to_string());
                    }
                    "currentMemory" => {
                        result.push_str("<currentMemory unit=\"KiB\">");
                        skip_text = true;
                        result.push_str(&new_memory_kib.to_string());
                    }
                    _ => {
                        result.push('<');
                        write_element(&mut result, e);
                        result.push('>');
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "vcpu" | "memory" | "currentMemory" => {
                        skip_text = false;
                    }
                    _ => {}
                }
                result.push_str(&format!("</{name}>"));
            }
            Ok(Event::Text(ref e)) => {
                if !skip_text {
                    result.push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(Event::Empty(ref e)) => {
                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(Event::Decl(ref e)) => {
                result.push_str("<?");
                result.push_str(&String::from_utf8_lossy(e.as_ref()));
                result.push_str("?>");
            }
            Ok(Event::Comment(ref e)) => {
                result.push_str("<!--");
                result.push_str(&String::from_utf8_lossy(e.as_ref()));
                result.push_str("-->");
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
            _ => {}
        }
    }

    Ok(result)
}

fn write_element(result: &mut String, e: &BytesStart) {
    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
    result.push_str(&name);
    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        let val = String::from_utf8_lossy(&attr.value).to_string();
        result.push_str(&format!(r#" {key}="{val}""#));
    }
}

fn copy_event(result: &mut String, event: &Event) {
    match event {
        Event::Start(ref e) => {
            result.push('<');
            write_element(result, e);
            result.push('>');
        }
        Event::End(ref e) => {
            let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
            result.push_str(&format!("</{name}>"));
        }
        Event::Empty(ref e) => {
            result.push('<');
            write_element(result, e);
            result.push_str("/>");
        }
        Event::Text(ref e) => {
            result.push_str(&e.unescape().unwrap_or_default());
        }
        Event::Decl(ref e) => {
            result.push_str("<?");
            result.push_str(&String::from_utf8_lossy(e.as_ref()));
            result.push_str("?>");
        }
        Event::Comment(ref e) => {
            result.push_str("<!--");
            result.push_str(&String::from_utf8_lossy(e.as_ref()));
            result.push_str("-->");
        }
        _ => {}
    }
}

pub fn modify_cpu_model(
    xml: &str,
    cpu_mode: CpuMode,
    cpu_model: Option<&str>,
) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut in_cpu = false;
    let mut cpu_depth = 0;
    let mut found_cpu = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "cpu" && !in_cpu {
                    in_cpu = true;
                    found_cpu = true;
                    cpu_depth = 1;
                    // Write new cpu element
                    match cpu_mode {
                        CpuMode::Custom => {
                            result.push_str(&format!(
                                r#"<cpu mode="custom" match="exact"><model fallback="forbid">{}</model>"#,
                                cpu_model.unwrap_or("qemu64")
                            ));
                        }
                        _ => {
                            result.push_str(&format!(r#"<cpu mode="{}""#, cpu_mode.as_str()));
                            // Will close with /> if empty or > if has children
                            // For simplicity, skip all children and close
                        }
                    }
                    continue;
                }
                if in_cpu {
                    cpu_depth += 1;
                    // Skip children of old cpu
                    continue;
                }
                result.push('<');
                write_element(&mut result, e);
                result.push('>');
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if in_cpu {
                    cpu_depth -= 1;
                    if cpu_depth == 0 {
                        in_cpu = false;
                        // Close the cpu element we wrote
                        match cpu_mode {
                            CpuMode::Custom => {
                                result.push_str("</cpu>");
                            }
                            _ => {
                                result.push_str("/>");
                            }
                        }
                    }
                    continue;
                }
                result.push_str(&format!("</{name}>"));
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "cpu" && !in_cpu {
                    found_cpu = true;
                    match cpu_mode {
                        CpuMode::Custom => {
                            result.push_str(&format!(
                                r#"<cpu mode="custom" match="exact"><model fallback="forbid">{}</model></cpu>"#,
                                cpu_model.unwrap_or("qemu64")
                            ));
                        }
                        _ => {
                            result.push_str(&format!(r#"<cpu mode="{}"/>"#, cpu_mode.as_str()));
                        }
                    }
                    continue;
                }
                if in_cpu {
                    continue;
                }
                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(ref event @ Event::Text(_)) => {
                if !in_cpu {
                    copy_event(&mut result, event);
                }
            }
            Ok(ref event @ Event::Decl(_)) | Ok(ref event @ Event::Comment(_)) => {
                copy_event(&mut result, event);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
            _ => {}
        }
    }

    if !found_cpu {
        // Insert before </domain>
        if let Some(pos) = result.rfind("</domain>") {
            let insert = match cpu_mode {
                CpuMode::Custom => format!(
                    r#"  <cpu mode="custom" match="exact"><model fallback="forbid">{}</model></cpu>
"#,
                    cpu_model.unwrap_or("qemu64")
                ),
                _ => format!("  <cpu mode=\"{}\"/>\n", cpu_mode.as_str()),
            };
            result.insert_str(pos, &insert);
        }
    }

    Ok(result)
}

pub fn modify_boot_order(
    xml: &str,
    boot_devices: &[BootDevice],
) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut in_os = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "os" {
                    in_os = true;
                }
                result.push('<');
                write_element(&mut result, e);
                result.push('>');
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "os" {
                    // Insert new boot entries before </os>
                    for dev in boot_devices {
                        result.push_str(&format!(r#"<boot dev="{}"/>"#, dev.as_str()));
                    }
                    in_os = false;
                }
                result.push_str(&format!("</{name}>"));
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                // Skip existing boot elements inside os
                if name == "boot" && in_os {
                    continue;
                }
                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(ref event) => {
                copy_event(&mut result, event);
                if matches!(event, Event::Eof) {
                    break;
                }
            }
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
        }
    }

    Ok(result)
}

pub fn modify_firmware(
    xml: &str,
    firmware: FirmwareType,
) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut in_os = false;
    let mut in_features = false;
    let mut found_smm = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "os" => {
                        in_os = true;
                        // Rewrite the <os> tag with or without firmware attribute
                        match firmware {
                            FirmwareType::Efi => {
                                result.push_str(r#"<os firmware="efi">"#);
                            }
                            FirmwareType::Bios => {
                                result.push_str("<os>");
                            }
                        }
                        continue;
                    }
                    "loader" | "nvram" if in_os => {
                        // Skip legacy loader/nvram elements (including their content)
                        // We need to skip until the closing tag
                        let mut depth = 1u32;
                        loop {
                            match reader.read_event() {
                                Ok(Event::Start(_)) => depth += 1,
                                Ok(Event::End(_)) => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                Ok(Event::Eof) => break,
                                _ => {}
                            }
                        }
                        continue;
                    }
                    "features" => {
                        in_features = true;
                        found_smm = false;
                        result.push('<');
                        write_element(&mut result, e);
                        result.push('>');
                    }
                    "smm" if in_features => {
                        found_smm = true;
                        if firmware == FirmwareType::Efi {
                            // Keep/update SMM
                            result.push_str(r#"<smm state="on">"#);
                        }
                        // If BIOS, skip the smm element
                        if firmware == FirmwareType::Bios {
                            let mut depth = 1u32;
                            loop {
                                match reader.read_event() {
                                    Ok(Event::Start(_)) => depth += 1,
                                    Ok(Event::End(_)) => {
                                        depth -= 1;
                                        if depth == 0 {
                                            break;
                                        }
                                    }
                                    Ok(Event::Eof) => break,
                                    _ => {}
                                }
                            }
                            continue;
                        } else {
                            result.push('<');
                            write_element(&mut result, e);
                            result.push('>');
                        }
                    }
                    _ => {
                        result.push('<');
                        write_element(&mut result, e);
                        result.push('>');
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "os" => {
                        in_os = false;
                        result.push_str("</os>");
                    }
                    "features" => {
                        // Insert SMM if EFI and not already present
                        if firmware == FirmwareType::Efi && !found_smm {
                            result.push_str(r#"<smm state="on"/>"#);
                        }
                        in_features = false;
                        result.push_str("</features>");
                    }
                    _ => {
                        result.push_str(&format!("</{name}>"));
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "loader" | "nvram" if in_os => {
                        // Skip legacy loader/nvram empty elements
                        continue;
                    }
                    "smm" if in_features => {
                        found_smm = true;
                        if firmware == FirmwareType::Efi {
                            result.push_str(r#"<smm state="on"/>"#);
                        }
                        // If BIOS, skip it
                        continue;
                    }
                    _ => {
                        result.push('<');
                        write_element(&mut result, e);
                        result.push_str("/>");
                    }
                }
            }
            Ok(ref event) => {
                if in_os {
                    // Check if this is text inside loader/nvram - those were already skipped
                    copy_event(&mut result, event);
                } else {
                    copy_event(&mut result, event);
                }
                if matches!(event, Event::Eof) {
                    break;
                }
            }
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
        }
    }

    Ok(result)
}

pub fn add_disk_device(xml: &str, params: &NewDiskParams) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let disk_xml = format!(
        r#"<disk type="file" device="{}"><driver name="qemu" type="{}"/><source file="{}"/><target dev="{}" bus="{}"/></disk>"#,
        params.device_type,
        params.driver_type,
        params.source_file,
        params.target_dev,
        params.bus,
    );

    loop {
        match reader.read_event() {
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "devices" {
                    result.push_str(&disk_xml);
                }
                result.push_str(&format!("</{name}>"));
            }
            Ok(ref event @ Event::Eof) => {
                copy_event(&mut result, event);
                break;
            }
            Ok(ref event) => {
                copy_event(&mut result, event);
            }
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
        }
    }

    Ok(result)
}

pub fn remove_disk_device(xml: &str, target_dev: &str) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut skip_depth: Option<u32> = None;
    let mut pending_disk: Option<(String, u32)> = None; // (buffered xml, depth)
    let mut disk_buffer = String::new();
    let mut disk_depth = 0u32;
    let mut found_target = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if let Some(ref mut depth) = skip_depth {
                    *depth += 1;
                    continue;
                }

                if pending_disk.is_some() {
                    disk_depth += 1;
                    // Check if this is a target element
                    if name == "target" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"dev" {
                                let dev = String::from_utf8_lossy(&attr.value).to_string();
                                if dev == target_dev {
                                    found_target = true;
                                }
                            }
                        }
                    }
                    disk_buffer.push('<');
                    write_element(&mut disk_buffer, e);
                    disk_buffer.push('>');
                    continue;
                }

                if name == "disk" {
                    pending_disk = Some((String::new(), 1));
                    disk_buffer.clear();
                    disk_buffer.push('<');
                    write_element(&mut disk_buffer, e);
                    disk_buffer.push('>');
                    disk_depth = 1;
                    found_target = false;
                    continue;
                }

                result.push('<');
                write_element(&mut result, e);
                result.push('>');
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if let Some(ref mut depth) = skip_depth {
                    *depth -= 1;
                    if *depth == 0 {
                        skip_depth = None;
                    }
                    continue;
                }

                if pending_disk.is_some() {
                    disk_depth -= 1;
                    disk_buffer.push_str(&format!("</{name}>"));
                    if disk_depth == 0 {
                        // Disk element is complete
                        if !found_target {
                            result.push_str(&disk_buffer);
                        }
                        pending_disk = None;
                        disk_buffer.clear();
                    }
                    continue;
                }

                result.push_str(&format!("</{name}>"));
            }
            Ok(Event::Empty(ref e)) => {
                if skip_depth.is_some() {
                    continue;
                }

                if pending_disk.is_some() {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name == "target" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"dev" {
                                let dev = String::from_utf8_lossy(&attr.value).to_string();
                                if dev == target_dev {
                                    found_target = true;
                                }
                            }
                        }
                    }
                    disk_buffer.push('<');
                    write_element(&mut disk_buffer, e);
                    disk_buffer.push_str("/>");
                    continue;
                }

                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(Event::Text(ref e)) => {
                if skip_depth.is_some() {
                    continue;
                }
                let text = e.unescape().unwrap_or_default().to_string();
                if pending_disk.is_some() {
                    disk_buffer.push_str(&text);
                } else {
                    result.push_str(&text);
                }
            }
            Ok(ref event @ Event::Decl(_)) | Ok(ref event @ Event::Comment(_)) => {
                copy_event(&mut result, event);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
            _ => {}
        }
    }

    Ok(result)
}

pub fn add_network_device(xml: &str, params: &NewNetworkParams) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let iface_xml = format!(
        r#"<interface type="network"><source network="{}"/><model type="{}"/></interface>"#,
        params.source_network, params.model_type,
    );

    loop {
        match reader.read_event() {
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "devices" {
                    result.push_str(&iface_xml);
                }
                result.push_str(&format!("</{name}>"));
            }
            Ok(ref event @ Event::Eof) => {
                copy_event(&mut result, event);
                break;
            }
            Ok(ref event) => {
                copy_event(&mut result, event);
            }
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
        }
    }

    Ok(result)
}

pub fn eject_cdrom(xml: &str, target_dev: &str) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    // Buffer the entire disk element, then decide whether to strip <source>
    let mut disk_buffer = String::new();
    let mut disk_depth = 0u32;
    let mut in_disk = false;
    let mut is_cdrom = false;
    let mut found_target = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if in_disk {
                    disk_depth += 1;
                    if name == "target" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"dev" {
                                let dev = String::from_utf8_lossy(&attr.value).to_string();
                                if dev == target_dev {
                                    found_target = true;
                                }
                            }
                        }
                    }
                    // Skip <source> children if this is our target cdrom
                    disk_buffer.push('<');
                    write_element(&mut disk_buffer, e);
                    disk_buffer.push('>');
                    continue;
                }

                if name == "disk" {
                    in_disk = true;
                    disk_depth = 1;
                    disk_buffer.clear();
                    is_cdrom = false;
                    found_target = false;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"device" {
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if val == "cdrom" {
                                is_cdrom = true;
                            }
                        }
                    }
                    disk_buffer.push('<');
                    write_element(&mut disk_buffer, e);
                    disk_buffer.push('>');
                    continue;
                }

                result.push('<');
                write_element(&mut result, e);
                result.push('>');
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if in_disk {
                    disk_depth -= 1;
                    disk_buffer.push_str(&format!("</{name}>"));
                    if disk_depth == 0 {
                        in_disk = false;
                        if is_cdrom && found_target {
                            // Re-emit the disk but strip <source .../> elements
                            result.push_str(&strip_source_from_disk(&disk_buffer));
                        } else {
                            result.push_str(&disk_buffer);
                        }
                        disk_buffer.clear();
                    }
                    continue;
                }

                result.push_str(&format!("</{name}>"));
            }
            Ok(Event::Empty(ref e)) => {
                if in_disk {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name == "target" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"dev" {
                                let dev = String::from_utf8_lossy(&attr.value).to_string();
                                if dev == target_dev {
                                    found_target = true;
                                }
                            }
                        }
                    }
                    disk_buffer.push('<');
                    write_element(&mut disk_buffer, e);
                    disk_buffer.push_str("/>");
                    continue;
                }

                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_disk {
                    disk_buffer.push_str(&text);
                } else {
                    result.push_str(&text);
                }
            }
            Ok(ref event @ Event::Decl(_)) | Ok(ref event @ Event::Comment(_)) => {
                copy_event(&mut result, event);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
            _ => {}
        }
    }

    Ok(result)
}

fn strip_source_from_disk(disk_xml: &str) -> String {
    let mut result = String::new();
    let mut reader = Reader::from_str(disk_xml);
    reader.config_mut().trim_text(false);

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "source" {
                    // Skip <source>...</source>
                    let mut depth = 1u32;
                    loop {
                        match reader.read_event() {
                            Ok(Event::Start(_)) => depth += 1,
                            Ok(Event::End(_)) => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            Ok(Event::Eof) => break,
                            _ => {}
                        }
                    }
                    continue;
                }
                result.push('<');
                write_element(&mut result, e);
                result.push('>');
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "source" {
                    continue;
                }
                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(ref event @ Event::Eof) => {
                copy_event(&mut result, event);
                break;
            }
            Ok(ref event) => {
                copy_event(&mut result, event);
            }
            Err(_) => break,
        }
    }

    result
}

pub fn change_cdrom_media(
    xml: &str,
    target_dev: &str,
    iso_path: &str,
) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut disk_buffer = String::new();
    let mut disk_depth = 0u32;
    let mut in_disk = false;
    let mut is_cdrom = false;
    let mut found_target = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if in_disk {
                    disk_depth += 1;
                    if name == "target" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"dev" {
                                let dev = String::from_utf8_lossy(&attr.value).to_string();
                                if dev == target_dev {
                                    found_target = true;
                                }
                            }
                        }
                    }
                    disk_buffer.push('<');
                    write_element(&mut disk_buffer, e);
                    disk_buffer.push('>');
                    continue;
                }

                if name == "disk" {
                    in_disk = true;
                    disk_depth = 1;
                    disk_buffer.clear();
                    is_cdrom = false;
                    found_target = false;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"device" {
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if val == "cdrom" {
                                is_cdrom = true;
                            }
                        }
                    }
                    disk_buffer.push('<');
                    write_element(&mut disk_buffer, e);
                    disk_buffer.push('>');
                    continue;
                }

                result.push('<');
                write_element(&mut result, e);
                result.push('>');
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if in_disk {
                    disk_depth -= 1;
                    disk_buffer.push_str(&format!("</{name}>"));
                    if disk_depth == 0 {
                        in_disk = false;
                        if is_cdrom && found_target {
                            result.push_str(&replace_source_in_disk(&disk_buffer, iso_path));
                        } else {
                            result.push_str(&disk_buffer);
                        }
                        disk_buffer.clear();
                    }
                    continue;
                }

                result.push_str(&format!("</{name}>"));
            }
            Ok(Event::Empty(ref e)) => {
                if in_disk {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name == "target" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"dev" {
                                let dev = String::from_utf8_lossy(&attr.value).to_string();
                                if dev == target_dev {
                                    found_target = true;
                                }
                            }
                        }
                    }
                    disk_buffer.push('<');
                    write_element(&mut disk_buffer, e);
                    disk_buffer.push_str("/>");
                    continue;
                }

                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_disk {
                    disk_buffer.push_str(&text);
                } else {
                    result.push_str(&text);
                }
            }
            Ok(ref event @ Event::Decl(_)) | Ok(ref event @ Event::Comment(_)) => {
                copy_event(&mut result, event);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
            _ => {}
        }
    }

    Ok(result)
}

fn replace_source_in_disk(disk_xml: &str, iso_path: &str) -> String {
    let mut result = String::new();
    let mut reader = Reader::from_str(disk_xml);
    reader.config_mut().trim_text(false);

    let mut found_source = false;
    let new_source = format!(r#"<source file="{iso_path}"/>"#);

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "source" {
                    found_source = true;
                    result.push_str(&new_source);
                    // Skip <source>...</source>
                    let mut depth = 1u32;
                    loop {
                        match reader.read_event() {
                            Ok(Event::Start(_)) => depth += 1,
                            Ok(Event::End(_)) => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            Ok(Event::Eof) => break,
                            _ => {}
                        }
                    }
                    continue;
                }
                if name == "target" && !found_source {
                    // Insert source before target
                    found_source = true;
                    result.push_str(&new_source);
                }
                result.push('<');
                write_element(&mut result, e);
                result.push('>');
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "source" {
                    found_source = true;
                    result.push_str(&new_source);
                    continue;
                }
                if name == "target" && !found_source {
                    found_source = true;
                    result.push_str(&new_source);
                }
                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(ref event @ Event::Eof) => {
                copy_event(&mut result, event);
                break;
            }
            Ok(ref event) => {
                copy_event(&mut result, event);
            }
            Err(_) => break,
        }
    }

    result
}

pub fn modify_cputune(xml: &str, cpu_tune: &CpuTune) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut found_cputune = false;
    let is_empty = cpu_tune.vcpu_pins.is_empty() && cpu_tune.emulatorpin.is_none();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "cputune" {
                    found_cputune = true;
                    // Skip existing cputune element entirely
                    let mut depth = 1u32;
                    loop {
                        match reader.read_event() {
                            Ok(Event::Start(_)) => depth += 1,
                            Ok(Event::End(_)) => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            Ok(Event::Eof) => break,
                            _ => {}
                        }
                    }
                    // Insert replacement if not empty
                    if !is_empty {
                        result.push_str(&build_cputune_xml(cpu_tune));
                    }
                    continue;
                }
                result.push('<');
                write_element(&mut result, e);
                result.push('>');
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "cputune" {
                    found_cputune = true;
                    if !is_empty {
                        result.push_str(&build_cputune_xml(cpu_tune));
                    }
                    continue;
                }
                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(ref event @ Event::Eof) => {
                copy_event(&mut result, event);
                break;
            }
            Ok(ref event) => {
                copy_event(&mut result, event);
            }
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
        }
    }

    if !found_cputune && !is_empty {
        // Insert before </domain>
        if let Some(pos) = result.rfind("</domain>") {
            result.insert_str(pos, &build_cputune_xml(cpu_tune));
        }
    }

    Ok(result)
}

fn build_cputune_xml(cpu_tune: &CpuTune) -> String {
    let mut xml = String::from("<cputune>");
    for pin in &cpu_tune.vcpu_pins {
        xml.push_str(&format!(
            r#"<vcpupin vcpu="{}" cpuset="{}"/>"#,
            pin.vcpu, pin.cpuset
        ));
    }
    if let Some(ref cpuset) = cpu_tune.emulatorpin {
        xml.push_str(&format!(r#"<emulatorpin cpuset="{}"/>"#, cpuset));
    }
    xml.push_str("</cputune>");
    xml
}

pub fn remove_network_device(xml: &str, mac_address: &str) -> Result<String, AppError> {
    let mut result = String::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut iface_buffer = String::new();
    let mut iface_depth = 0u32;
    let mut in_iface = false;
    let mut found_mac = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if in_iface {
                    iface_depth += 1;
                    iface_buffer.push('<');
                    write_element(&mut iface_buffer, e);
                    iface_buffer.push('>');
                    continue;
                }

                if name == "interface" {
                    in_iface = true;
                    iface_depth = 1;
                    iface_buffer.clear();
                    iface_buffer.push('<');
                    write_element(&mut iface_buffer, e);
                    iface_buffer.push('>');
                    found_mac = false;
                    continue;
                }

                result.push('<');
                write_element(&mut result, e);
                result.push('>');
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if in_iface {
                    iface_depth -= 1;
                    iface_buffer.push_str(&format!("</{name}>"));
                    if iface_depth == 0 {
                        in_iface = false;
                        if !found_mac {
                            result.push_str(&iface_buffer);
                        }
                        iface_buffer.clear();
                    }
                    continue;
                }

                result.push_str(&format!("</{name}>"));
            }
            Ok(Event::Empty(ref e)) => {
                if in_iface {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name == "mac" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"address" {
                                let addr = String::from_utf8_lossy(&attr.value).to_string();
                                if addr == mac_address {
                                    found_mac = true;
                                }
                            }
                        }
                    }
                    iface_buffer.push('<');
                    write_element(&mut iface_buffer, e);
                    iface_buffer.push_str("/>");
                    continue;
                }

                result.push('<');
                write_element(&mut result, e);
                result.push_str("/>");
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_iface {
                    iface_buffer.push_str(&text);
                } else {
                    result.push_str(&text);
                }
            }
            Ok(ref event @ Event::Decl(_)) | Ok(ref event @ Event::Comment(_)) => {
                copy_event(&mut result, event);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Xml(format!("XML parse error: {e}"))),
            _ => {}
        }
    }

    Ok(result)
}
