#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementNodeId(u64);

impl ElementNodeId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}
