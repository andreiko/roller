use core::ops::{Add, Sub, Div};
use num_traits::cast::AsPrimitive;

/// Implements simple aggregations over a ring buffer.
///
/// Notes:
/// The buffer size is fixed to avoid dynamic memory allocation.
pub struct Agg<T, const SIZE: usize> {
    data: [Option<T>; SIZE],
    next_put_at: usize,
}

impl<T: Copy + PartialOrd + Sub<Output=T>, const SIZE: usize> Agg<T, SIZE> {
    /// Returns a new uninitialized Agg instance.
    pub const fn new() -> Self {
        Self {
            data: [None; SIZE],
            next_put_at: 0,
        }
    }

    /// Adds a value to the ring buffer, possibly overwriting the oldest existing value.
    pub fn put(&mut self, n: T) {
        self.data[self.next_put_at] = Some(n);
        self.next_put_at = (self.next_put_at + 1) % self.data.len();
    }

    /// Returns the sum of the first n elements represented as type S.
    /// Returns None if there are fewer than n elements in the buffer.
    pub fn sum_of_first<S: 'static + Copy + Default + Add<Output=S>>(&self, n: usize) -> Option<S>
        where T: AsPrimitive<S>,
    {
        if n > SIZE {
            return None;
        }

        let mut sum = S::default();

        for el in &self.data[0..n] {
            if let Some(x) = el {
                let s: S = AsPrimitive::<S>::as_(*x);

                sum = sum + s;
            } else {
                return None;
            }
        }

        Some(sum)
    }

    /// Returns the average of all values stored in the buffer calculated through intermediate type S.
    /// Returns None if the buffer hasn't been fully initialized.
    pub fn avg_full<S>(&self) -> Option<T>
        where S: 'static + Copy + Default + Add<Output=S> + Div<Output=S>,
              usize: AsPrimitive<S>,
              T: AsPrimitive<S>,
              S: AsPrimitive<T>,
    {
        self.avg_of_first::<S>(SIZE)
    }

    /// Returns the average of the first n values stored in the buffer calculated through intermediate type S.
    /// Returns None if there are fewer than n elements in the buffer.
    pub fn avg_of_first<S>(&self, n: usize) -> Option<T>
        where S: 'static + Copy + Default + Add<Output=S> + Div<Output=S>,
              usize: AsPrimitive<S>,
              T: AsPrimitive<S>,
              S: AsPrimitive<T>,
    {
        let sum = if let Some(sum) = self.sum_of_first::<S>(n) {
            sum
        } else {
            return None;
        };

        let avg = sum / AsPrimitive::<S>::as_(n);

        Some(AsPrimitive::<T>::as_(avg))
    }

    /// Returns the smallest and the largest of the first n values in the buffer.
    /// Returns None if there are fewer than n elements in the buffer.
    pub fn range_of_first(&self, n: usize) -> Option<(T, T)> {
        let mut min: Option<T> = None;
        let mut max: Option<T> = None;

        for el in &self.data[0..n] {
            if let Some(x) = el {
                match min {
                    None => {
                        min = Some(*x)
                    }
                    Some(m) if *x < m => {
                        min = Some(*x)
                    }
                    _ => {}
                }

                match max {
                    None => {
                        max = Some(*x)
                    }
                    Some(m) if *x > m => {
                        max = Some(*x)
                    }
                    _ => {}
                }
            } else {
                return None;
            }
        }

        if let (Some(min), Some(max)) = (min, max) {
            Some((min, max))
        } else {
            None
        }
    }

    /// Returns the difference between the smallest and the largest values from the buffer.
    /// Returns None if the buffer hasn't been fully initialized.
    pub fn amplitude_full(&self) -> Option<T> {
        self.amplitude_of_first(SIZE)
    }

    /// Returns the difference between the smallest and the largest of the first n values from the buffer.
    /// Returns None if there are fewer than n elements in the buffer.
    pub fn amplitude_of_first(&self, n: usize) -> Option<T> {
        self.range_of_first(n).map(|(min, max)| max - min)
    }
}

#[cfg(feature = "debug_spi")]
const RING_SIZE: usize = 16;

#[cfg(feature = "debug_spi")]
pub struct Ring {
    buffer: [u8; RING_SIZE],
    next_write: usize,
    next_read: usize,
    full: bool,
}

#[cfg(feature = "debug_spi")]
impl Ring {
    pub const fn new() -> Self {
        Self {
            buffer: [0; RING_SIZE],
            next_write: 0,
            next_read: 0,
            full: false,
        }
    }

    pub fn read(&mut self) -> Option<u8> {
        if self.next_read == self.next_write {
            return None;
        }

        let result = self.buffer[self.next_read];
        self.next_read = (self.next_read + 1) % RING_SIZE;

        Some(result)
    }

    pub fn write(&mut self, data: u8) {
        if self.full {
            panic!();
        }

        self.buffer[self.next_write] = data;
        self.next_write = (self.next_write + 1) % RING_SIZE;
        self.full = self.next_write == self.next_read;
    }
}
