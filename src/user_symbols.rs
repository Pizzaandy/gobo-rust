use crate::chunked_index_vec::ChunkedIndexVec;
use crate::source_text::TextSpan;
use crate::typed_index;
use crate::typed_index::TypedIndex;
use std::collections::HashMap;
use std::hash::Hash;

typed_index!(pub struct IdentifierId(u32));
typed_index!(pub struct StringLiteralId(u32));
typed_index!(pub struct NumberLiteralId(u32));

// Storage for unique values like identifiers and literals used during compilation
pub struct UserSymbols {
    pub identifiers: UniqueChunkedIndexVec<TextSpan, IdentifierId>,
    pub string_literals: UniqueChunkedIndexVec<TextSpan, StringLiteralId>,
    pub number_literals: UniqueChunkedIndexVec<TextSpan, NumberLiteralId>,
}

impl UserSymbols {
    pub fn new() -> Self {
        Self {
            identifiers: UniqueChunkedIndexVec::new(),
            string_literals: UniqueChunkedIndexVec::new(),
            number_literals: UniqueChunkedIndexVec::new(),
        }
    }
}

pub struct UniqueChunkedIndexVec<T, I: TypedIndex> {
    vec: ChunkedIndexVec<T, I>,
    map: HashMap<T, I>,
}

impl<T: Eq + Hash + Clone, I: TypedIndex> UniqueChunkedIndexVec<T, I> {
    pub fn new() -> Self {
        Self {
            vec: ChunkedIndexVec::new(),
            map: HashMap::new(),
        }
    }

    pub fn push(&mut self, value: T) -> I {
        if let Some(&idx) = self.map.get(&value) {
            return idx;
        }

        let idx: I = self.vec.len().into();
        self.vec.push(value.clone());
        self.map.insert(value, idx);
        idx
    }

    pub fn get(&self, index: I) -> &T {
        self.vec.get(index)
    }

    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        self.vec.iter()
    }
}
