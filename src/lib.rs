use std::fs::{self, File};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use error_chain;
use glob::glob;
use nix::sys::stat::{fstat, SFlag};

/// Newtype pattern to avoid type errors.
/// https://www.gnu.org/software/libc/manual/html_node/Process-Identification.html
#[derive(Debug)]
pub struct Pid(u32);

error_chain::error_chain! {
    foreign_links {
        Io(::std::io::Error);
        Nix(nix::Error);
    }
}

macro_rules! unwrap_or_continue {
    ($e:expr) => {{
        if let Ok(x) = $e {
            x
        } else {
            continue;
        }
    }};
}

//fn extract_pid_from_proc<P: AsRef<Path>>(proc_entry: P) -> Result<Pid> {
//
//    let vec: Vec<&str> = proc_entry.as_os_string()?
//                        .split('/')
//                        .collect();
//
//    eprintln!("vec: {:?}", vec);
//    Ok(Pid(0))
////                        .collect::<Vec<&str>>()[2]
////                        .parse::<u32>().unwrap();
////            pids.push(Pid(pid));
//}

/// Given a file path, return the process id of any processes that have an open file descriptor
/// pointing to the given file.
pub fn opath<P: AsRef<Path>>(path: P) -> Result<Vec<Pid>> {
    let mut path_buf = PathBuf::new();
    path_buf.push(path);
    let mut pids: Vec<Pid> = Vec::new();
    let stat_info = nix::sys::stat::lstat(&path_buf)?;

    let mut target_path = PathBuf::new();
    target_path.push(fs::canonicalize(&path_buf)?);

    // FIXME: not sure what the *right* way to do this is. Revisit later.
    if SFlag::S_IFMT.bits() & stat_info.st_mode == SFlag::S_IFREG.bits() {
        eprintln!("stat info reg file: {:?}", stat_info.st_mode);
    } else if SFlag::S_IFMT.bits() & stat_info.st_mode == SFlag::S_IFSOCK.bits() {
        eprintln!("stat info socket file: {:?}", stat_info.st_mode);
    }

    for entry in glob("/proc/*/fd/*").expect("Failed to read glob pattern") {
        let e = unwrap_or_continue!(entry);
        let real = unwrap_or_continue!(fs::read_link(&e));

        if real == target_path {
            let pbuf = e.to_str().unwrap().split('/').collect::<Vec<&str>>()[2];
            let pid = unwrap_or_continue!(pbuf.parse::<u32>());
            pids.push(Pid(pid));
            eprintln!("process: {:?} -> real: {:?}", pid, real);
        }
    }

    Ok(pids)
}

#[cfg(test)]
mod tests {
    use super::opath;
    use std::fs::File;
    use std::io::Write;
    use std::thread;
    use std::time::Duration;

    use nix::unistd::{fork, ForkResult};
    use rusty_fork::rusty_fork_id;
    use rusty_fork::rusty_fork_test;
    use rusty_fork::rusty_fork_test_name;
    use tempfile::NamedTempFile;

    // TODO: test symlink, socket file, directory, fifo

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn test_ofile_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "T").unwrap();

        let p = file.path();

        let ofile_pid = opath(p).unwrap().pop().unwrap();

        assert_eq!(ofile_pid.0, std::process::id());
    }

    rusty_fork_test! {
    #[test]
    fn test_ofile_other_process() {
        let path = "/tmp/.opath_tmp";

        match fork() {
            Ok(ForkResult::Parent { child, .. }) => {
                eprintln!("Child pid: {}", child);
                let pid = opath(&path).unwrap().pop().unwrap();

                assert_eq!(pid.0, child.as_raw() as u32);
            },
            Ok(ForkResult::Child) => {
                let mut f = File::create(&path).unwrap();
                writeln!(f, "test").unwrap();

                thread::sleep(Duration::from_millis(100));
            },
            Err(_) => panic!("Fork failed"),
        }
    }
    }
}
