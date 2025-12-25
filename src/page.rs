#![allow(dead_code)]

use serde::Serialize;

#[derive(Serialize)]
pub struct Page<T: Serialize> {
    pub total: u64,
    pub items: Vec<T>,
}

impl<T: Serialize> Page<T> {
    pub fn new(total: u64, items: Vec<T>) -> Self {
        Self { total, items }
    }
}
