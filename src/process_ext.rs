//! Module extending the process handling.
//!
use std::convert::From;
#[cfg(target_family = "windows")]
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use std::process::ExitStatus;
#[cfg(target_family = "windows")]
use std::thread::sleep;
#[cfg(target_family = "windows")]
use std::time::Duration;
#[cfg(target_family = "windows")]
use win32job::Job;

/// Polling interval when waiting for grand children to terminate.
#[cfg(target_family = "windows")]
const PROCESS_POLLING_INTERVAL: u64 = 1000;

/// Newtype wrapping some `Child`.
/// The wrapper "overloads" the `Child::wait()` function when compiled for Windows.
#[derive(Debug)]
pub struct ChildExt(Child);

impl From<Child> for ChildExt {
    fn from(inner: Child) -> Self {
        Self(inner)
    }
}

impl ChildExt {
    #[cfg(not(target_family = "windows"))]
    #[inline]
    pub fn wait(&mut self) -> Result<ExitStatus, anyhow::Error> {
        // Remember ID for debugging.
        let process_id = self.0.id();
        log::info!("Process started: id={}", process_id);

        // Under Unix `wait()` should also wait for the termination of all grand children.
        let exit_status = self.0.wait();

        log::info!(
            "Process terminated: id={}, {}",
            process_id,
            match &exit_status {
                Ok(ex_st) => ex_st.to_string(),
                Err(e) => e.to_string(),
            }
        );

        Ok(exit_status?)
    }

    /// This `wait()` implementation not only waits until the `Child` process
    /// terminates, it also waits until all its subprocesses terminate.
    #[cfg(target_family = "windows")]
    pub fn wait(&mut self) -> Result<ExitStatus, anyhow::Error> {
        // Remember ID for debugging.
        let process_id = self.0.id();
        log::debug!("Process started: id={}", process_id);

        // We create a job to monitor the wrapped child.
        let job = Job::create()?;
        let handle = self.0.as_raw_handle();
        job.assign_process(handle)?;

        // Under Windows, this might most likely returns immediately. The
        // child terminates, after having launched processes himself.
        let exit_status = self.0.wait()?;

        log::debug!("Process terminated: id={}, {}", process_id, exit_status);

        if !exit_status.success() {
            return Ok(exit_status);
        };

        // We check if the Window is still open and wait eventually a bit
        // longer. We check this by counting all process, the grandchildren
        // included.
        // When all grandchildren terminated, this will be 0.
        let ids = job.query_process_id_list()?;
        if ids.len() > 0 {
            log::debug!(
                "Processes id={} launched still running ids:{:?}.",
                process_id,
                ids
            );
        }
        // Wait until all will have terminated.
        while job.query_process_id_list()?.len() > 0 {
            sleep(Duration::from_millis(PROCESS_POLLING_INTERVAL));
        }

        if ids.len() > 0 {
            log::debug!("All processes launched by id={} terminated.", process_id);
        };

        Ok(exit_status)
    }
}
