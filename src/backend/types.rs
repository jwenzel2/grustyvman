use std::fmt;

// --- Snapshot Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotState {
    Running,
    Paused,
    Shutoff,
    DiskSnapshot,
    Other,
}

impl SnapshotState {
    pub fn from_xml_str(s: &str) -> Self {
        match s {
            "running" => SnapshotState::Running,
            "paused" => SnapshotState::Paused,
            "shutoff" => SnapshotState::Shutoff,
            "disk-snapshot" => SnapshotState::DiskSnapshot,
            _ => SnapshotState::Other,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SnapshotState::Running => "Running",
            SnapshotState::Paused => "Paused",
            SnapshotState::Shutoff => "Shutoff",
            SnapshotState::DiskSnapshot => "Disk Snapshot",
            SnapshotState::Other => "Unknown",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            SnapshotState::Running => "status-running",
            SnapshotState::Paused => "status-paused",
            SnapshotState::Shutoff => "status-shutoff",
            SnapshotState::DiskSnapshot => "status-paused",
            SnapshotState::Other => "status-shutoff",
        }
    }
}

impl fmt::Display for SnapshotState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotInfo {
    pub name: String,
    pub description: String,
    pub state: SnapshotState,
    pub creation_time: i64,
    pub is_current: bool,
}

#[derive(Debug, Clone)]
pub struct CreateSnapshotParams {
    pub name: String,
    pub description: String,
}

// --- Performance Monitoring Types ---

pub struct RawPerfSample {
    pub timestamp_ns: u64,
    pub cpu_time_ns: u64,
    pub nr_vcpus: u32,
    pub memory_total_kib: u64,
    pub memory_unused_kib: u64,
    pub disk_rd_bytes: i64,
    pub disk_wr_bytes: i64,
    pub net_rx_bytes: i64,
    pub net_tx_bytes: i64,
}

pub struct PerfDataPoint {
    pub cpu_percent: f64,
    pub memory_used_percent: f64,
    pub memory_used_mib: f64,
    pub memory_total_mib: f64,
    pub disk_read_bytes_sec: f64,
    pub disk_write_bytes_sec: f64,
    pub net_rx_bytes_sec: f64,
    pub net_tx_bytes_sec: f64,
}

// --- Storage Pool Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolState {
    Inactive,
    Building,
    Running,
    Degraded,
    Inaccessible,
}

impl PoolState {
    pub fn from_libvirt(state: u32) -> Self {
        match state {
            0 => PoolState::Inactive,
            1 => PoolState::Building,
            2 => PoolState::Running,
            3 => PoolState::Degraded,
            4 => PoolState::Inaccessible,
            _ => PoolState::Inactive,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PoolState::Inactive => "Inactive",
            PoolState::Building => "Building",
            PoolState::Running => "Active",
            PoolState::Degraded => "Degraded",
            PoolState::Inaccessible => "Inaccessible",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            PoolState::Running => "status-running",
            PoolState::Building => "status-paused",
            PoolState::Inactive => "status-shutoff",
            PoolState::Degraded | PoolState::Inaccessible => "status-crashed",
        }
    }
}

impl fmt::Display for PoolState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct PoolInfo {
    pub name: String,
    pub uuid: String,
    pub state: PoolState,
    pub capacity: u64,
    pub allocation: u64,
    pub available: u64,
    pub active: bool,
    pub persistent: bool,
    pub autostart: bool,
}

impl PoolInfo {
    pub fn subtitle(&self) -> String {
        if self.active {
            format!("{} - {}", self.state, format_bytes(self.capacity))
        } else {
            self.state.to_string()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeType {
    File,
    Block,
    Dir,
    Network,
    NetDir,
    Ploop,
}

impl VolumeType {
    pub fn from_libvirt(kind: u32) -> Self {
        match kind {
            0 => VolumeType::File,
            1 => VolumeType::Block,
            2 => VolumeType::Dir,
            3 => VolumeType::Network,
            4 => VolumeType::NetDir,
            5 => VolumeType::Ploop,
            _ => VolumeType::File,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            VolumeType::File => "File",
            VolumeType::Block => "Block",
            VolumeType::Dir => "Directory",
            VolumeType::Network => "Network",
            VolumeType::NetDir => "NetDir",
            VolumeType::Ploop => "Ploop",
        }
    }
}

impl fmt::Display for VolumeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct VolumeInfo {
    pub name: String,
    pub path: String,
    pub kind: VolumeType,
    pub capacity: u64,
    pub allocation: u64,
}

#[derive(Debug, Clone, Default)]
pub struct PoolCreateParams {
    pub target_path: String,
    pub source_device: String,
    pub source_host: String,
    pub source_dir: String,
    pub source_name: String,
    pub source_format: String,
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;

    if bytes >= TIB {
        format!("{:.1} TiB", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

// --- Network Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkState {
    Active,
    Inactive,
}

impl NetworkState {
    pub fn label(&self) -> &'static str {
        match self {
            NetworkState::Active => "Active",
            NetworkState::Inactive => "Inactive",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            NetworkState::Active => "status-running",
            NetworkState::Inactive => "status-shutoff",
        }
    }
}

impl fmt::Display for NetworkState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardMode {
    Nat,
    Route,
    Isolated,
    Bridge,
    Open,
}

impl ForwardMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ForwardMode::Nat => "nat",
            ForwardMode::Route => "route",
            ForwardMode::Isolated => "isolated",
            ForwardMode::Bridge => "bridge",
            ForwardMode::Open => "open",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "nat" => ForwardMode::Nat,
            "route" => ForwardMode::Route,
            "bridge" => ForwardMode::Bridge,
            "open" => ForwardMode::Open,
            _ => ForwardMode::Isolated,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ForwardMode::Nat => "NAT",
            ForwardMode::Route => "Routed",
            ForwardMode::Isolated => "Isolated",
            ForwardMode::Bridge => "Bridge",
            ForwardMode::Open => "Open",
        }
    }

    pub const ALL: &[ForwardMode] = &[
        ForwardMode::Nat,
        ForwardMode::Route,
        ForwardMode::Isolated,
        ForwardMode::Bridge,
        ForwardMode::Open,
    ];
}

impl fmt::Display for ForwardMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct VirtNetworkInfo {
    pub name: String,
    pub uuid: String,
    pub state: NetworkState,
    pub active: bool,
    pub persistent: bool,
    pub autostart: bool,
    pub forward_mode: ForwardMode,
    pub bridge_name: Option<String>,
    pub ip_address: Option<String>,
    pub ip_netmask: Option<String>,
    pub dhcp_start: Option<String>,
    pub dhcp_end: Option<String>,
}

impl VirtNetworkInfo {
    pub fn subtitle(&self) -> String {
        if self.active {
            format!("{} - {}", self.state, self.forward_mode)
        } else {
            self.state.to_string()
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkCreateParams {
    pub name: String,
    pub forward_mode: ForwardMode,
    pub bridge_name: String,
    pub ip_address: String,
    pub ip_netmask: String,
    pub dhcp_enabled: bool,
    pub dhcp_start: String,
    pub dhcp_end: String,
}

// --- Host Info Types ---

#[derive(Debug, Clone)]
pub struct HostInfo {
    pub hostname: String,
    pub uri: String,
    pub libvirt_version: String,
    pub hypervisor_version: String,
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub cpu_threads: u32,
    pub cpu_mhz: u32,
    pub cpu_sockets: u32,
    pub cpu_nodes: u32,
    pub memory_kib: u64,
}

// --- Graphics/Video/Sound Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsType {
    Spice,
    Vnc,
    None,
}

impl GraphicsType {
    pub fn as_str(&self) -> &'static str {
        match self {
            GraphicsType::Spice => "spice",
            GraphicsType::Vnc => "vnc",
            GraphicsType::None => "none",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "spice" => GraphicsType::Spice,
            "vnc" => GraphicsType::Vnc,
            _ => GraphicsType::None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            GraphicsType::Spice => "SPICE",
            GraphicsType::Vnc => "VNC",
            GraphicsType::None => "None",
        }
    }

    pub const ALL: &[GraphicsType] = &[
        GraphicsType::Spice,
        GraphicsType::Vnc,
        GraphicsType::None,
    ];
}

impl fmt::Display for GraphicsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct GraphicsInfo {
    pub graphics_type: GraphicsType,
    pub port: Option<i32>,
    pub autoport: bool,
    pub listen_address: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoModel {
    Virtio,
    Qxl,
    Vga,
    Bochs,
    Ramfb,
    None,
}

impl VideoModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            VideoModel::Virtio => "virtio",
            VideoModel::Qxl => "qxl",
            VideoModel::Vga => "vga",
            VideoModel::Bochs => "bochs",
            VideoModel::Ramfb => "ramfb",
            VideoModel::None => "none",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "virtio" => VideoModel::Virtio,
            "qxl" => VideoModel::Qxl,
            "vga" => VideoModel::Vga,
            "bochs" => VideoModel::Bochs,
            "ramfb" => VideoModel::Ramfb,
            _ => VideoModel::None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            VideoModel::Virtio => "Virtio",
            VideoModel::Qxl => "QXL",
            VideoModel::Vga => "VGA",
            VideoModel::Bochs => "Bochs",
            VideoModel::Ramfb => "Ramfb",
            VideoModel::None => "None",
        }
    }

    pub const ALL: &[VideoModel] = &[
        VideoModel::Virtio,
        VideoModel::Qxl,
        VideoModel::Vga,
        VideoModel::Bochs,
        VideoModel::Ramfb,
        VideoModel::None,
    ];
}

impl fmt::Display for VideoModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub model: VideoModel,
    pub vram: Option<u32>,
    pub heads: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundModel {
    Ich9,
    Ich6,
    Ac97,
    Usb,
    None,
}

impl SoundModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            SoundModel::Ich9 => "ich9",
            SoundModel::Ich6 => "ich6",
            SoundModel::Ac97 => "ac97",
            SoundModel::Usb => "usb",
            SoundModel::None => "none",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "ich9" => SoundModel::Ich9,
            "ich6" => SoundModel::Ich6,
            "ac97" => SoundModel::Ac97,
            "usb" => SoundModel::Usb,
            _ => SoundModel::None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SoundModel::Ich9 => "ICH9",
            SoundModel::Ich6 => "ICH6",
            SoundModel::Ac97 => "AC97",
            SoundModel::Usb => "USB",
            SoundModel::None => "None",
        }
    }

    pub const ALL: &[SoundModel] = &[
        SoundModel::Ich9,
        SoundModel::Ich6,
        SoundModel::Ac97,
        SoundModel::Usb,
        SoundModel::None,
    ];
}

impl fmt::Display for SoundModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct SoundInfo {
    pub model: SoundModel,
}

// --- TPM Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpmModel {
    Crb,
    Tis,
    None,
}

impl TpmModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            TpmModel::Crb => "tpm-crb",
            TpmModel::Tis => "tpm-tis",
            TpmModel::None => "none",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "tpm-crb" => TpmModel::Crb,
            "tpm-tis" => TpmModel::Tis,
            _ => TpmModel::None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TpmModel::Crb => "CRB",
            TpmModel::Tis => "TIS",
            TpmModel::None => "None",
        }
    }

    pub const ALL: &[TpmModel] = &[TpmModel::Crb, TpmModel::Tis, TpmModel::None];
}

impl fmt::Display for TpmModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct TpmInfo {
    pub model: TpmModel,
    pub version: String,
}

// --- Filesystem Passthrough Types ---

#[derive(Debug, Clone)]
pub struct FilesystemInfo {
    pub driver: String,
    pub source_dir: String,
    pub target_dir: String,
    pub accessmode: Option<String>,
}

// --- CPU Pinning Types ---

#[derive(Debug, Clone)]
pub struct VcpuPin {
    pub vcpu: u32,
    pub cpuset: String,
}

#[derive(Debug, Clone, Default)]
pub struct CpuTune {
    pub vcpu_pins: Vec<VcpuPin>,
    pub emulatorpin: Option<String>,
}

// --- VM Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskFormat {
    Qcow2,
    Raw,
}

impl DiskFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiskFormat::Qcow2 => "qcow2",
            DiskFormat::Raw => "raw",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            DiskFormat::Qcow2 => "qcow2",
            DiskFormat::Raw => "img",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DiskFormat::Qcow2 => "qcow2",
            DiskFormat::Raw => "raw",
        }
    }

    pub const ALL: &[DiskFormat] = &[DiskFormat::Qcow2, DiskFormat::Raw];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareType {
    Bios,
    Efi,
}

impl FirmwareType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FirmwareType::Bios => "bios",
            FirmwareType::Efi => "efi",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "efi" => FirmwareType::Efi,
            _ => FirmwareType::Bios,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            FirmwareType::Bios => "BIOS",
            FirmwareType::Efi => "UEFI",
        }
    }

    pub const ALL: &[FirmwareType] = &[FirmwareType::Bios, FirmwareType::Efi];
}

impl fmt::Display for FirmwareType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

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
    pub mac_address: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigChanges {
    pub vcpus: u32,
    pub memory_mib: u64,
    pub cpu_mode: CpuMode,
    pub cpu_model: Option<String>,
    pub boot_order: Vec<BootDevice>,
    pub autostart: bool,
    pub firmware: FirmwareType,
}

#[derive(Debug, Clone)]
pub enum ConfigAction {
    ApplyGeneral(ConfigChanges),
    AddDisk(NewDiskParams),
    RemoveDisk(String),
    AddNetwork(NewNetworkParams),
    RemoveNetwork(String),
    ChangeNetworkSource(String, ChangeNetworkSourceParams), // (mac, params)
    SetAutostart(bool),
    ModifyGraphics(GraphicsType),
    ModifyVideo(VideoModel),
    ModifySound(SoundModel),
    EjectCdrom(String),
    InsertCdrom(String, String),
    ChangeDiskImage(String, String), // (target_dev, new_image_path)
    ApplyCpuTune(CpuTune),
    ModifyTpm(TpmModel),
    AddFilesystem(FilesystemInfo),
    RemoveFilesystem(String),
    AddHostdev(HostdevInfo),
    RemoveHostdev(HostdevInfo),
    AddSerial(SerialInfo),
    RemoveSerial(SerialInfo),
    ModifyRng(Option<RngBackend>),
    ModifyWatchdog(WatchdogModel, WatchdogAction),
    AddInput(InputInfo),
    RemoveInput(InputInfo),
    AddChannel(ChannelInfo),
    RemoveChannel(String), // target_name
    AddController(ControllerInfo),
    RemoveController(ControllerInfo),
    AddParallel,
    RemoveParallel(u32),   // by port number
    ModifyPanic(PanicModel),
    AddUsbredir,
    RemoveUsbredir(u32),   // by index
    ModifySmartcard(SmartcardMode),
    ModifyMemballoon(MemballoonModel),
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
    pub interface_type: String,       // "network", "bridge", "direct", "vdpa"
    pub source_network: Option<String>, // for type="network"
    pub source_bridge: Option<String>,  // for type="bridge"
    pub source_dev: Option<String>,     // for type="direct" or "vdpa"
    pub model_type: Option<String>,
}

impl NetworkInfo {
    pub fn display_source(&self) -> String {
        match self.interface_type.as_str() {
            "bridge" => format!(
                "Bridge: {}",
                self.source_bridge.as_deref().unwrap_or("N/A")
            ),
            "direct" => format!(
                "Macvtap: {}",
                self.source_dev.as_deref().unwrap_or("N/A")
            ),
            "vdpa" => format!(
                "vDPA: {}",
                self.source_dev.as_deref().unwrap_or("N/A")
            ),
            _ => format!(
                "Network: {}",
                self.source_network.as_deref().unwrap_or("N/A")
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkSourceType {
    VirtualNetwork,
    Bridge,
    Macvtap,
    Vdpa,
}

impl NetworkSourceType {
    pub const ALL: &'static [NetworkSourceType] = &[
        NetworkSourceType::VirtualNetwork,
        NetworkSourceType::Bridge,
        NetworkSourceType::Macvtap,
        NetworkSourceType::Vdpa,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            NetworkSourceType::VirtualNetwork => "Virtual Network",
            NetworkSourceType::Bridge => "Bridge Device",
            NetworkSourceType::Macvtap => "Macvtap Device",
            NetworkSourceType::Vdpa => "vDPA Device",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChangeNetworkSourceParams {
    pub source_type: NetworkSourceType,
    pub value: String, // network name, bridge dev, or device path
}

#[derive(Debug, Clone)]
pub struct HostdevInfo {
    pub device_type: String,       // "pci" or "usb"
    pub pci_domain: Option<String>,
    pub pci_bus: Option<String>,
    pub pci_slot: Option<String>,
    pub pci_function: Option<String>,
    pub usb_vendor: Option<String>,
    pub usb_product: Option<String>,
    pub display_name: String,
}

impl HostdevInfo {
    pub fn address_key(&self) -> String {
        if self.device_type == "pci" {
            format!(
                "pci:{}:{}:{}:{}",
                self.pci_domain.as_deref().unwrap_or("0x0000"),
                self.pci_bus.as_deref().unwrap_or("0x00"),
                self.pci_slot.as_deref().unwrap_or("0x00"),
                self.pci_function.as_deref().unwrap_or("0x0")
            )
        } else {
            format!(
                "usb:{}:{}",
                self.usb_vendor.as_deref().unwrap_or(""),
                self.usb_product.as_deref().unwrap_or("")
            )
        }
    }

    pub fn display_subtitle(&self) -> String {
        if self.device_type == "pci" {
            format!(
                "PCI {}:{}:{}.{}",
                self.pci_domain.as_deref().unwrap_or("0000").trim_start_matches("0x"),
                self.pci_bus.as_deref().unwrap_or("00").trim_start_matches("0x"),
                self.pci_slot.as_deref().unwrap_or("00").trim_start_matches("0x"),
                self.pci_function.as_deref().unwrap_or("0").trim_start_matches("0x"),
            )
        } else {
            format!(
                "USB {}:{}",
                self.usb_vendor.as_deref().unwrap_or(""),
                self.usb_product.as_deref().unwrap_or("")
            )
        }
    }
}

// --- Serial / Console Types ---

#[derive(Debug, Clone)]
pub struct SerialInfo {
    pub is_console: bool,    // true = <console> element, false = <serial>
    pub target_type: String, // "isa-serial", "virtio", "serial"
    pub port: u32,
}

impl SerialInfo {
    pub fn display_name(&self) -> String {
        let kind = if self.is_console { "Console" } else { "Serial" };
        format!("{} Port {}", kind, self.port)
    }

    pub fn display_subtitle(&self) -> String {
        let kind = if self.is_console { "console" } else { "serial" };
        format!("type=pty, target={} ({})", self.target_type, kind)
    }
}

// --- RNG Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RngBackend {
    Random,
    Urandom,
}

impl RngBackend {
    pub const ALL: &'static [RngBackend] = &[RngBackend::Random, RngBackend::Urandom];

    pub fn label(&self) -> &'static str {
        match self {
            RngBackend::Random => "/dev/random",
            RngBackend::Urandom => "/dev/urandom",
        }
    }

    pub fn path(&self) -> &'static str {
        match self {
            RngBackend::Random => "/dev/random",
            RngBackend::Urandom => "/dev/urandom",
        }
    }

    pub fn from_path(s: &str) -> Option<Self> {
        match s {
            "/dev/random" => Some(RngBackend::Random),
            "/dev/urandom" => Some(RngBackend::Urandom),
            _ => Some(RngBackend::Urandom),
        }
    }
}

// --- Watchdog Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogModel {
    I6300esb,
    Ib700,
    Diag288,
    None,
}

impl WatchdogModel {
    pub const ALL: &'static [WatchdogModel] = &[
        WatchdogModel::I6300esb,
        WatchdogModel::Ib700,
        WatchdogModel::Diag288,
        WatchdogModel::None,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            WatchdogModel::I6300esb => "i6300esb",
            WatchdogModel::Ib700 => "ib700",
            WatchdogModel::Diag288 => "diag288",
            WatchdogModel::None => "None (disabled)",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            WatchdogModel::I6300esb => "i6300esb",
            WatchdogModel::Ib700 => "ib700",
            WatchdogModel::Diag288 => "diag288",
            WatchdogModel::None => "none",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "i6300esb" => WatchdogModel::I6300esb,
            "ib700" => WatchdogModel::Ib700,
            "diag288" => WatchdogModel::Diag288,
            _ => WatchdogModel::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogAction {
    Reset,
    Shutdown,
    Poweroff,
    Pause,
    None,
}

impl WatchdogAction {
    pub const ALL: &'static [WatchdogAction] = &[
        WatchdogAction::Reset,
        WatchdogAction::Shutdown,
        WatchdogAction::Poweroff,
        WatchdogAction::Pause,
        WatchdogAction::None,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            WatchdogAction::Reset => "Reset",
            WatchdogAction::Shutdown => "Shutdown",
            WatchdogAction::Poweroff => "Power Off",
            WatchdogAction::Pause => "Pause",
            WatchdogAction::None => "None",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            WatchdogAction::Reset => "reset",
            WatchdogAction::Shutdown => "shutdown",
            WatchdogAction::Poweroff => "poweroff",
            WatchdogAction::Pause => "pause",
            WatchdogAction::None => "none",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "reset" => WatchdogAction::Reset,
            "shutdown" => WatchdogAction::Shutdown,
            "poweroff" => WatchdogAction::Poweroff,
            "pause" => WatchdogAction::Pause,
            _ => WatchdogAction::None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WatchdogInfo {
    pub model: WatchdogModel,
    pub action: WatchdogAction,
}

// --- Input Device Types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputInfo {
    pub input_type: String, // "tablet", "mouse", "keyboard"
    pub bus: String,        // "usb", "virtio", "ps2"
}

impl InputInfo {
    pub fn display_name(&self) -> String {
        let bus = match self.bus.as_str() {
            "virtio" => "VirtIO",
            "usb" => "USB",
            "ps2" => "PS/2",
            other => other,
        };
        let kind = match self.input_type.as_str() {
            "tablet" => "Tablet",
            "mouse" => "Mouse",
            "keyboard" => "Keyboard",
            other => other,
        };
        format!("{bus} {kind}")
    }

    pub fn display_subtitle(&self) -> String {
        format!("type={}, bus={}", self.input_type, self.bus)
    }
}

// --- Controller Types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerInfo {
    pub controller_type: String, // "usb", "scsi", "virtio-serial"
    pub model: Option<String>,   // "qemu-xhci", "virtio-scsi", etc.
    pub index: u32,
}

impl ControllerInfo {
    pub fn display_name(&self) -> String {
        match self.controller_type.as_str() {
            "usb" => match self.model.as_deref() {
                Some("qemu-xhci") => "USB 3.0 Controller (xHCI)".to_string(),
                Some("ich9-ehci1") | Some("ich9-uhci1") => "USB 2.0 Controller (EHCI)".to_string(),
                Some(m) => format!("USB Controller ({})", m),
                None => "USB Controller".to_string(),
            },
            "scsi" => match self.model.as_deref() {
                Some("virtio-scsi") => "VirtIO SCSI Controller".to_string(),
                Some(m) => format!("SCSI Controller ({})", m),
                None => "SCSI Controller".to_string(),
            },
            "virtio-serial" => "VirtIO Serial Controller".to_string(),
            other => format!("{} Controller", other),
        }
    }

    pub fn display_subtitle(&self) -> String {
        format!(
            "type={}, index={}{}",
            self.controller_type,
            self.index,
            self.model.as_deref().map(|m| format!(", model={}", m)).unwrap_or_default()
        )
    }

    /// Returns true for system controllers that should not be shown in UI
    pub fn is_system(&self) -> bool {
        matches!(
            self.controller_type.as_str(),
            "pci" | "pcie-root" | "pcie-root-port" | "ide" | "sata" | "pcie-to-pci-bridge"
        )
    }
}

// --- Parallel Port Types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParallelInfo {
    pub port: u32,
}

impl ParallelInfo {
    pub fn display_name(&self) -> String {
        format!("Parallel Port {}", self.port)
    }

    pub fn display_subtitle(&self) -> String {
        format!("type=pty, port={}", self.port)
    }
}

// --- Panic Device Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanicModel {
    Hyperv,
    Isa,
    Pvpanic,
    None,
}

impl PanicModel {
    pub const ALL: &'static [PanicModel] = &[
        PanicModel::Hyperv,
        PanicModel::Isa,
        PanicModel::Pvpanic,
        PanicModel::None,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            PanicModel::Hyperv => "hyperv",
            PanicModel::Isa => "isa",
            PanicModel::Pvpanic => "pvpanic",
            PanicModel::None => "none",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "hyperv" => PanicModel::Hyperv,
            "isa" => PanicModel::Isa,
            "pvpanic" => PanicModel::Pvpanic,
            _ => PanicModel::None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PanicModel::Hyperv => "Hyper-V (Windows guests)",
            PanicModel::Isa => "ISA (legacy)",
            PanicModel::Pvpanic => "PVPanic (Linux guests)",
            PanicModel::None => "None (disabled)",
        }
    }
}

// --- USB Redirection Types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbredirInfo {
    pub index: u32,
}

impl UsbredirInfo {
    pub fn display_name(&self) -> String {
        format!("USB Redirect Channel {}", self.index)
    }

    pub fn display_subtitle(&self) -> String {
        "type=spicevmc, bus=usb".to_string()
    }
}

// --- Smartcard Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartcardMode {
    Passthrough,
    Host,
    None,
}

impl SmartcardMode {
    pub const ALL: &'static [SmartcardMode] = &[
        SmartcardMode::Passthrough,
        SmartcardMode::Host,
        SmartcardMode::None,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            SmartcardMode::Passthrough => "passthrough",
            SmartcardMode::Host => "host",
            SmartcardMode::None => "none",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "passthrough" => SmartcardMode::Passthrough,
            "host" => SmartcardMode::Host,
            _ => SmartcardMode::None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SmartcardMode::Passthrough => "Passthrough (SPICE)",
            SmartcardMode::Host => "Host (emulated)",
            SmartcardMode::None => "None (disabled)",
        }
    }
}

// --- Memory Balloon Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemballoonModel {
    Virtio,
    None,
}

impl MemballoonModel {
    pub const ALL: &'static [MemballoonModel] = &[MemballoonModel::Virtio, MemballoonModel::None];

    pub fn as_str(&self) -> &'static str {
        match self {
            MemballoonModel::Virtio => "virtio",
            MemballoonModel::None => "none",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "virtio" => MemballoonModel::Virtio,
            _ => MemballoonModel::None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            MemballoonModel::Virtio => "VirtIO",
            MemballoonModel::None => "None (disabled)",
        }
    }
}

// --- Channel Types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelInfo {
    pub channel_type: String, // "spicevmc", "unix", "pty"
    pub target_name: String,  // "com.redhat.spice.0", "org.qemu.guest_agent.0", etc.
}

impl ChannelInfo {
    pub fn display_name(&self) -> String {
        match self.target_name.as_str() {
            "com.redhat.spice.0" => "SPICE Agent".to_string(),
            "org.qemu.guest_agent.0" => "QEMU Guest Agent".to_string(),
            other => other.to_string(),
        }
    }

    pub fn display_subtitle(&self) -> String {
        format!("type={}, target=virtio", self.channel_type)
    }
}

// --- Domain Details ---

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
    pub firmware: FirmwareType,
    pub graphics: Option<GraphicsInfo>,
    pub video: Option<VideoInfo>,
    pub sound: Option<SoundInfo>,
    pub cpu_tune: CpuTune,
    pub tpm: Option<TpmInfo>,
    pub filesystems: Vec<FilesystemInfo>,
    pub hostdevs: Vec<HostdevInfo>,
    pub serials: Vec<SerialInfo>,
    pub rng: Option<RngBackend>,
    pub watchdog: Option<WatchdogInfo>,
    pub inputs: Vec<InputInfo>,
    pub channels: Vec<ChannelInfo>,
    pub controllers: Vec<ControllerInfo>,
    pub parallels: Vec<ParallelInfo>,
    pub panic: Option<PanicModel>,
    pub usbredirs: Vec<UsbredirInfo>,
    pub smartcard: Option<SmartcardMode>,
    pub memballoon: Option<MemballoonModel>,
}
