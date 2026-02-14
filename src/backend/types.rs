use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootDevice {
    Hd,
    Cdrom,
    Network,
    Fd,
}

impl BootDevice {
    pub fn as_str(&self) -> &'static str {
        match self {
            BootDevice::Hd => "hd",
            BootDevice::Cdrom => "cdrom",
            BootDevice::Network => "network",
            BootDevice::Fd => "fd",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "hd" => Some(BootDevice::Hd),
            "cdrom" => Some(BootDevice::Cdrom),
            "network" => Some(BootDevice::Network),
            "fd" => Some(BootDevice::Fd),
        _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            BootDevice::Hd => "Hard Disk",
            BootDevice::Cdrom => "CD-ROM",
            BootDevice::Network => "Network (PXE)",
            BootDevice::Fd => "Floppy",
        }
    }

    pub const ALL: &[BootDevice] = &[
        BootDevice::Hd,
        BootDevice::Cdrom,
        BootDevice::Network,
        BootDevice::Fd,
    ];
}

impl fmt::Display for BootDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuMode {
    HostPassthrough,
    HostModel,
    Custom,
}

impl CpuMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            CpuMode::HostPassthrough => "host-passthrough",
            CpuMode::HostModel => "host-model",
            CpuMode::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "host-passthrough" => CpuMode::HostPassthrough,
            "host-model" => CpuMode::HostModel,
            "custom" => CpuMode::Custom,
            _ => CpuMode::HostPassthrough,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            CpuMode::HostPassthrough => "Host Passthrough",
            CpuMode::HostModel => "Host Model",
            CpuMode::Custom => "Custom",
        }
    }

    pub const ALL: &[CpuMode] = &[
        CpuMode::HostPassthrough,
        CpuMode::HostModel,
        CpuMode::Custom,
    ];
}

impl fmt::Display for CpuMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

pub const CPU_MODELS: &[&str] = &[
    "Skylake-Client",
    "Skylake-Server",
    "Cascadelake-Server",
    "Haswell",
    "Broadwell",
    "IvyBridge",
    "SandyBridge",
    "Westmere",
    "Nehalem",
    "EPYC",
    "EPYC-Rome",
    "EPYC-Milan",
    "Opteron_G5",
    "qemu64",
];

#[derive(Debug, Clone)]
pub struct NewDiskParams {
    pub source_file: String,
    pub target_dev: String,
    pub bus: String,
    pub device_type: String,
    pub driver_type: String,
    pub create_new: bool,
    pub size_gib: u64,
}

#[derive(Debug, Clone)]
pub struct NewNetworkParams {
    pub source_network: String,
    pub model_type: String,
}

#[derive(Debug, Clone)]
pub struct ConfigChanges {
    pub vcpus: u32,
    pub memory_mib: u64,
    pub cpu_mode: CpuMode,
    pub cpu_model: Option<String>,
    pub boot_order: Vec<BootDevice>,
    pub autostart: bool,
}

#[derive(Debug, Clone)]
pub enum ConfigAction {
    ApplyGeneral(ConfigChanges),
    AddDisk(NewDiskParams),
    RemoveDisk(String),
    AddNetwork(NewNetworkParams),
    RemoveNetwork(String),
    SetAutostart(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmState {
    Running,
    Paused,
    Shutoff,
    Crashed,
    PmSuspended,
    Other,
}

impl VmState {
    pub fn from_libvirt(state: u32) -> Self {
        match state {
            1 => VmState::Running,
            3 => VmState::Paused,
            5 => VmState::Shutoff,
            6 => VmState::Crashed,
            7 => VmState::PmSuspended,
            _ => VmState::Other,
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            VmState::Running => "status-running",
            VmState::Paused => "status-paused",
            VmState::Shutoff => "status-shutoff",
            VmState::Crashed => "status-crashed",
            VmState::PmSuspended => "status-paused",
            VmState::Other => "status-shutoff",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            VmState::Running => "Running",
            VmState::Paused => "Paused",
            VmState::Shutoff => "Shutoff",
            VmState::Crashed => "Crashed",
            VmState::PmSuspended => "Suspended",
            VmState::Other => "Unknown",
        }
    }

    pub fn as_str(&self) -> &'static str {
        self.label()
    }
}

impl fmt::Display for VmState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct VmInfo {
    pub name: String,
    pub uuid: String,
    pub state: VmState,
    pub vcpus: u32,
    pub memory_kib: u64,
    pub id: Option<u32>,
}

impl VmInfo {
    pub fn memory_mib(&self) -> u64 {
        self.memory_kib / 1024
    }

    pub fn subtitle(&self) -> String {
        match self.state {
            VmState::Running => format!("{} - {} vCPUs, {} MiB", self.state, self.vcpus, self.memory_mib()),
            _ => self.state.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub target_dev: String,
    pub source_file: Option<String>,
    pub bus: String,
    pub device_type: String,
}

#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub mac_address: Option<String>,
    pub source_network: Option<String>,
    pub model_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DomainDetails {
    pub name: String,
    pub uuid: String,
    pub memory_kib: u64,
    pub vcpus: u32,
    pub os_type: String,
    pub arch: String,
    pub disks: Vec<DiskInfo>,
    pub networks: Vec<NetworkInfo>,
    pub has_graphics: bool,
    pub boot_order: Vec<BootDevice>,
    pub cpu_mode: CpuMode,
    pub cpu_model: Option<String>,
}
