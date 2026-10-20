// SPDX-License-Identifier: GPL-2.0-or-later
//! Kernel ABI declarations generated from the exact target kernel headers.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]
#![allow(dead_code, improper_ctypes, missing_docs, unreachable_pub)]
// Bindgen helpers explicitly require unsafe callers; generated code predates the
// kernel's deny(unsafe_op_in_unsafe_fn) lint. Authored driver code keeps that lint.
#![allow(unsafe_op_in_unsafe_fn)]
include!("bindings_generated.rs");
