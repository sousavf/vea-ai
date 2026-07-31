use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HostInfo {
    app_version: &'static str,
    protocol_version: u8,
    security_boundary: &'static str,
}

#[tauri::command]
fn get_host_info() -> HostInfo {
    HostInfo {
        app_version: env!("CARGO_PKG_VERSION"),
        protocol_version: 1,
        security_boundary: "rust-host",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_host_info])
        .run(tauri::generate_context!())
        .expect("failed to run Vea desktop host");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_info_exposes_protocol_without_privileged_handles() {
        let info = get_host_info();
        assert_eq!(info.protocol_version, 1);
        assert_eq!(info.security_boundary, "rust-host");
    }
}
