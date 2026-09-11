// Nautilus
// Copyright (C) 2024  Daniel Teuchert, Cornelius Aschermann, Sergej Schumilo

extern crate byteorder;
extern crate nix;
extern crate snafu;
extern crate tempfile;
extern crate timeout_readwrite;
#[macro_use]
extern crate serde_derive;

pub mod exitreason;
pub mod newtypes;

use nix::fcntl;
use nix::libc::{
    __errno_location, IPC_CREAT, IPC_EXCL, IPC_PRIVATE, IPC_RMID, shmat, shmctl, shmget, strerror,
};
use nix::sys::signal::{self, Signal};
use nix::sys::stat;
use nix::sys::wait::WaitStatus;
use nix::unistd;
use nix::unistd::Pid;
use nix::unistd::{ForkResult, fork};
use std::ffi::CString;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::io::RawFd;

use std::io::BufReader;
use std::ptr;
use std::time::Duration;
use timeout_readwrite::TimeoutReader;

use byteorder::{LittleEndian, ReadBytesExt};
use std::fs::File;

use exitreason::ExitReason;
use newtypes::*;
use snafu::ResultExt;

// This only runs in the forked child; setup failure is fatal, as with the previous
// `nix::unistd::dup2(...).expect(...)` calls.
fn dup2_raw_fd(old: RawFd, new: RawFd, error_message: &str) {
    let res = unsafe { nix::libc::dup2(old, new) };
    if res < 0 {
        panic!("{}: {}", error_message, std::io::Error::last_os_error());
    }
}

pub struct ForkServer {
    inp_file: File,
    ctl_in: File,
    shared_data: *mut [u8],
    st_out: std::io::BufReader<TimeoutReader<File>>,
}

impl ForkServer {
    pub fn new(
        path: String,
        args: Vec<String>,
        hide_output: bool,
        timeout_in_millis: u64,
        bitmap_size: usize,
    ) -> Self {
        let inp_file = tempfile::NamedTempFile::new().expect("couldn't create temp file");
        let (inp_file, in_path) = inp_file
            .keep()
            .expect("couldn't persists temp file for input");
        let inp_file_path = in_path
            .to_str()
            .expect("temp path should be unicode!")
            .to_string();
        let args = args
            .into_iter()
            .map(|s| if s == "@@" { inp_file_path.clone() } else { s })
            .collect::<Vec<_>>();
        let (ctl_out, ctl_in) = nix::unistd::pipe().expect("failed to create ctl_pipe");
        let (st_out, st_in) = nix::unistd::pipe().expect("failed to create st_pipe");
        let (shm_file, shared_data) = ForkServer::create_shm(bitmap_size);

        match unsafe { fork() }.expect("couldn't fork") {
            // Parent returns
            ForkResult::Parent { .. } => {
                unistd::close(ctl_out).expect("coulnd't close ctl_out");
                unistd::close(st_in).expect("coulnd't close st_out");
                let mut st_out = BufReader::new(TimeoutReader::new(
                    File::from(st_out),
                    Duration::from_millis(timeout_in_millis),
                ));
                st_out
                    .read_u32::<LittleEndian>()
                    .expect("couldn't read child hello");
                Self {
                    inp_file,
                    ctl_in: File::from(ctl_in),
                    shared_data,
                    st_out,
                }
            }
            //Child does complex stuff
            ForkResult::Child => {
                let forkserver_fd = 198; // from AFL config.h
                dup2_raw_fd(
                    ctl_out.as_raw_fd(),
                    forkserver_fd,
                    "couldn't dup2 ctl_out to FROKSRV_FD",
                );
                dup2_raw_fd(
                    st_in.as_raw_fd(),
                    forkserver_fd + 1,
                    "couldn't dup2 st_in to FROKSRV_FD+1",
                );

                dup2_raw_fd(inp_file.as_raw_fd(), 0, "couldn't dup2 input file to stdin");
                drop(inp_file);

                unistd::close(ctl_in).expect("couldn't close ctl_in");
                unistd::close(ctl_out).expect("couldn't close ctl_out");
                unistd::close(st_in).expect("couldn't close st_in");
                unistd::close(st_out).expect("couldn't close st_out");

                let path = CString::new(path).expect("binary path must not contain zero");
                let args = args
                    .into_iter()
                    .map(|s| CString::new(s).expect("args must not contain zero"))
                    .collect::<Vec<_>>();

                let shm_id = CString::new(format!("__AFL_SHM_ID={}", shm_file)).unwrap();

                //Asan options: set asan SIG to 223 and disable leak detection
                let asan_settings =
                    CString::new("ASAN_OPTIONS=exitcode=223,abort_on_erro=true,detect_leaks=0")
                        .expect("RAND_2089158993");

                let env = vec![shm_id, asan_settings];

                if hide_output {
                    let null = fcntl::open("/dev/null", fcntl::OFlag::O_RDWR, stat::Mode::empty())
                        .expect("couldn't open /dev/null");
                    dup2_raw_fd(
                        null.as_fd().as_raw_fd(),
                        1,
                        "couldn't dup2 /dev/null to stdout",
                    );
                    dup2_raw_fd(
                        null.as_fd().as_raw_fd(),
                        2,
                        "couldn't dup2 /dev/null to stderr",
                    );
                    unistd::close(null).expect("couldn't close /dev/null");
                }
                println!("EXECVE {:?} {:?} {:?}", path, args, env);
                let _ = unistd::execve(&path, &args, &env);
                panic!("couldn't execve forkserver target");
            }
        }
    }

    pub fn run(&mut self, data: &[u8]) -> Result<ExitReason, SubprocessError> {
        for i in self.get_shared_mut().iter_mut() {
            *i = 0;
        }
        unistd::ftruncate(self.inp_file.as_fd(), 0).context(QemuRunNixSnafu {
            task: "Couldn't truncate inp_file",
        })?;
        unistd::lseek(self.inp_file.as_fd(), 0, unistd::Whence::SeekSet).context(
            QemuRunNixSnafu {
                task: "Couldn't seek inp_file",
            },
        )?;
        unistd::write(self.inp_file.as_fd(), data).context(QemuRunNixSnafu {
            task: "Couldn't write data to inp_file",
        })?;
        unistd::lseek(self.inp_file.as_fd(), 0, unistd::Whence::SeekSet).context(
            QemuRunNixSnafu {
                task: "Couldn't seek inp_file",
            },
        )?;

        unistd::write(self.ctl_in.as_fd(), &[0, 0, 0, 0]).context(QemuRunNixSnafu {
            task: "Couldn't send start command",
        })?;

        let pid = Pid::from_raw(self.st_out.read_i32::<LittleEndian>().context(
            QemuRunIOSnafu {
                task: "Couldn't read target pid",
            },
        )?);

        if let Ok(status) = self.st_out.read_i32::<LittleEndian>() {
            return Ok(ExitReason::from_wait_status(
                WaitStatus::from_raw(pid, status).expect("402104968"),
            ));
        }
        signal::kill(pid, Signal::SIGKILL).context(QemuRunNixSnafu {
            task: "Couldn't kill timed out process",
        })?;
        self.st_out
            .read_u32::<LittleEndian>()
            .context(QemuRunIOSnafu {
                task: "couldn't read timeout exitcode",
            })?;
        Ok(ExitReason::Timeouted)
    }

    pub fn get_shared_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *self.shared_data }
    }
    pub fn get_shared(&self) -> &[u8] {
        unsafe { &*self.shared_data }
    }

    fn create_shm(bitmap_size: usize) -> (i32, *mut [u8]) {
        unsafe {
            let shm_id = shmget(IPC_PRIVATE, bitmap_size, IPC_CREAT | IPC_EXCL | 0o600);
            if shm_id < 0 {
                panic!(
                    "shm_id {:?}",
                    CString::from_raw(strerror(*__errno_location()))
                );
            }

            let trace_bits = shmat(shm_id, ptr::null(), 0);
            if (trace_bits as isize) < 0 {
                panic!(
                    "shmat {:?}",
                    CString::from_raw(strerror(*__errno_location()))
                );
            }

            let res = shmctl(
                shm_id,
                IPC_RMID,
                std::ptr::null_mut::<nix::libc::shmid_ds>(),
            );
            if res < 0 {
                panic!(
                    "shmclt {:?}",
                    CString::from_raw(strerror(*__errno_location()))
                );
            }
            (shm_id, trace_bits as *mut [u8; 1 << 16])
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn run_forkserver() {
        let hide_output = false;
        let timeout_in_millis = 1_000;
        let bitmap_size = 1 << 16;
        let target = "../test".to_string();
        let args = vec![];
        let mut fork = ForkServer::new(target, args, hide_output, timeout_in_millis, bitmap_size);
        assert!(fork.get_shared()[1..].iter().all(|v| *v == 0));
        assert_eq!(
            fork.run(b"deadbeeg").unwrap(),
            exitreason::ExitReason::Normal(0)
        );
        assert_eq!(
            fork.run(b"deadbeef").unwrap(),
            exitreason::ExitReason::Signaled(6)
        );
        assert!(fork.get_shared()[1..].iter().any(|v| *v != 0));
    }
}
