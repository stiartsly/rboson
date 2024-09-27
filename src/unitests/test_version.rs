use crate::core::version;

#[test]
fn test_def_version() {
    let ver = version::ver();
    let ver_str = version::normailized_version(ver);
    assert_eq!(ver_str, "Meerkat/1");
}

#[test]
fn test_mk_version() {
    let ver = version::build("MK", 5);
    let ver_str = version::normailized_version(ver);
    assert_eq!(ver_str, "Meerkat/5");
}

#[test]
fn test_or_version() {
    let ver = version::build("OR", 8);
    let ver_str = version::normailized_version(ver);
    assert_eq!(ver_str, "Orca/8");
}

#[test]
fn test_na_version() {
    let ver_str = version::normailized_version(0);
    println!("ver_str:{}", ver_str);
    assert_eq!(ver_str, "N/A");
}
