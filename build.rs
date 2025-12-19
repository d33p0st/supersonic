
fn main() {
    let rustc_version = rustc_version::version_meta().unwrap();
    match rustc_version.channel {
        rustc_version::Channel::Nightly => {}
        _ => panic!("This crate requires a nightly compiler to build."),
    }
}