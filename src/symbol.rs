use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct GlobalSymbol {
    pub slot: usize,
    pub is_vector: bool,
    /// Heap index of the vector's first element (when is_vector is true).
    pub vector_base: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct LocalSymbol {
    pub slot: usize,
    pub is_vector: bool,
    /// Size of the vector (when is_vector is true).
    /// The actual heap allocation happens at call time in call_function.
    pub vector_size: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct LocalLayout {
    pub symbols: HashMap<String, LocalSymbol>,
    pub total_slots: usize,
}
