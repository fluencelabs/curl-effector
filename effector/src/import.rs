use marine_rs_sdk::{marine, MountedBinaryResult};

#[marine]
#[host_import]
extern "C" {
    /// Execute provided cmd as a parameters of curl, return result.
    pub fn curl(cmd: Vec<String>) -> MountedBinaryResult;
}
