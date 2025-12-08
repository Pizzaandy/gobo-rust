use std::fmt::Debug;
use std::hash::Hash;

pub trait TypedIndex: Copy + Eq + Debug + Hash + 'static + Into<usize> + From<usize> {}

#[macro_export]
macro_rules! typed_index {
    (
        $(#[$attrs:meta])*
        $vis:vis struct $type:ident($index_type:ty)
    ) => {
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $(#[$attrs])*
        $vis struct $type {
            raw: $index_type,
        }

        impl $type {
            pub const MAX: usize = <$index_type>::MAX as usize;

            pub fn value(&self) -> $index_type {
                self.raw
            }

            pub fn convert<T: $crate::typed_index::TypedIndex>(self) -> T {
                if size_of::<T>() <= size_of::<Self>() {
                    T::from(usize::from(self))
                } else {
                    panic!()
                }
            }
        }

        const _: () = assert!(
            size_of::<$index_type>() <= size_of::<usize>(),
            "index type must fit into usize"
        );

        impl $crate::typed_index::TypedIndex for $type {}

        impl From<$type> for usize {
            fn from(item: $type) -> usize {
                item.raw as usize
            }
        }

        impl From<usize> for $type {
            fn from(item: usize) -> $type {
                $type { raw: item as $index_type }
            }
        }

        impl std::ops::Add for $type {
            type Output = $type;
            fn add(self, rhs: Self) -> Self::Output {
                $type { raw: self.raw + rhs.raw }
            }
        }

        impl std::ops::Add<usize> for $type {
            type Output = $type;
            fn add(self, rhs: usize) -> Self::Output {
                $type { raw: self.raw + rhs as $index_type }
            }
        }

        impl std::ops::AddAssign<usize> for $type {
            fn add_assign(&mut self, rhs: usize) {
                self.raw += rhs as $index_type;
            }
        }

        impl std::ops::Sub for $type {
            type Output = $type;
            fn sub(self, rhs: Self) -> Self::Output {
                $type { raw: self.raw - rhs.raw }
            }
        }

        impl std::ops::Sub<usize> for $type {
            type Output = $type;
            fn sub(self, rhs: usize) -> Self::Output {
                $type { raw: self.raw - rhs as $index_type }
            }
        }

        impl PartialEq<$index_type> for $type {
            fn eq(&self, other: &$index_type) -> bool {
                self.raw == *other
            }
        }

        impl PartialOrd<$index_type> for $type {
            fn partial_cmp(&self, other: &$index_type) -> Option<std::cmp::Ordering> {
                Some(self.raw.cmp(other))
            }
        }

        impl std::fmt::Debug for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                 write!(f, "{}", self.raw)
            }
        }

        impl std::fmt::Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                 write!(f, "{}", self.raw)
            }
        }
    };
}
