// Self-contained deployment stages the Windows App Runtime beside the exe, which needs
// Windows' curl.exe and tar.exe, so it only runs on a Windows host build.
fn main() {
    if std::env::var_os("REACTOR_SELF_CONTAINED").is_some() {
        windows_reactor_setup::as_self_contained();
    }
}
