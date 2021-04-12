//! Traits extending the process handling.
//!
use std::convert::From;
use std::process::Child;
use std::process::ExitStatus;
#[cfg(target_family = "windows")]
use std::thread::sleep;
#[cfg(target_family = "windows")]
use std::time::Duration;

/// Newtype wrapping some Child.
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
    pub fn wait(&mut self) -> Result<ExitStatus, std::io::Error> {
        self.0.wait()
    }

    #[cfg(target_family = "windows")]
    pub fn wait(&mut self) -> Result<ExitStatus, anyhow::Error> {
        // This might return immediately.
        let exit_status = self.0.wait()?;
        if !exit_status.success() {
            return Ok(exit_status);
        };
        // We check if the Window is still open and wait eventually
        // a bit longer.
        // TODO
        loop {
            //println!("waiting ...");
            sleep(Duration::from_millis(1000));

            // TODO: check if the browser Window is still open.
            if false {
                // We do nothing, just continue waiting.
                //println!("waiting ...");
            } else {
                break;
            };
        }
        Ok(exit_status)
    }
}
