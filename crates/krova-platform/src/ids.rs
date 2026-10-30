//! Parsing of OS monitor identifiers into the stable ids `krova-core` matches on.

/// Extract the PnP hardware id (manufacturer + product, e.g. `DELA0B7`) from a
/// monitor device interface name or device id:
///
/// - `\\?\DISPLAY#DELA0B7#5&2d2b8a8f&0&UID4353#{e6f07b5f-...}`
/// - `MONITOR\DELA0B7\{4d36e96e-...}\0001`
///
/// Generic fallbacks such as `Default_Monitor` carry no identity and give `None`.
/// Two identical models share this id; `krova-core` tells them apart by the full
/// interface name (the device path).
pub fn monitor_hardware_id(interface: &str) -> Option<String> {
    let part = if interface.contains('#') {
        interface.split('#').nth(1)?
    } else {
        let mut parts = interface.split('\\');
        parts.find(|p| p.eq_ignore_ascii_case("MONITOR"))?;
        parts.next()?
    };
    let id = part.trim();
    if id.is_empty() || id.eq_ignore_ascii_case("Default_Monitor") {
        return None;
    }
    Some(id.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_name_form() {
        let s = r"\\?\DISPLAY#DELA0B7#5&2d2b8a8f&0&UID4353#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}";
        assert_eq!(monitor_hardware_id(s).as_deref(), Some("DELA0B7"));
    }

    #[test]
    fn device_id_form() {
        let s = r"MONITOR\lgd0517\{4d36e96e-e325-11ce-bfc1-08002be10318}\0001";
        assert_eq!(monitor_hardware_id(s).as_deref(), Some("LGD0517"));
    }

    #[test]
    fn generic_and_garbage_ids_are_rejected() {
        assert_eq!(
            monitor_hardware_id(r"\\?\DISPLAY#Default_Monitor#4&1#{guid}"),
            None
        );
        assert_eq!(
            monitor_hardware_id(r"MONITOR\Default_Monitor\{guid}\0001"),
            None
        );
        assert_eq!(monitor_hardware_id(""), None);
        assert_eq!(monitor_hardware_id("DISPLAY"), None);
        assert_eq!(monitor_hardware_id(r"\\?\DISPLAY##x#{guid}"), None);
    }
}
