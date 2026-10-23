use crate::version;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_def_version() {
        let ver = version::ver();
        let ver_str = version::format_version(ver);
        assert_eq!(ver_str, "Meerkat/1");
    }

    #[test]
    fn test_mk_version() {
        let ver = version::build("MK", 5).unwrap();
        let ver_str = version::format_version(ver);
        assert_eq!(ver_str, "Meerkat/5");
    }

    #[test]
    fn test_or_version() {
        let ver = version::build("OR", 8).unwrap();
        let ver_str = version::format_version(ver);
        assert_eq!(ver_str, "Orca/8");
    }

    #[test]
    fn test_na_version() {
        let ver_str = version::format_version(0);
        assert_eq!(ver_str, "N/A");
    }

    #[test]
    fn builds_and_formats_versions() {
        let version = version::build("MK", 5).unwrap();
        assert_eq!(version::format_version(version), "Meerkat/5");

        let version = version::build("OR", 8).unwrap();
        assert_eq!(version::format_version(version), "Orca/8");
        assert_eq!(version::format_version(0), version::VERSION_NOT_AVAILABLE);
    }

    #[test]
    fn rejects_short_names() {
        assert!(version::build("M", 1).is_err());
    }

    #[test]
    fn sanitizes_non_printable_names() {
        let version = (b'\n' as i32) << 24 | (b'\x1b' as i32) << 16 | 3;
        assert_eq!(version::format_version(version), "../3");
    }
}
