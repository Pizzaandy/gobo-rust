use crate::typed_index::TypedIndex;
use std::alloc::{self, Layout};
use std::marker::PhantomData;
use std::mem;
use std::mem::MaybeUninit;
use std::ptr::NonNull;

pub struct ChunkedIndexVec<T, I: TypedIndex> {
    len: usize,
    chunks: Vec<Chunk<T>>,
    _marker: PhantomData<fn(&I)>,
}

impl<T, I: TypedIndex> ChunkedIndexVec<T, I> {
    pub fn new() -> Self {
        Self {
            len: 0,
            chunks: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn push(&mut self, value: T) -> I {
        let id = self.len;
        let (chunk_index, _) = Self::get_chunk_and_index(self.len);

        if chunk_index == self.chunks.len() {
            self.chunks.push(Chunk::new());
        }

        self.chunks[chunk_index].push(value);
        self.len += 1;

        I::from(id)
    }

    pub fn get(&self, id: I) -> &T {
        debug_assert!(id.into() < self.len);
        let (chunk_index, pos) = Self::get_chunk_and_index(id.into());
        self.chunks[chunk_index].get(pos)
    }

    pub fn get_mut(&mut self, id: I) -> &mut T {
        debug_assert!(id.into() < self.len);
        let (chunk_index, pos) = Self::get_chunk_and_index(id.into());
        self.chunks[chunk_index].get_mut(pos)
    }

    pub fn reserve(&mut self, len: usize) {
        if len <= self.len {
            return;
        }
        let (final_chunk_index, _) = Self::get_chunk_and_index(len - 1);
        self.chunks.resize_with(final_chunk_index + 1, Chunk::new);
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        Iter {
            inner: self,
            index: 0,
        }
    }

    fn get_chunk_and_index(index: usize) -> (usize, usize) {
        let low_bits = Chunk::<T>::index_bits();
        let chunk_index = index >> low_bits;
        let pos = index & ((1 << low_bits) - 1);
        (chunk_index, pos)
    }
}

/// An allocated chunk of `T` with fixed capacity.
struct Chunk<T> {
    ptr: NonNull<MaybeUninit<T>>,
    len: usize,
}

impl<T> Chunk<T> {
    const MAX_SIZE_BYTES: usize = 4 * 1024;

    // Number of elements stored in each chunk.
    // The number must be a power of two so that that there are no unused values
    // in bits indexing into the allocation.
    pub const fn capacity() -> usize {
        let max_elements = Self::MAX_SIZE_BYTES / size_of::<T>();
        // Previous power of two
        if max_elements == 0 {
            0
        } else {
            1 << max_elements.ilog2()
        }
    }

    // Number of bits needed to index each element in a chunk allocation.
    pub const fn index_bits() -> usize {
        const {
            assert!(Self::capacity() > 0);
        };
        Self::capacity().next_power_of_two().trailing_zeros() as usize
    }

    pub fn new() -> Self {
        const {
            assert!(1 << Self::index_bits() == Self::capacity());
        };

        let capacity = Self::capacity();
        let layout = Layout::array::<MaybeUninit<T>>(capacity).unwrap();
        let raw_ptr = unsafe { alloc::alloc(layout) } as *mut MaybeUninit<T>;
        if raw_ptr.is_null() {
            alloc::handle_alloc_error(layout);
        }
        Self {
            ptr: unsafe { NonNull::new_unchecked(raw_ptr) },
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        assert!(self.len < Self::capacity());
        unsafe {
            self.ptr
                .as_ptr()
                .add(self.len)
                .write(MaybeUninit::new(value))
        };
        self.len += 1;
    }

    pub fn get(&self, index: usize) -> &T {
        debug_assert!(index < self.len);
        unsafe { &*self.ptr.as_ptr().add(index).cast::<T>() }
    }

    pub fn get_mut(&mut self, index: usize) -> &mut T {
        debug_assert!(index < self.len);
        unsafe { &mut *self.ptr.as_ptr().add(index).cast::<T>() }
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl<T> Drop for Chunk<T> {
    fn drop(&mut self) {
        if mem::needs_drop::<T>() {
            for i in 0..self.len {
                unsafe { self.ptr.as_ptr().add(i).cast::<T>().drop_in_place() };
            }
        }
        let layout = Layout::array::<MaybeUninit<T>>(Self::capacity()).unwrap();
        unsafe { alloc::dealloc(self.ptr.as_ptr() as *mut u8, layout) };
    }
}

struct Iter<'a, T, I: TypedIndex> {
    inner: &'a ChunkedIndexVec<T, I>,
    index: usize,
}

impl<'a, T, I: TypedIndex> Iterator for Iter<'a, T, I> {
    type Item = (I, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.inner.len() {
            let i = I::from(self.index);
            self.index += 1;
            Some((i, self.inner.get(i)))
        } else {
            None
        }
    }
}
