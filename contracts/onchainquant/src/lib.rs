#![no_std]

#[cfg(not(feature = "binary-vendor"))]
mod contract;

#[cfg(test)]
mod tests;
