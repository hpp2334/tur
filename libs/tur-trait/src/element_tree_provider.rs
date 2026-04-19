use crate::DynElement;

pub trait ElementTreeProvider {
    fn root_id(&self) -> Option<u64>;
    fn children_of(&self, id: u64) -> Vec<u64>;
    fn element_for(&self, id: u64) -> &dyn DynElement;
}
