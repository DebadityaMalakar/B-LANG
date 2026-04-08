use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct GlobalSymbol {
    pub slot: usize,
    pub is_vector: bool,
    pub vector_base: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct LocalSymbol {
    pub slot: usize,
    pub is_vector: bool,
    pub vector_base: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct LocalLayout {
    pub symbols: HashMap<String, LocalSymbol>,
    pub total_slots: usize,
}
