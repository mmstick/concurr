use std::io;
use libc;
use libc::*;
use std::mem;
use std::ptr;

pub(crate) fn signal(signal: i32, handler: extern "C" fn(i32)) -> io::Result<()> {
    if unsafe { libc::signal(signal as c_int, handler as sighandler_t) } == SIG_ERR {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(crate) fn block() {
    unsafe {
        let mut sigset = mem::uninitialized::<sigset_t>();
        sigemptyset(&mut sigset as *mut sigset_t);
        sigaddset(&mut sigset as *mut sigset_t, SIGTSTP);
        sigaddset(&mut sigset as *mut sigset_t, SIGTTOU);
        sigaddset(&mut sigset as *mut sigset_t, SIGTTIN);
        sigaddset(&mut sigset as *mut sigset_t, SIGCHLD);
        sigprocmask(SIG_BLOCK, &sigset as *const sigset_t, ptr::null_mut() as *mut sigset_t);
    }
}
