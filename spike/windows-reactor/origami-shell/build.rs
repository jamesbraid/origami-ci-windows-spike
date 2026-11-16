// Stages a private Windows App Runtime beside the exe, so the app runs with no installed
// runtime. Needs Windows' curl.exe and tar.exe, so it only runs on a Windows host.
fn main() {
    if std::env::var_os("REACTOR_FRAMEWORK_DEPENDENT").is_none() {
        windows_reactor_setup::as_self_contained();
    }
}
