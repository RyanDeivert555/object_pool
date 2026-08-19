use std::cell::{Cell, UnsafeCell};

pub struct ObjectPool<T, const N: usize>
where
    T: Default,
{
    pool: [UnsafeCell<T>; N],
    free_list: [Cell<usize>; N],
    free_count: Cell<usize>,
    reset: fn(&mut T),
}

#[derive(Debug, Clone, Copy)]
pub struct NoFreeSlotsError;

impl std::fmt::Display for NoFreeSlotsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "No free slots left")
    }
}

impl std::error::Error for NoFreeSlotsError {}

#[must_use]
pub struct Handle<'pool, T, const N: usize>
where
    T: Default,
{
    index: usize,
    pool_ref: &'pool ObjectPool<T, N>,
}

impl<'pool, T, const N: usize> Handle<'pool, T, N>
where
    T: Default,
{
    fn new(index: usize, pool_ref: &'pool ObjectPool<T, N>) -> Self {
        Self { index, pool_ref }
    }
}

impl<'pool, T, const N: usize> std::ops::Deref for Handle<'pool, T, N>
where
    T: Default,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let cell_ptr = &self.pool_ref.pool[self.index].get();

        // SAFETY: Cells are always unique in the pool and never overlap
        unsafe { cell_ptr.as_ref_unchecked() }
    }
}

impl<'pool, T, const N: usize> std::ops::DerefMut for Handle<'pool, T, N>
where
    T: Default,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        let cell_ptr = &self.pool_ref.pool[self.index].get();

        // SAFETY: Cells are always unique in the pool and never overlap
        unsafe { cell_ptr.as_mut_unchecked() }
    }
}

impl<'pool, T, const N: usize> Drop for Handle<'pool, T, N>
where
    T: Default,
{
    fn drop(&mut self) {
        let cell_ptr = &self.pool_ref.pool[self.index].get();
        // SAFETY: Cells are always unique in the pool and never overlap
        unsafe { (self.pool_ref.reset)(cell_ptr.as_mut_unchecked()) };
        self.pool_ref.free_list[self.pool_ref.free_count.get()].set(self.index);
        self.pool_ref.free_count.update(|n| n + 1);
    }
}

impl<T, const N: usize> Default for ObjectPool<T, N>
where
    T: Default,
{
    fn default() -> Self {
        ObjectPool::new(|x| *x = T::default())
    }
}

impl<T, const N: usize> ObjectPool<T, N>
where
    T: Default,
{
    /// Creates a object pool. The items within the pool are default-constructed.
    /// The pool uses interior mutability, so the pool itself does not need to be mutable.
    ///
    /// # Examples
    ///
    /// ```
    /// use object_pool::*;
    ///
    /// let pool = ObjectPool::<Vec<i32>, 10>::new(Vec::clear);
    /// ```
    pub fn new(reset: fn(&mut T)) -> Self {
        Self {
            pool: std::array::from_fn(|_| UnsafeCell::new(T::default())),
            free_list: std::array::from_fn(Cell::new),
            free_count: Cell::new(N),
            reset,
        }
    }

    fn reserve(&self) -> Handle<'_, T, N> {
        let index = self.free_list[self.free_count.get() - 1].get();
        self.free_count.update(|n| n - 1);

        Handle::new(index, self)
    }

    /// Takes an element of the pool through a handle.
    /// The element returned will be default constructed.
    ///
    /// # Examples
    ///
    /// ```
    /// use object_pool::*;
    ///
    /// let pool: ObjectPool<Vec<i32>, 10> = ObjectPool::new(Vec::clear);
    /// let handle = pool.take().unwrap();
    /// assert_eq!(*handle, Vec::default());
    /// ```
    pub fn take(&self) -> Result<Handle<'_, T, N>, NoFreeSlotsError> {
        if self.free_count.get() > 0 {
            Ok(self.reserve())
        } else {
            Err(NoFreeSlotsError)
        }
    }

    /// Takes an element of the pool through a handle with the assigned value.
    /// This is equivalent to
    /// ```ignore
    /// let value = ...;
    /// let mut handle = pool.take().unwrap();
    /// *handle = value;
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use object_pool::*;
    ///
    /// let pool: ObjectPool<Vec<i32>, 10> = ObjectPool::new(Vec::clear);
    /// let handle = pool.take_with_value(Vec::with_capacity(10)).unwrap();
    /// assert_eq!(handle.capacity(), 10);
    /// ```
    pub fn take_with_value(&self, value: T) -> Result<Handle<'_, T, N>, NoFreeSlotsError> {
        if self.free_count.get() > 0 {
            let mut handle = self.reserve();
            *handle = value;

            Ok(handle)
        } else {
            Err(NoFreeSlotsError)
        }
    }

    /// Similar to ObjectPool::take_with_value, but the value is only assigned if another slot is
    /// available in the pool.
    ///
    /// # Examples
    ///
    /// ```
    /// use object_pool::*;
    ///
    /// let pool: ObjectPool<Vec<i32>, 10> = ObjectPool::new(Vec::clear);
    /// let handle = pool.lazy_take(|| Vec::with_capacity(10)).unwrap();
    /// assert_eq!(handle.capacity(), 10);
    /// ```
    pub fn lazy_take(&self, f: impl FnOnce() -> T) -> Result<Handle<'_, T, N>, NoFreeSlotsError> {
        if self.free_count.get() > 0 {
            let value = f();
            let mut handle = self.reserve();
            *handle = value;

            Ok(handle)
        } else {
            Err(NoFreeSlotsError)
        }
    }

    /// Manually release a handle back into the pool. This is functionaly equivalent to dropping the
    /// handle manually, but with a check asserting the handle belongs to the pool.
    ///
    /// # Examples
    ///
    /// ```
    /// use object_pool::*;
    ///
    /// let pool: ObjectPool<Vec<i32>, 1> = ObjectPool::new(Vec::clear);
    /// let handle = pool.take().unwrap();
    /// pool.release(handle);
    /// ```
    pub fn release(&self, handle: Handle<'_, T, N>) {
        assert!(std::ptr::eq(self, handle.pool_ref));
        drop(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_creation() {
        let pool = ObjectPool::<i32, 3>::new(|x| *x = 0);
        let a = pool.take_with_value(1).unwrap();
        let b = pool.take_with_value(2).unwrap();
        let c = pool.take_with_value(3).unwrap();
        assert_eq!(*a, 1);
        assert_eq!(*b, 2);
        assert_eq!(*c, 3);
    }

    #[test]
    fn writes_to_slot() {
        let pool = ObjectPool::<i32, 1>::new(|x| *x = 0);
        let mut a = pool.take_with_value(10).unwrap();
        *a = 20;
        assert_eq!(*a, 20);
    }

    #[test]
    fn fail_on_full_capacity() {
        let pool = ObjectPool::<i32, 2>::new(|x| *x = 0);
        let _a = pool.take().unwrap();
        let _b = pool.take().unwrap();
        assert!(pool.take().is_err());
        assert!(pool.take().is_err());
    }

    #[test]
    fn slot_reuse() {
        let pool = ObjectPool::<i32, 1>::new(|x| *x = 0);
        {
            let _a = pool.take_with_value(1).unwrap();
            assert!(pool.take().is_err());
        }

        let b = pool.take_with_value(2).unwrap();
        assert_eq!(*b, 2);
    }

    #[test]
    fn drained_and_refill() {
        let pool = ObjectPool::<i32, 3>::new(|x| *x = 0);
        for round in 0..5 {
            let a = pool.take_with_value(round).unwrap();
            let b = pool.take_with_value(round).unwrap();
            let c = pool.take_with_value(round).unwrap();
            assert!(pool.take_with_value(round).is_err());
            drop(a);
            drop(b);
            drop(c);
        }
    }

    #[test]
    fn reset() {
        let pool = ObjectPool::<Vec<i32>, 1>::new(Vec::clear);

        {
            let mut mem = pool.take().unwrap();
            mem.push(1);
            assert!(mem.len() == 1);
            assert!(mem.capacity() >= 1);
        }
        {
            let mem = pool.take().unwrap();
            assert!(mem.len() == 0);
            assert!(mem.capacity() >= 1);
        }
    }

    #[test]
    fn manual_release() {
        let pool = ObjectPool::<Vec<i32>, 1>::new(Vec::clear);

        let mem = pool.take().unwrap();
        assert!(pool.take().is_err());

        pool.release(mem);

        let _mem = pool.take().unwrap();
        assert!(pool.take().is_err());
    }

    #[test]
    fn lazy_take() {
        let pool = ObjectPool::<i32, 1>::new(|x| *x = 0);

        let mem = pool.lazy_take(|| 999).unwrap();
        assert_eq!(*mem, 999);

        assert!(pool.lazy_take(|| unreachable!()).is_err());
    }
}
