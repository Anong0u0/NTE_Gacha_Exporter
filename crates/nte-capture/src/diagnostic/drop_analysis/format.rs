#[cfg(any(windows, test))]
fn is_vlan_ethertype(value: u16) -> bool {
    matches!(value, 0x8100 | 0x88a8 | 0x9100)
}

#[cfg(any(windows, test))]
fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]))
}

#[cfg(any(windows, test))]
fn hex_u16(value: u16) -> String {
    format!("0x{value:04x}")
}

#[cfg(any(windows, test))]
fn hex_u8(value: u8) -> String {
    format!("0x{value:02x}")
}

#[cfg(any(windows, test))]
fn ip_protocol_name(value: u8) -> String {
    match value {
        6 => "tcp".to_string(),
        17 => "udp".to_string(),
        _ => hex_u8(value),
    }
}

#[cfg(any(windows, test))]
fn prefix_hex(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
