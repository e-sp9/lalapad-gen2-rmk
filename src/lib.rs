#![cfg_attr(not(test), no_std)]

pub mod iqs9151;
pub mod rgb_widget;

#[cfg(not(target_arch = "arm"))]
pub(crate) mod host_test_stubs;

#[cfg(test)]
mod zmk_parity_tests;
