// SPDX-License-Identifier: GPL-2.0-or-later
//! This branch supports exactly the PHN16-71 that was hardware-tested.
#![forbid(unsafe_code)]
pub(crate) fn supported(vendor: &str, product: &str) -> bool {
    vendor == "Acer" && product == "Predator PHN16-71"
}
#[cfg(test)]
mod tests {
    #[test]
    fn reject_other_models_and_vendors() {
        assert!(super::supported("Acer", "Predator PHN16-71"));
        for (vendor, product) in [
            ("Acer", "Predator PHN16-72"),
            ("Acer", "Nitro AN16-71"),
            ("Other", "Predator PHN16-71"),
        ] {
            assert!(!super::supported(vendor, product));
        }
    }
}
