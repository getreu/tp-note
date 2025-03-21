//! Module extending the process handling.

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

/// Extension trait with a method that waits under Windows not only the started
/// process, but also all subprocesses as far as they are known.
pub trait ChildExt {
    fn wait_subprocess(&mut self) -> std::io::Result<ExitStatus>;
}

impl ChildExt for Child {
    #[cfg(not(target_family = "windows"))]
    #[inline]
    /// Windows: This `wait()` implementation not only waits until the `Child`
    /// process terminates, it also waits until all its subprocesses terminate.
    /// All other OS: Just executes the usual `wait()` method.
    fn wait_subprocess(&mut self) -> std::io::Result<ExitStatus> {
        // Remember ID for debugging.
        let process_id = self.id();
        log::debug!("Process started: id={}", process_id);

        let exit_status = self.wait();

        log::debug!(
            "Process terminated: id={}, {}",
            process_id,
            match &exit_status {
                Ok(ex_st) => ex_st.to_string(),
                Err(e) => e.to_string(),
            }
        );

        exit_status
    }

    /// Windows: This `wait()` implementation not only waits until the `Child`
    /// process terminates, it also waits until all its subprocesses terminate.
    /// All other OS: Just executes the usual `wait()` method.
    #[cfg(target_family = "windows")]
    fn wait_subprocess(&mut self) -> std::io::Result<ExitStatus> {
        // Initialize the job monitor via a job handle.
        fn wait_init(me: &Child) -> Result<Job, Box<dyn std::error::Error>> {
            // We create a job to monitor the wrapped child.
            let job = Job::create()?;
            let handle = me.as_raw_handle();
            job.assign_process(handle as isize)?;
            Ok(job)
        }

        // At this point, the parent process just terminated. We here we wait
        // for the children and grandchildren also. When all grandchildren
        // terminated, the `process_id_list` will be 0.
        fn wait_more(me: &Child, job: Job) -> Result<(), Box<dyn std::error::Error>> {
            let ids = job.query_process_id_list()?;
            if ids.len() > 0 {
                log::debug!(
                    "Processes id={} launched still running ids:{:?}.",
                    me.id(),
                    ids
                );
            }
            // Wait until all will have terminated.
            while job.query_process_id_list()?.len() > 0 {
                sleep(Duration::from_millis(PROCESS_POLLING_INTERVAL));
            }

            if ids.len() > 0 {
                log::debug!("All processes launched by id={} terminated.", me.id());
            };

            Ok(())
        }

        // Remember ID for debugging.
        let process_id = self.id();
        log::debug!("Process started: id={}", process_id);
        let job = wait_init(&self);

        // For most browsers under Windows, this might most likely returns
        // immediately. The `Child` terminates, after having launched processes
        // himself.
        let exit_status = self.wait();
        if exit_status.is_err() {
            return exit_status;
        };
        log::debug!("Process terminated: id={}, {:?}", process_id, exit_status);

        // Wait for subprocesses to finish.
        match job {
            Ok(job) => {
                if let Err(e) = wait_more(&self, job) {
                    log::debug!("Error handling job list: {}", e);
                }
            }
            Err(e) => log::debug!("Error initializing job list: {}", e),
        }
        exit_status
    }
}
