use std::fs;
use std::path::PathBuf;

pub enum RedirectionSource {
    Pipe,
    File(PathBuf),
}

#[cfg(not(unix))]
/// At this time, only operating systems that feature a `proc` filesystem are supported
pub fn source() -> Option<RedirectionSource> { None }

#[cfg(unix)]
/// On UNIX systems that feature a `proc` filesystem, if `/proc/self/fd/0` points to a
/// location other than `/dev/pts` or `pipe:`, then the standard input has been redirected.
///
/// - **/proc/self/fd/0** is the current process's standard input
/// - **/proc/self/fd/1** is the current process's standard output
/// - **/proc/self/fd/2** is the current process's standard error
pub fn source() -> Option<RedirectionSource> {
    if let Ok(link) = fs::read_link("/proc/self/fd/0") {
        let slink = link.to_string_lossy();
        return if slink.starts_with("/dev/pts") {
            None
        } else if slink.starts_with("pipe:") {
            Some(RedirectionSource::Pipe)
        } else {
            Some(RedirectionSource::File(link.clone()))
        };
    }
    None
}
