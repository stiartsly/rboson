use crate::core::version;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_def_version() {
        let ver = version::ver();
        let ver_str = version::format_version(ver);
        assert_eq!(ver_str, "MK/1");
    }

    #[test]
    fn test_mk_version() {
        let ver = version::build("MK", 5);
        let ver_str = version::format_version(ver);
        assert_eq!(ver_str, "MK/5");
    }

    #[test]
    fn test_or_version() {
        let ver = version::build("OR", 8);
        let ver_str = version::format_version(ver);
        assert_eq!(ver_str, "OR/8");
    }

    #[test]
    fn test_na_version() {
        let ver_str = version::format_version(0);
        assert_eq!(ver_str, "N/A");
    }
}
