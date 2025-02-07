use crate::gpuapi::{GpuAPI, GPU, Card, CardState, Process};
use crate::ps;

pub struct MockGpuAPI {
    cards: Vec<Card>,
}

impl MockGpuAPI {
    pub fn new(cards: Vec<Card>) -> MockGpuAPI {
        MockGpuAPI {
            cards,
        }
    }
}

impl GpuAPI for MockGpuAPI {
    fn probe(&self) -> Option<Box<dyn GPU>> {
        Some(Box::new(MockGpus{ cards: self.cards.clone() }))
    }
}

pub struct MockGpus {
    cards: Vec<Card>
}

impl GPU for MockGpus {
    fn get_manufacturer(&mut self) -> String {
        "Yoyodyne, Inc.".to_string()
    }

    fn get_card_configuration(&mut self) -> Result<Vec<Card>, String> {
        Ok(self.cards.clone())
    }

    fn get_process_utilization(
        &mut self,
        _user_by_pid: &ps::UserTable,
    ) -> Result<Vec<Process>, String> {
        Err("No processes yet".to_string())
    }

    fn get_card_utilization(&mut self) -> Result<Vec<CardState>, String> {
        Err("No utilization yet".to_string())
    }
}
