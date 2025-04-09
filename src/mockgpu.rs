use crate::gpuapi::{Card, CardState, GpuAPI, Process, GPU};
use crate::ps;

pub struct MockGpuAPI {
    cards: Vec<Card>,
}

impl MockGpuAPI {
    pub fn new(cards: Vec<Card>) -> MockGpuAPI {
        MockGpuAPI { cards }
    }
}

impl GpuAPI for MockGpuAPI {
    fn probe(&self) -> Option<Box<dyn GPU>> {
        Some(Box::new(MockGpus {
            cards: self.cards.clone(),
        }))
    }
}

pub struct MockGpus {
    cards: Vec<Card>,
}

impl GPU for MockGpus {
    fn get_manufacturer(&self) -> String {
        "Yoyodyne, Inc.".to_string()
    }

    fn get_card_configuration(&self) -> Result<Vec<Card>, String> {
        Ok(self.cards.clone())
    }

    fn get_process_utilization(&self, _ptable: &ps::ProcessTable) -> Result<Vec<Process>, String> {
        Err("No processes yet".to_string())
    }

    fn get_card_utilization(&self) -> Result<Vec<CardState>, String> {
        Err("No utilization yet".to_string())
    }
}
