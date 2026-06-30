use anyhow::Result;
use serde::Serialize;

const KNOWN_TARGET_PORTS: &[u16] = &[30031, 10012, 30230];
#[cfg(windows)]
const TCP_STATE_ESTAB: u32 = 5;

#[derive(Debug, Clone, Serialize)]
pub struct CaptureDoctorReport {
    pub windows: bool,
    pub admin: bool,
    pub exe: String,
    pub pid: Option<u32>,
    pub ports: Vec<u16>,
    pub pppoe_detection: PppoeDetection,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PppoeDetection {
    pub detected: bool,
    pub sources: Vec<PppoeDetectionSource>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PppoeDetectionSource {
    pub source: String,
    pub name: Option<String>,
    pub detail: String,
}

pub fn capture_doctor(exe: &str) -> Result<CaptureDoctorReport> {
    let pid = find_process_pid(exe)?;
    let ports = pid.map(candidate_ports).transpose()?.unwrap_or_default();
    let pppoe_detection = detect_pppoe();
    let mut notes = Vec::new();
    if !cfg!(windows) {
        notes.push("pktmon capture requires Windows".to_string());
    }
    if cfg!(windows) && !is_admin() {
        notes.push("pktmon capture requires administrator privilege".to_string());
    }
    if pid.is_none() {
        notes.push(format!("{exe} not found"));
    }
    if pppoe_detection.detected {
        notes.push("PPPoE detected; pktmon capture will use no capture filters".to_string());
    }
    notes.extend(
        pppoe_detection
            .errors
            .iter()
            .map(|error| format!("PPPoE detection error: {error}")),
    );
    Ok(CaptureDoctorReport {
        windows: cfg!(windows),
        admin: is_admin(),
        exe: exe.to_string(),
        pid,
        ports,
        pppoe_detection,
        notes,
    })
}

#[cfg(not(windows))]
pub fn detect_pppoe() -> PppoeDetection {
    PppoeDetection::default()
}

#[cfg(windows)]
pub fn detect_pppoe() -> PppoeDetection {
    let mut detection = PppoeDetection::default();
    if let Err(error) = detect_pppoe_ras(&mut detection) {
        detection.errors.push(format!("RAS: {error}"));
    }
    if let Err(error) = detect_pppoe_adapters(&mut detection) {
        detection.errors.push(format!("adapters: {error}"));
    }
    detection.detected = !detection.sources.is_empty();
    detection
}

#[cfg(not(windows))]
pub fn is_admin() -> bool {
    false
}

#[cfg(windows)]
pub fn is_admin() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

#[cfg(not(windows))]
pub fn find_process_pid(_exe: &str) -> Result<Option<u32>> {
    Ok(None)
}

#[cfg(not(windows))]
pub fn find_process_pids(_exe: &str) -> Result<Vec<u32>> {
    Ok(Vec::new())
}

#[cfg(windows)]
pub fn find_process_pid(exe_name: &str) -> Result<Option<u32>> {
    Ok(find_process_pids(exe_name)?.into_iter().next())
}

#[cfg(windows)]
pub fn find_process_pids(exe_name: &str) -> Result<Vec<u32>> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Ok(Vec::new());
        }
        let mut entry = std::mem::zeroed::<PROCESSENTRY32W>();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = Vec::new();
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let name = wide_z_to_string(&entry.szExeFile);
                if name.eq_ignore_ascii_case(exe_name)
                    || name.trim_end_matches(".exe").eq_ignore_ascii_case(exe_name)
                {
                    found.push(entry.th32ProcessID);
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        Ok(found)
    }
}

#[cfg(not(windows))]
pub fn candidate_ports(_pid: u32) -> Result<Vec<u16>> {
    Ok(KNOWN_TARGET_PORTS.to_vec())
}

#[cfg(windows)]
pub fn candidate_ports(pid: u32) -> Result<Vec<u16>> {
    let mut ports = KNOWN_TARGET_PORTS.to_vec();
    for conn in tcp_table()? {
        if conn.pid != pid || is_localhost(&conn.remote_ip) {
            continue;
        }
        if conn.remote_port == 443 {
            continue;
        }
        if !ports.contains(&conn.remote_port) {
            ports.push(conn.remote_port);
        }
    }
    for endpoint in udp_table()? {
        if endpoint.pid == pid && !ports.contains(&endpoint.local_port) {
            ports.push(endpoint.local_port);
        }
    }
    ports.sort_unstable();
    ports.dedup();
    Ok(ports)
}

#[cfg(windows)]
pub fn limited_filter_ports(ports: &[u16]) -> Vec<u16> {
    let mut selected = ports.to_vec();
    selected.sort_unstable();
    selected.dedup();
    if selected.len() > 16 {
        selected.truncate(16);
    }
    selected
}

#[cfg(windows)]
#[derive(Debug, Clone)]
struct TcpConnection {
    remote_ip: String,
    remote_port: u16,
    pid: u32,
}

#[cfg(windows)]
#[derive(Debug, Clone)]
struct UdpEndpoint {
    local_port: u16,
    pid: u32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct MibTcpRowOwnerPid {
    state: u32,
    local_addr: u32,
    local_port: u32,
    remote_addr: u32,
    remote_port: u32,
    owning_pid: u32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct MibTcp6RowOwnerPid {
    local_addr: [u8; 16],
    local_scope_id: u32,
    local_port: u32,
    remote_addr: [u8; 16],
    remote_scope_id: u32,
    remote_port: u32,
    state: u32,
    owning_pid: u32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct MibUdpRowOwnerPid {
    local_addr: u32,
    local_port: u32,
    owning_pid: u32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct MibUdp6RowOwnerPid {
    local_addr: [u8; 16],
    local_scope_id: u32,
    local_port: u32,
    owning_pid: u32,
}

#[cfg(windows)]
fn tcp_table() -> Result<Vec<TcpConnection>> {
    let mut rows = Vec::new();
    rows.extend(query_tcp_v4()?);
    rows.extend(query_tcp_v6()?);
    Ok(rows)
}

#[cfg(windows)]
fn udp_table() -> Result<Vec<UdpEndpoint>> {
    let mut rows = Vec::new();
    rows.extend(query_udp_v4()?);
    rows.extend(query_udp_v6()?);
    Ok(rows)
}

#[cfg(windows)]
fn query_tcp_v4() -> Result<Vec<TcpConnection>> {
    let bytes =
        query_extended_tcp_table(windows_sys::Win32::Networking::WinSock::AF_INET as u32, 5)?;
    Ok(read_rows::<MibTcpRowOwnerPid>(&bytes)
        .into_iter()
        .filter(|row| row.state == TCP_STATE_ESTAB)
        .map(|row| TcpConnection {
            remote_ip: ipv4(row.remote_addr),
            remote_port: port(row.remote_port),
            pid: row.owning_pid,
        })
        .collect())
}

#[cfg(windows)]
fn query_tcp_v6() -> Result<Vec<TcpConnection>> {
    let bytes =
        query_extended_tcp_table(windows_sys::Win32::Networking::WinSock::AF_INET6 as u32, 5)?;
    Ok(read_rows::<MibTcp6RowOwnerPid>(&bytes)
        .into_iter()
        .filter(|row| row.state == TCP_STATE_ESTAB)
        .map(|row| TcpConnection {
            remote_ip: std::net::Ipv6Addr::from(row.remote_addr).to_string(),
            remote_port: port(row.remote_port),
            pid: row.owning_pid,
        })
        .collect())
}

#[cfg(windows)]
fn query_udp_v4() -> Result<Vec<UdpEndpoint>> {
    let bytes =
        query_extended_udp_table(windows_sys::Win32::Networking::WinSock::AF_INET as u32, 1)?;
    Ok(read_rows::<MibUdpRowOwnerPid>(&bytes)
        .into_iter()
        .map(|row| UdpEndpoint {
            local_port: port(row.local_port),
            pid: row.owning_pid,
        })
        .collect())
}

#[cfg(windows)]
fn query_udp_v6() -> Result<Vec<UdpEndpoint>> {
    let bytes =
        query_extended_udp_table(windows_sys::Win32::Networking::WinSock::AF_INET6 as u32, 1)?;
    Ok(read_rows::<MibUdp6RowOwnerPid>(&bytes)
        .into_iter()
        .map(|row| UdpEndpoint {
            local_port: port(row.local_port),
            pid: row.owning_pid,
        })
        .collect())
}

#[cfg(windows)]
fn query_extended_tcp_table(af: u32, table_class: i32) -> Result<Vec<u8>> {
    use windows_sys::Win32::NetworkManagement::IpHelper::GetExtendedTcpTable;

    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
    const NO_ERROR: u32 = 0;

    unsafe {
        let mut size = 0_u32;
        let first = GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, af, table_class, 0);
        if first != ERROR_INSUFFICIENT_BUFFER && first != NO_ERROR {
            anyhow::bail!("GetExtendedTcpTable size failed: {first}");
        }
        let mut bytes = vec![0_u8; size as usize];
        let ret = GetExtendedTcpTable(bytes.as_mut_ptr().cast(), &mut size, 0, af, table_class, 0);
        if ret != NO_ERROR {
            anyhow::bail!("GetExtendedTcpTable failed: {ret}");
        }
        bytes.truncate(size as usize);
        Ok(bytes)
    }
}

#[cfg(windows)]
fn query_extended_udp_table(af: u32, table_class: i32) -> Result<Vec<u8>> {
    use windows_sys::Win32::NetworkManagement::IpHelper::GetExtendedUdpTable;

    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
    const NO_ERROR: u32 = 0;

    unsafe {
        let mut size = 0_u32;
        let first = GetExtendedUdpTable(std::ptr::null_mut(), &mut size, 0, af, table_class, 0);
        if first != ERROR_INSUFFICIENT_BUFFER && first != NO_ERROR {
            anyhow::bail!("GetExtendedUdpTable size failed: {first}");
        }
        let mut bytes = vec![0_u8; size as usize];
        let ret = GetExtendedUdpTable(bytes.as_mut_ptr().cast(), &mut size, 0, af, table_class, 0);
        if ret != NO_ERROR {
            anyhow::bail!("GetExtendedUdpTable failed: {ret}");
        }
        bytes.truncate(size as usize);
        Ok(bytes)
    }
}

#[cfg(windows)]
fn detect_pppoe_ras(detection: &mut PppoeDetection) -> Result<()> {
    use windows_sys::Win32::NetworkManagement::Rras::{
        ERROR_BUFFER_TOO_SMALL, RASCONNW, RasEnumConnectionsW,
    };

    unsafe {
        let entry_size = std::mem::size_of::<RASCONNW>() as u32;
        let mut size = entry_size;
        let mut count = 0_u32;
        let mut entries = vec![RASCONNW::default(); 1];
        entries[0].dwSize = entry_size;

        let mut ret = RasEnumConnectionsW(entries.as_mut_ptr(), &mut size, &mut count);
        if ret == ERROR_BUFFER_TOO_SMALL {
            let capacity = (size as usize)
                .div_ceil(std::mem::size_of::<RASCONNW>())
                .max(1);
            entries = vec![RASCONNW::default(); capacity];
            entries[0].dwSize = entry_size;
            ret = RasEnumConnectionsW(entries.as_mut_ptr(), &mut size, &mut count);
        }
        if ret != 0 {
            anyhow::bail!("RasEnumConnectionsW failed: {ret}");
        }

        for entry in entries.iter().take(count as usize) {
            let device_type = wide_z_to_string(&entry.szDeviceType);
            let device_name = wide_z_to_string(&entry.szDeviceName);
            let entry_name = wide_z_to_string(&entry.szEntryName);
            if is_pppoe_text(&device_type)
                || is_pppoe_text(&device_name)
                || is_pppoe_text(&entry_name)
            {
                detection.sources.push(PppoeDetectionSource {
                    source: "ras".to_string(),
                    name: non_empty(entry_name).or_else(|| non_empty(device_name.clone())),
                    detail: format!("device_type={device_type}, device_name={device_name}"),
                });
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn detect_pppoe_adapters(detection: &mut PppoeDetection) -> Result<()> {
    use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_INCLUDE_ALL_INTERFACES, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
        GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses, IF_TYPE_PPP, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;

    const AF_UNSPEC: u32 = 0;
    let flags = GAA_FLAG_INCLUDE_ALL_INTERFACES
        | GAA_FLAG_SKIP_ANYCAST
        | GAA_FLAG_SKIP_DNS_SERVER
        | GAA_FLAG_SKIP_MULTICAST;

    unsafe {
        let mut size = 15_000_u32;
        let mut bytes = Vec::new();
        let ret = loop {
            bytes.resize(size as usize, 0);
            let ret = GetAdaptersAddresses(
                AF_UNSPEC,
                flags,
                std::ptr::null(),
                bytes.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>(),
                &mut size,
            );
            if ret == ERROR_BUFFER_OVERFLOW {
                continue;
            }
            break ret;
        };
        if ret != ERROR_SUCCESS {
            anyhow::bail!("GetAdaptersAddresses failed: {ret}");
        }

        let mut current = bytes.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        while !current.is_null() {
            let adapter = &*current;
            let friendly = pwstr_to_string(adapter.FriendlyName);
            let description = pwstr_to_string(adapter.Description);
            let is_up = adapter.OperStatus == IfOperStatusUp;
            let has_pppoe_name = is_pppoe_text(&friendly) || is_pppoe_text(&description);
            if is_up && adapter.IfType == IF_TYPE_PPP && has_pppoe_name {
                detection.sources.push(PppoeDetectionSource {
                    source: "adapter".to_string(),
                    name: non_empty(friendly.clone()).or_else(|| non_empty(description.clone())),
                    detail: format!(
                        "if_type={}, oper_status={}, friendly_name={}, description={}",
                        adapter.IfType, adapter.OperStatus, friendly, description
                    ),
                });
            }
            current = adapter.Next;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn read_rows<T: Copy>(bytes: &[u8]) -> Vec<T> {
    if bytes.len() < 4 {
        return Vec::new();
    }
    let count = u32::from_ne_bytes(bytes[0..4].try_into().expect("4 bytes")) as usize;
    let row_size = std::mem::size_of::<T>();
    let mut rows = Vec::new();
    for index in 0..count {
        let offset = 4 + index * row_size;
        let Some(row_bytes) = bytes.get(offset..offset + row_size) else {
            break;
        };
        let row = unsafe { std::ptr::read_unaligned(row_bytes.as_ptr().cast::<T>()) };
        rows.push(row);
    }
    rows
}

#[cfg(windows)]
fn port(value: u32) -> u16 {
    u16::from_be(value as u16)
}

#[cfg(windows)]
fn ipv4(value: u32) -> String {
    std::net::Ipv4Addr::from(value.to_le_bytes()).to_string()
}

#[cfg(windows)]
fn is_localhost(ip: &str) -> bool {
    ip == "::1" || ip == "127.0.0.1" || ip.starts_with("127.")
}

#[cfg(windows)]
fn wide_z_to_string(value: &[u16]) -> String {
    let len = value.iter().position(|ch| *ch == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..len])
}

#[cfg(windows)]
fn pwstr_to_string(value: windows_sys::core::PWSTR) -> String {
    if value.is_null() {
        return String::new();
    }
    unsafe {
        let mut len = 0_usize;
        while *value.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(value, len))
    }
}

#[cfg(windows)]
fn is_pppoe_text(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("pppoe") || lower.contains("raspppoe")
}

#[cfg(windows)]
fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}
