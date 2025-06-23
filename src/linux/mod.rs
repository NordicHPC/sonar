#[cfg(test)]
pub mod mocksystem;
mod procfs;
#[cfg(test)]
mod procfs_test;
mod procfsapi;
mod slurm;
#[cfg(test)]
mod slurm_test;
pub mod system;
