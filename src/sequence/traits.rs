
use std::pin::Pin;
use std::future::Future;
use std::sync::Arc;

use crate::{Candidate, BincodeConfiguration};

/// ### -> `Allocation<T> Trait`.
/// 
/// Provides allocation capabilities for sequences and is the foundational
/// trait for all sequence types.
/// 
/// Type Parameters:
/// - `T`: The type of elements stored in the sequence. Must implement `Send`, `Sync`, and have a static lifetime.
///
/// This trait typically works with both Sequence<T> and Arc<Sequence<T>> types,
/// allowing for flexible allocation strategies in concurrent environments. Though,
/// the implementation for Arc<Sequence<T>> is useless and is only provided to satisfy
/// further needs of other traits which require `Allocation<T>`.
/// 
/// All methods in this trait are asynchronous and return pinned boxed futures (as async
/// functions cannot be directly part of traits in Rust).
/// 
/// ### -> `Methods`
/// - `allocate(capacity: usize) -> Arc<Self::SelfType>`:
/// Asynchronously allocates a new sequence with the specified capacity and returns it wrapped in an Arc. `(Recommended)`
/// - `allocate_raw(capacity: usize) -> Self::SelfType`:
/// Asynchronously allocates a new sequence with the specified capacity and returns it directly.
/// 
/// ### -> `Usage`
/// 
/// ```
/// use supersonic::sequence::prelude::*;
/// use anyhow::Result;
/// 
/// async fn example() -> Result<()> {
///     let sequence = Sequence::<i32>::allocate(5).await;
///     assert_eq!(sequence.length(), 0);
///     assert!(sequence.capacity() == 5);
/// 
///     // Append some values
///     for i in 0..5 {
///        sequence.append(Candidate::Value(i as i32)).await?;
///     }
/// 
///     assert_eq!(sequence.length(), 5);
/// 
///     Ok(())
/// }
/// 
/// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
/// supersonic::future!(example());
/// ```
pub trait Allocation<T>
where
    T: Send + Sync + 'static,
    Self: Sized
{
    /// The self-type defined for convenience as each trait requires `Allocation<T>` and every
    /// trait has to be implemented for `Self` and `Arc<Self>`.
    type SelfType;
    
    /// ### -> `allocate`
    /// 
    /// Asynchronously allocates a new sequence with the specified capacity
    /// and returns it wrapped in an Arc.
    /// 
    /// This method is recommended for most use cases as it provides a convenient
    /// way to allocate sequences wrapped in an Arc, facilitating shared ownership
    /// and thread-safe access.
    /// 
    /// ### -> `Parameters`
    /// - `capacity: usize`: The initial capacity of the sequence to be allocated.
    /// 
    /// ### -> `Returns`
    /// - `Arc<Sequence<T>>`: An asynchronously allocated sequence wrapped in an Arc.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(5).await;
    ///     assert_eq!(sequence.length(), 0);
    ///     assert!(sequence.capacity() == 5);
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    #[must_use = "Allocated sequences must have a purpose!"]
    fn allocate(capacity: usize) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + 'static>> {
        Box::pin(async move {
            Arc::new(Self::allocate_raw(capacity).await)
        })
    }

    /// ### -> `allocate_raw`
    /// 
    /// Asynchronously allocates a new sequence with the specified capacity
    /// and returns it directly.
    /// 
    /// This method is useful when you need direct ownership of the sequence
    /// without the overhead of Arc wrapping.
    /// 
    /// ### -> `Parameters`
    /// - `capacity: usize`: The initial capacity of the sequence to be allocated.
    /// 
    /// ### -> `Returns`
    /// - `Sequence<T>`: An asynchronously allocated sequence.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate_raw(5).await;
    ///     assert_eq!(sequence.length(), 0);
    ///     assert!(sequence.capacity() == 5);
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    #[must_use = "Allocated sequences must have a purpose!"]
    fn allocate_raw(capacity: usize) -> Pin<Box<dyn Future<Output = Self::SelfType> + Send + 'static>>;
}


/// ### -> `Operation<T> Trait`.
/// 
/// Provides fundamental operations for sequences, including element access,
/// modification, insertion, and removal. This trait serves as a base for more
/// specialized sequence traits such as `Reactive<T>` and `NonReactive<T>`.
/// 
/// Type Parameters:
/// - `T`: The type of elements stored in the sequence. Must implement `Send`, `Sync`, and have a static lifetime.
/// 
/// This trait typically works with both Sequence<T> and Arc<Sequence<T>> types,
/// allowing for flexible operation strategies in concurrent environments.
/// 
/// All methods in this trait are asynchronous and return pinned boxed futures (as async
/// functions cannot be directly part of traits in Rust).
///
/// ### -> `Methods`
/// - `get(index: usize, synchronize: bool) -> Compat<T>`:
///     - Asynchronously retrieves the element at the specified index.
///     - `synchronize`: If true, ensures the returned element is synchronized with the latest state (expensive) or a cheap read (if false).
///     - Returns a `Compat<T>` which can represent either a value or an empty state.
///     - The index must be within bounds; otherwise, it returns an empty `Compat<T>`.
/// 
/// - `set(index: usize, value: Candidate<T>) -> Result<Compat<T>>`:
///     - Asynchronously sets the element at the specified index to the provided value.
///     - Returns the previous element wrapped in a `Compat<T>`.
///     - The index must be within bounds (index < length); otherwise, it returns an error.
///     - No resizing occurs; the sequence size remains unchanged.
///     - `Candidate<T>` can be either a direct value or a an pre-built Arc.
///     - Replaces the entire Arc at the specified index with a new Arc containing the provided value. (Non-Reactive)
/// 
/// - `insert(index: usize, value: Candidate<T>)`:
///     - Asynchronously inserts the provided value at the specified index.
///     - The index can be equal to the length of the sequence (appending).
///     - The sequence may resize to accommodate the new element.
///     - Resizing strategy has been designed to avoid repeated resizes on each insert.
///     - `Candidate<T>` can be either a direct value or a an pre-built Arc.
///     - Returns nothing because the operation is responsible for making space (shifting other elements) and then inserting the new element. The Old pointer is always null in this case.
/// 
/// - `remove(index: usize) -> Result<Compat<T>>`:
///     - Asynchronously removes the element at the specified index.
///     - Returns the removed element wrapped in a `Compat<T>`.
///     - Removal of an element may lead to shifting of subsequent elements to fill the gap.
///     - The index must be within bounds (index < length); otherwise, it returns an error.
///     - The sequence size decreases by one upon successful removal.
///     - The sequence capacity remains unchanged after removal.
/// 
/// - `append(value: Candidate<T>) -> Result<()>`:
///     - Asynchronously appends the provided value to the end of the sequence.
///     - `Candidate<T>` can be either a direct value or a an pre-built Arc.
///     - May result in an increase in sequence capacity.
///     - Resizing strategy has been designed to avoid repeated resizes on each insert.
/// 
/// - `extend(iter: impl IntoIterator<Item = T>) `:
///     - Asynchronously extends the sequence by appending elements from the provided iterator.
///     - The sequence may resize to accommodate the new elements.
///     - Resizing strategy has been designed to avoid repeated resizes on each insert.
/// 
/// - `drain(range: Option<std::ops::Range<usize>>) -> Arc<Sequence<T>>`:
///     - Asynchronously removes and returns a sequence containing elements within the specified range.
///     - If `range` is `None`, all elements are drained.
///     - The elements are permanently removed from the original sequence.
///     - The returned sequence contains the drained elements in their original order.
/// 
/// ### -> `Usage`
/// 
/// ```
/// use supersonic::sequence::prelude::*;
/// use anyhow::Result;
/// 
/// async fn example() -> Result<()> {
///     let sequence = Sequence::<i32>::allocate(5).await;
///     assert_eq!(sequence.length(), 0);
///     assert!(sequence.capacity() == 5);
/// 
///     // Append some values
///     for i in 0..5 {
///        sequence.append(Candidate::Value(i as i32)).await?;
///     }
/// 
///     assert_eq!(sequence.length(), 5);
/// 
///     // Get value at index 2
///     let value = sequence.get(2, true).await;
///     assert!(!value.empty());
///     assert_eq!(*value.as_arc().await.read().await, 2);
/// 
///     // Set value at index 2
///     let old_value = sequence.set(2, Candidate::Value(20)).await?;
///     assert!(!old_value.empty());
/// 
///     let new_value = sequence.get(2, true).await;
///     assert!(!new_value.empty());
/// 
///     assert_eq!(*new_value.as_arc().await.read().await, 20);
/// 
///     Ok(())
/// }
/// 
/// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
/// supersonic::future!(example());
/// ```
///
pub trait Operation<T>: Allocation<T> + Length
where
    T: Send + Sync + 'static,
    Self: Sync
{
    /// The type defined for consistency across traits.
    /// Represents `Compat<T>`.
    type ArcSwapRef;

    /// Asynchronously retrieves the element at the specified index.
    /// - `synchronize`: If true, ensures the returned element is synchronized with the latest state (expensive) or a cheap read (if false).
    /// - Returns a `Compat<T>` which can represent either a value or an empty state.
    /// - The index must be within bounds; otherwise, it returns an empty `Compat<T>`.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(5).await;
    ///     for i in 0..5 {
    ///        sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    ///     let value = sequence.get(2, true).await;
    ///     assert!(!value.empty());
    ///     assert_eq!(*value.as_arc().await.read().await, 2);
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    #[must_use = "Fetched elements must have a purpose!"]
    fn get(&self, index: usize, synchronize: bool) -> Pin<Box<dyn Future<Output = Self::ArcSwapRef> + Send + '_>>;

    /// Asynchronously sets the element at the specified index to the provided value.
    /// - Returns the previous element wrapped in a `Compat<T>`.
    /// - The index must be within bounds (index < length); otherwise, it returns an error.
    /// - No resizing occurs; the sequence size remains unchanged.
    /// - `Candidate<T>` can be either a direct value or a an pre-built Arc.
    /// - Replaces the entire Arc at the specified index with a new Arc containing the provided value. (Non-Reactive)
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(5).await;
    ///     for i in 0..5 {
    ///         sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    ///     let old_value = sequence.set(2, Candidate::Value(20)).await?;
    ///     assert!(!old_value.empty());
    /// 
    ///     let new_value = sequence.get(2, true).await;
    ///     assert!(!new_value.empty());
    /// 
    ///     assert_eq!(*new_value.as_arc().await.read().await, 20);
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn set(&self, index: usize, value: crate::Candidate<T>) -> Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send + '_>>;

    /// Asynchronously inserts the provided value at the specified index.
    /// - The index can be equal to the length of the sequence (appending).
    /// - The sequence may resize to accommodate the new element.
    /// - Resizing strategy has been designed to avoid repeated resizes on each insert.
    /// - `Candidate<T>` can be either a direct value or a an pre-built Arc.
    /// - Returns nothing because the operation is responsible for making space (shifting other elements) and
    /// then inserting the new element. The Old pointer is always null in this case.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(5).await;
    ///     for i in 0..5 {
    ///         sequence.insert(i, Candidate::Value(i as i32)).await;
    ///     }
    ///     
    ///     assert_eq!(sequence.length(), 5);
    ///     assert_eq!(sequence.capacity(), 5);
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn insert(&self, index: usize, value: crate::Candidate<T>) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    /// Asynchronously removes the element at the specified index.
    /// - Returns the removed element wrapped in a `Compat<T>`.
    /// - Removal of an element may lead to shifting of subsequent elements to fill the gap.
    /// - The index must be within bounds (index < length); otherwise, it returns an error.
    /// - The sequence size decreases by one upon successful removal.
    /// - The sequence capacity remains unchanged after removal.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(5).await;
    ///     for i in 0..5 {
    ///         sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    ///     let removed_value = sequence.remove(2).await?;
    ///     assert!(!removed_value.empty());
    ///     assert_eq!(*removed_value.as_arc().await.read().await, 2);
    /// 
    ///     assert_eq!(sequence.length(), 4);
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn remove(&self, index: usize) -> Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send +'_>>;

    /// Asynchronously appends the provided value to the end of the sequence.
    /// - `Candidate<T>` can be either a direct value or a an pre-built Arc.
    /// - May result in an increase in sequence capacity.
    /// - Resizing strategy has been designed to avoid repeated resizes on each insert.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(5).await;
    ///     for i in 0..5 {
    ///         sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    /// 
    ///     assert_eq!(sequence.length(), 5);
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn append(&self, value: crate::Candidate<T>) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>;

    /// Asynchronously extends the sequence by appending elements from the provided iterator.
    /// - The sequence may resize to accommodate the new elements.
    /// - Resizing strategy has been designed to avoid repeated resizes on each insert.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(5).await;
    ///     sequence.extend(vec![0, 1, 2, 3, 4]).await;
    ///     sequence.extend(vec![5, 6, 7, 8, 9]).await;
    /// 
    ///     assert_eq!(sequence.length(), 10);
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn extend(&self, iter: impl IntoIterator<Item = T>) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    /// Asynchronously removes and returns a sequence containing elements within the specified range.
    /// - If `range` is `None`, all elements are drained.
    /// - The elements are permanently removed from the original sequence.
    /// - The returned sequence contains the drained elements in their original order.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(10).await;
    ///     for i in 0..10 {
    ///         sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    /// 
    ///     let drained = sequence.drain(Some(0..5)).await;
    ///     assert_eq!(drained.length(), 5);
    ///     for i in 0..5 {
    ///         let value = drained.get(i as usize, true).await;
    ///         assert!(!value.empty());
    ///         assert_eq!(*value.as_arc().await.read().await, i as i32);
    ///     }
    /// 
    ///    assert_eq!(sequence.length(), 5);
    ///    for i in 0..5 {
    ///        let value = sequence.get(i as usize, true).await;
    ///        assert!(!value.empty());
    ///        assert_eq!(*value.as_arc().await.read().await, (i + 5) as i32);
    ///    }
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn drain(&self, range: Option<std::ops::Range<usize>>) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>
    where
        T: Clone;

}


/// ### -> `Reactive<T> Trait`.
/// 
/// Provides reactive operations for sequences, allowing for dynamic updates
/// and modifications that reflect changes across related sequences.
/// 
/// ### -> `Reactivity`
/// 
/// Reactive sequences ensure that any modifications made to the original
/// sequence are automatically reflected in derived sequences (like slices,
/// reverses, etc.) and vice versa. This is particularly useful in scenarios
/// where data consistency and synchronization are critical.
///
/// ### -> `Example Scenarios`
/// 
/// - Slicing a sequence with a negative step to create a reversed view:
///     - Modifying an element in the original sequence should update the corresponding
///       element in the reversed slice and vice versa.
/// 
/// - Reversing a sequence to create a new view:
///     - Changes in the original sequence should be reflected in the reversed sequence,
///       and modifications in the reversed sequence should update the original.
///
/// Type Parameters:
/// - `T`: The type of elements stored in the sequence. Must implement `Send`, `Sync`, and have a static lifetime.
///
/// This trait typically works with both Sequence<T> and Arc<Sequence<T>> types,
/// allowing for flexible reactive strategies in concurrent environments.
///
/// All methods in this trait are asynchronous and return pinned boxed futures (as async
/// functions cannot be directly part of traits in Rust).
/// 
/// ### -> `Methods`
/// 
/// - `extract(range: Option<std::ops::Range<usize>>) -> Arc<Sequence<T>>`:
///     - Asynchronously extracts a subsequence defined by the specified range.
///     - The extracted subsequence maintains a reactive link to the original sequence,
///       ensuring that changes in either sequence are reflected in the other.
///     - This operation is similar to draining, but the original sequence retains its elements.
/// 
/// - `slice(start: Option<isize>, stop: Option<isize>, step: Option<isize>) -> Arc<Sequence<T>>`:
///     - Asynchronously creates a sliced view of the sequence based on the provided parameters.
///     - This is similar to Python-style slicing, supporting negative indices and steps.
///     - The sliced sequence is reactive, meaning modifications in either the original
///       sequence or the slice are reflected in both.
/// 
/// - `split(index: usize) -> (Arc<Sequence<T>>, Arc<Sequence<T>>)`:
///     - Asynchronously splits the sequence into two at the specified index.
///     - Both resulting sequences maintain a reactive relationship with the original sequence,
///       ensuring that changes in one are reflected in the others.
/// 
/// - `reverse() -> Arc<Sequence<T>>`:
///     - Asynchronously creates a reversed view of the sequence.
///     - The reversed sequence is reactive, so modifications in either the original
///       sequence or the reversed sequence are reflected in both.
///
/// - `modify(index: usize, value: T) -> Result<Compat<T>>`:
///     - Asynchronously modifies the value at the specified index without replacing the entire Arc.
///     - This operation ensures that changes are propagated reactively to any derived sequences.
///     - Returns the previous value wrapped in a `Compat<T>`.
///     - The index must be within bounds (index < length); otherwise, it returns an error.
///
/// ### -> `Usage`
/// 
/// ```
/// use supersonic::sequence::prelude::{Sequence, Allocation, Length, Operation, Reactive, Candidate};
/// use anyhow::Result;
/// 
/// async fn example() -> Result<()> {
///     let sequence = Sequence::<i32>::allocate(10).await;
///     for i in 0..10 {
///         sequence.append(Candidate::Value(i as i32)).await?;
///     }
/// 
///     let reversed = Reactive::reverse(&sequence).await;
///     assert_eq!(reversed.length(), 10);
///     for i in 0..10 {
///         let value = reversed.get(i as usize, true).await;
///         assert!(!value.empty());
///         assert_eq!(*value.as_arc().await.read().await, (9 - i) as i32);
///     }
/// 
///    // now check modification in original sequence changes reversed sequence
///    // and vice versa
///    // must use -> modify fn, not set fn
///
///    // Modify original sequence at index 3 (value should be 3)
///    Reactive::modify(&sequence, 3, 100).await?;
/// 
///    // Check that reversed sequence at index 6 (which points to original index 3) reflects the change
///    let value_in_reversed = reversed.get(6, true).await;
///    assert!(!value_in_reversed.empty());
///    assert_eq!(*value_in_reversed.as_arc().await.read().await, 100, "Modification in original should reflect in reversed");
///
///    // Modify reversed sequence at index 2 (which points to original index 7)
///    Reactive::modify(&reversed, 2, 200).await?;
///
///    // Check that original sequence at index 7 reflects the change
///    let value_in_original = sequence.get(7, true).await;
///    assert!(!value_in_original.empty());
///    assert_eq!(*value_in_original.as_arc().await.read().await, 200, "Modification in reversed should reflect in original");
/// 
///    Ok(())
/// }
/// 
/// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
/// supersonic::future!(example());
/// ```
/// 
/// ### -> `Note`
/// 
/// The methods provided in this traits also contain non-reactive counterparts under `NonReactive<T>` trait.
/// Therefore, ensure to choose the appropriate trait based on whether you require reactive behavior or not.
/// `modify` method is exclusive to `Reactive<T>` trait as it deals with in-place modifications that need to be reflected reactively.
pub trait Reactive<T>: Operation<T>
where
    T: Send + Sync + 'static,
    Self: Sync
{

    /// Asynchronously extracts a subsequence defined by the specified range.
    /// - The extracted subsequence maintains a reactive link to the original sequence,
    ///   ensuring that changes in either sequence are reflected in the other.
    /// - This operation is similar to draining, but the original sequence retains its elements.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::{Sequence, Allocation, Length, Operation, Reactive, Candidate};
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(10).await;
    ///     for i in 0..10 {
    ///         sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    /// 
    ///     let extracted = Reactive::extract(&sequence, Some(2..7)).await;
    ///     assert_eq!(extracted.length(), 5);
    ///     for i in 0..5 {
    ///         let value = extracted.get(i as usize, true).await;
    ///         assert!(!value.empty());
    ///         assert_eq!(*value.as_arc().await.read().await, (i + 2) as i32);
    ///     }
    /// 
    ///     // Check that original sequence remains unchanged
    ///     assert_eq!(sequence.length(), 10);
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    /// 
    /// ### -> `Note`
    /// 
    /// The extracted sequence is reactive, meaning modifications in either the original
    /// sequence or the extracted sequence are reflected in both.
    /// 
    /// This method also has a non-reactive counterpart under `NonReactive<T>` trait.
    #[must_use = "Extraction of elements increments reference counts of pre-existing elements without removing them! Must serve a purpose!"]
    fn extract(&self, range: Option<std::ops::Range<usize>>) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>;
    
    /// Asynchronously creates a sliced view of the sequence based on the provided parameters.
    /// - This is similar to Python-style slicing, supporting negative indices and steps.
    /// - The sliced sequence is reactive, meaning modifications in either the original
    ///   sequence or the slice are reflected in both.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::{Sequence, Allocation, Length, Operation, Reactive, Candidate};
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(10).await;
    ///     for i in 0..10 {
    ///         sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    /// 
    ///     let sliced = Reactive::slice(&sequence, Some(2), Some(8), Some(2)).await;
    ///     assert_eq!(sliced.length(), 3);
    ///     for i in 0..3 {
    ///         let value = sliced.get(i as usize, true).await;
    ///         assert!(!value.empty());
    ///         assert_eq!(*value.as_arc().await.read().await, (2 + i * 2) as i32);
    ///     }
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    /// 
    /// ### -> `Note`
    /// 
    /// The sliced sequence is reactive, meaning modifications in either the original
    /// sequence or the slice are reflected in both.
    /// 
    /// This method also has a non-reactive counterpart under `NonReactive<T>` trait.
    #[must_use = "Slicing is not 0 cost and must serve a purpose!"]
    fn slice(&self, start: Option<isize>, stop: Option<isize>, step: Option<isize>) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>;
    
    /// Asynchronously splits the sequence into two at the specified index.
    /// - Both resulting sequences maintain a reactive relationship with the original sequence,
    ///   ensuring that changes in one are reflected in the others.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::{Sequence, Allocation, Length, Operation, Reactive, Candidate};
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///    let sequence = Sequence::<i32>::allocate(10).await;
    ///    for i in 0..10 {
    ///        sequence.append(Candidate::Value(i as i32)).await?;
    ///    }
    ///
    ///    let (first_half, second_half) = Reactive::split(&sequence, 5).await;
    ///    assert_eq!(first_half.length(), 5);
    ///    assert_eq!(second_half.length(), 5);
    ///    for i in 0..5 {
    ///        let value_first = first_half.get(i as usize, true).await;
    ///        assert!(!value_first.empty());
    ///        assert_eq!(*value_first.as_arc().await.read().await, i as i32);
    ///
    ///        let value_second = second_half.get(i as usize, true).await;
    ///        assert!(!value_second.empty());
    ///        assert_eq!(*value_second.as_arc().await.read().await, (i + 5) as i32);
    ///    } 
    /// 
    ///    Ok(())
    /// }
    ///
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    /// 
    /// ### -> `Note`
    /// 
    /// Both resulting sequences are reactive, ensuring that modifications in one
    /// are reflected in the others.
    /// 
    /// This method also has a non-reactive counterpart under `NonReactive<T>` trait.
    #[must_use = "Splitting is not 0 cost and must serve a purpose!"]
    fn split(&self, index: usize) -> Pin<Box<dyn Future<Output = (Arc<Self::SelfType>, Arc<Self::SelfType>)> + Send + '_>>;

    /// Asynchronously creates a reversed view of the sequence.
    /// - The reversed sequence is reactive, so modifications in either the original
    ///   sequence or the reversed sequence are reflected in both.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::{Sequence, Allocation, Length, Operation, Reactive, Candidate};
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(10).await;
    ///     for i in 0..10 {
    ///         sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    /// 
    ///     let reversed = Reactive::reverse(&sequence).await;
    ///     assert_eq!(reversed.length(), 10);
    ///     for i in 0..10 {
    ///         let value = reversed.get(i as usize, true).await;
    ///         assert!(!value.empty());
    ///         assert_eq!(*value.as_arc().await.read().await, (9 - i) as i32);
    ///     }
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    /// 
    /// ### -> `Note`
    /// 
    /// The reversed sequence is reactive, meaning modifications in either the original
    /// sequence or the reversed sequence are reflected in both.
    /// 
    /// This method also has a non-reactive counterpart under `NonReactive<T>` trait.
    #[must_use = "Reversing is not 0 cost and must serve a purpose!"]
    fn reverse(&self) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>;

    /// Asynchronously modifies the value at the specified index without replacing the entire Arc.
    /// - This operation ensures that changes are propagated reactively to any derived sequences.
    /// - Returns the previous value wrapped in a `Compat<T>`.
    /// - The index must be within bounds (index < length); otherwise, it returns an error.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::{Sequence, Allocation, Length, Operation, Reactive, Candidate};
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(5).await;
    ///     for i in 0..5 {
    ///         sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    /// 
    ///     let old_value = Reactive::modify(&sequence, 2, 20).await?;
    ///     assert!(!old_value.empty());
    /// 
    ///     let new_value = sequence.get(2, true).await;
    ///     assert!(!new_value.empty());
    /// 
    ///     assert_eq!(*new_value.as_arc().await.read().await, 20);
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    /// 
    /// ### -> `Note`
    /// 
    /// This method is exclusive to `Reactive<T>` trait as it deals with in-place modifications
    /// that need to be reflected reactively.
    /// 
    /// This method does not have a non-reactive counterpart as modifying in place
    fn modify(&self, index: usize, value: T) -> Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send + '_>>;

}

/// ### -> `NonReactive<T> Trait`.
/// 
/// Provides non-reactive operations for sequences, allowing for static
/// manipulations that do not reflect changes across related sequences.
/// 
//// ### -> `Non-Reactivity`
/// 
/// Non-reactive sequences ensure that any modifications made to the original
/// sequence do not affect derived sequences (like slices, reverses, etc.) and vice versa.
/// This is useful in scenarios where isolated data manipulations are required.
///
/// ### -> `Example Scenarios`
/// - Slicing a sequence to create a static view:
///     - Modifying an element in the original sequence does not update the corresponding
///       element in the slice and vice versa.
/// 
/// - Reversing a sequence to create a new static view:
///     - Changes in the original sequence do not reflect in the reversed sequence,
///       and modifications in the reversed sequence do not update the original.
/// 
/// Type Parameters:
/// - `T`: The type of elements stored in the sequence. Must implement `Send`, `Sync`, `Clone`, and have a static lifetime.
///
/// This trait typically works with both Sequence<T> and Arc<Sequence<T>> types,
/// allowing for flexible non-reactive strategies in concurrent environments.
///
/// All methods in this trait are asynchronous and return pinned boxed futures (as async
/// functions cannot be directly part of traits in Rust).
/// 
/// ### -> `Methods`
/// - `extract(range: Option<std::ops::Range<usize>>) -> Arc<Sequence<T>>`:
///    - Asynchronously extracts a subsequence defined by the specified range.
///    - The extracted subsequence does not maintain any link to the original sequence,
///      ensuring that changes in either sequence do not affect the other.
///    - This operation is similar to draining, but the original sequence retains its elements.
/// 
/// - `slice(start: Option<isize>, stop: Option<isize>, step: Option<isize>) -> Arc<Sequence<T>>`:
///    - Asynchronously creates a sliced view of the sequence based on the provided parameters.
///    - This is similar to Python-style slicing, supporting negative indices and steps.
///    - The sliced sequence is non-reactive, meaning modifications in either the original
///      sequence or the slice do not affect each other.
/// 
/// - `split(index: usize) -> (Arc<Sequence<T>>, Arc<Sequence<T>>)`:
///    - Asynchronously splits the sequence into two at the specified index.
///    - Both resulting sequences do not maintain any relationship with the original sequence,
///      ensuring that changes in one do not affect the others.
/// 
/// - `reverse() -> Arc<Sequence<T>>`:
///    - Asynchronously creates a reversed view of the sequence.
///    - The reversed sequence is non-reactive, so modifications in either the original
///      sequence or the reversed sequence do not affect each other.
///
/// ### -> `Usage`
/// 
/// ```
/// use supersonic::sequence::prelude::{Sequence, Allocation, Length, Operation, NonReactive, Candidate};
/// use anyhow::Result;
/// 
/// async fn example() -> Result<()> {
///     let sequence = Sequence::<i32>::allocate(10).await;
///     for i in 0..10 {
///         sequence.append(Candidate::Value(i as i32)).await?;
///     }
///     let reversed = NonReactive::reverse(&sequence).await;
///     assert_eq!(reversed.length(), 10);
///     for i in 0..10 {
///         let value = reversed.get(i as usize, true).await;
///         assert!(!value.empty());
///         assert_eq!(*value.as_arc().await.read().await, (9 - i) as i32);
///     }
///     Ok(())
/// }
/// 
/// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
/// supersonic::future!(example());
/// ```
/// 
/// ### -> `Note`
/// 
/// The methods provided in this traits also contain reactive counterparts under `Reactive<T>` trait.
/// Therefore, ensure to choose the appropriate trait based on whether you require reactive behavior or not.
pub trait NonReactive<T>: Operation<T>
where
    T: Send + Sync + 'static,
    T: Clone,
    Self: Sync,
{

    /// Asynchronously extracts a subsequence from the specified range without removing elements from the original sequence.
    /// - If `range` is `None`, extracts the entire sequence.
    /// - This operation creates a new sequence containing cloned values (not shared references).
    /// - The extracted sequence is non-reactive, meaning changes to the original sequence
    ///   or the extracted sequence do NOT affect each other.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::{Sequence, Allocation, Length, Operation, NonReactive, Candidate};
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(10).await;
    ///     for i in 0..10 {
    ///         sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    /// 
    ///     let extracted = NonReactive::extract(&sequence, Some(2..7)).await;
    ///     assert_eq!(extracted.length(), 5);
    ///     for i in 0..5 {
    ///         let value = extracted.get(i as usize, true).await;
    ///         assert!(!value.empty());
    ///         assert_eq!(*value.as_arc().await.read().await, (i + 2) as i32);
    ///     }
    /// 
    ///     // Check that original sequence remains unchanged
    ///     assert_eq!(sequence.length(), 10);
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    /// 
    /// ### -> `Note`
    /// 
    /// The extracted sequence is non-reactive, meaning modifications in either the original
    /// sequence or the extracted sequence are NOT reflected in each other.
    /// 
    /// This method has a reactive counterpart under `Reactive<T>` trait.
    #[must_use = "Extraction of elements increments reference counts of pre-existing elements without removing them! Must serve a purpose!"]
    fn extract(&self, range: Option<std::ops::Range<usize>>) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>;
    
    /// Asynchronously creates a sliced view of the sequence based on the provided parameters.
    /// - This is similar to Python-style slicing, supporting negative indices and steps.
    /// - The sliced sequence is non-reactive, meaning modifications in either the original
    ///   sequence or the slice are NOT reflected in each other.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::{Sequence, Allocation, Length, Operation, NonReactive, Candidate};
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(10).await;
    ///     for i in 0..10 {
    ///         sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    /// 
    ///     let sliced = NonReactive::slice(&sequence, Some(2), Some(8), Some(2)).await;
    ///     assert_eq!(sliced.length(), 3);
    ///     for i in 0..3 {
    ///         let value = sliced.get(i as usize, true).await;
    ///         assert!(!value.empty());
    ///         assert_eq!(*value.as_arc().await.read().await, (2 + i * 2) as i32);
    ///     }
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    /// 
    /// ### -> `Note`
    /// 
    /// The sliced sequence is non-reactive, meaning modifications in either the original
    /// sequence or the slice are NOT reflected in each other.
    /// 
    /// This method has a reactive counterpart under `Reactive<T>` trait.
    #[must_use = "Slicing is not 0 cost and must serve a purpose!"]
    fn slice(&self, start: Option<isize>, stop: Option<isize>, step: Option<isize>) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>;
    
    /// Asynchronously splits the sequence into two at the specified index.
    /// - Both resulting sequences are non-reactive, maintaining independence from the original sequence.
    /// - Changes in one sequence do NOT affect the others.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::{Sequence, Allocation, Length, Operation, NonReactive, Candidate};
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///    let sequence = Sequence::<i32>::allocate(10).await;
    ///    for i in 0..10 {
    ///        sequence.append(Candidate::Value(i as i32)).await?;
    ///    }
    ///
    ///    let (first_half, second_half) = NonReactive::split(&sequence, 5).await;
    ///    assert_eq!(first_half.length(), 5);
    ///    assert_eq!(second_half.length(), 5);
    ///    for i in 0..5 {
    ///        let value_first = first_half.get(i as usize, true).await;
    ///        assert!(!value_first.empty());
    ///        assert_eq!(*value_first.as_arc().await.read().await, i as i32);
    ///
    ///        let value_second = second_half.get(i as usize, true).await;
    ///        assert!(!value_second.empty());
    ///        assert_eq!(*value_second.as_arc().await.read().await, (i + 5) as i32);
    ///    }
    ///
    ///    Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    /// 
    /// ### -> `Note`
    /// 
    /// The split sequences are non-reactive, meaning modifications in either the original
    /// sequence or the split sequences are NOT reflected in each other.
    /// 
    /// This method has a reactive counterpart under `Reactive<T>` trait.
    #[must_use = "Splitting is not 0 cost and must serve a purpose!"]
    fn split(&self, index: usize) -> Pin<Box<dyn Future<Output = (Arc<Self::SelfType>, Arc<Self::SelfType>)> + Send + '_>>;

    /// Asynchronously creates a new sequence with elements in reversed order.
    /// - The reversed sequence is non-reactive, meaning changes to the original sequence
    ///   or the reversed sequence do NOT affect each other.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::{Sequence, Allocation, Length, Operation, NonReactive, Candidate};
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(10).await;
    ///     for i in 0..10 {
    ///         sequence.append(Candidate::Value(i as i32)).await?;
    ///     }
    /// 
    ///     let reversed = NonReactive::reverse(&sequence).await;
    ///     assert_eq!(reversed.length(), 10);
    ///     for i in 0..10 {
    ///         let value = reversed.get(i as usize, true).await;
    ///         assert!(!value.empty());
    ///         assert_eq!(*value.as_arc().await.read().await, (9 - i) as i32);
    ///     }
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    /// 
    /// ### -> `Note`
    /// 
    /// The reversed sequence is non-reactive, meaning modifications in either the original
    /// sequence or the reversed sequence are NOT reflected in each other.
    /// 
    /// This method has a reactive counterpart under `Reactive<T>` trait.
    #[must_use = "Reversing is not 0 cost and must serve a purpose!"]
    fn reverse(&self) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>;
}


/// ### -> `Stack<T> Trait`.
/// 
/// Provides stack (LIFO - Last In, First Out) operations for sequences,
/// enabling push, pop, peek, and bulk operations on stack-based data structures.
/// This trait implements classic stack semantics with async operations.
/// 
/// Type Parameters:
/// - `T`: The type of elements stored in the stack. Must implement `Send`, `Sync`, and have a static lifetime.
/// 
/// This trait typically works with both Sequence<T> and Arc<Sequence<T>> types,
/// allowing for flexible stack operations in concurrent environments.
/// 
/// All methods in this trait are asynchronous and return pinned boxed futures (as async
/// functions cannot be directly part of traits in Rust).
///
/// ### -> `Methods`
/// - `push(value: Candidate<T>)`:
///     - Asynchronously pushes a value onto the top of the stack.
///     - May trigger resizing if capacity is reached.
///     - `Candidate<T>` can be either a direct value or a pre-built Arc.
/// 
/// - `pop() -> Compat<T>`:
///     - Asynchronously removes and returns the top element from the stack.
///     - Returns a `Compat<T>` which can be empty if the stack is empty.
///     - Decreases the stack size by one.
/// 
/// - `peek() -> Compat<T>`:
///     - Asynchronously returns the top element without removing it.
///     - Returns a `Compat<T>` which can be empty if the stack is empty.
///     - Does not modify the stack.
/// 
/// - `push_n(iter: I, ignore_errors: bool, AoN: bool)`:
///     - Asynchronously pushes multiple elements onto the stack from an iterator.
///     - `ignore_errors`: If true, silently handles overflow; if false, panics on overflow.
///     - `AoN` (All or Nothing): If true, either all elements are pushed or none; if false, pushes as many as fit.
///     - May trigger resizing if capacity is insufficient.
/// 
/// - `pop_n(n: usize, ignore_errors: bool, AoN: bool) -> Arc<Sequence<T>>`:
///     - Asynchronously removes and returns n elements from the top of the stack.
///     - `ignore_errors`: If true, handles underflow gracefully; if false, panics on underflow.
///     - `AoN` (All or Nothing): If true, either pops exactly n or none; if false, pops as many as available.
///     - Returns a new sequence containing the popped elements (non-reactive).
/// 
/// - `swap_top() -> bool`:
///     - Asynchronously swaps the top two elements of the stack.
///     - Returns true if swap was successful, false if stack has fewer than 2 elements.
/// 
/// - `dup() -> bool`:
///     - Asynchronously duplicates the top element of the stack.
///     - Returns true if duplication was successful, false if stack is empty.
///     - Requires T to implement Clone.
/// 
/// ### -> `Usage`
/// 
/// ```
/// use supersonic::sequence::prelude::*;
/// use anyhow::Result;
/// 
/// async fn example() -> Result<()> {
///     let stack = Sequence::<i32>::allocate(10).await;
///     
///     // Push elements
///     stack.push(Candidate::Value(1)).await;
///     stack.push(Candidate::Value(2)).await;
///     stack.push(Candidate::Value(3)).await;
///     
///     assert_eq!(stack.length(), 3);
///     
///     // Peek at top element
///     let top = stack.peek().await;
///     assert!(!top.empty());
///     assert_eq!(*top.as_arc().await.read().await, 3);
///     assert_eq!(stack.length(), 3); // Length unchanged
///     
///     // Pop element
///     let popped = stack.pop().await;
///     assert!(!popped.empty());
///     assert_eq!(*popped.as_arc().await.read().await, 3);
///     assert_eq!(stack.length(), 2);
///     
///     // Swap top two elements
///     let swapped = stack.swap_top().await;
///     assert!(swapped);
///     
///     let top_after_swap = stack.peek().await;
///     assert_eq!(*top_after_swap.as_arc().await.read().await, 1);
///     
///     Ok(())
/// }
/// 
/// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
/// supersonic::future!(example());
/// ```
///
#[allow(non_snake_case)]
pub trait Stack<T> : Allocation<T>
where
    T: Send + Sync + 'static,
    Self: Sync
{
    /// The type defined for consistency across traits.
    /// Represents `Compat<T>`.
    type ArcSwapRef;

    /// Asynchronously pushes a value onto the top of the stack.
    /// - May trigger automatic resizing if the current capacity is reached.
    /// - `Candidate<T>` can be either a direct value or a pre-built Arc.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let stack = Sequence::<i32>::allocate(5).await;
    ///     stack.push(Candidate::Value(10)).await;
    ///     stack.push(Candidate::Value(20)).await;
    ///     
    ///     assert_eq!(stack.length(), 2);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn push(&self, value: Candidate<T>) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
    
    /// Asynchronously removes and returns the top element from the stack.
    /// - Returns a `Compat<T>` which can be empty if the stack is empty.
    /// - Decreases the stack size by one upon successful pop.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let stack = Sequence::<i32>::allocate(5).await;
    ///     stack.push(Candidate::Value(10)).await;
    ///     stack.push(Candidate::Value(20)).await;
    ///     
    ///     let popped = stack.pop().await;
    ///     assert!(!popped.empty());
    ///     assert_eq!(*popped.as_arc().await.read().await, 20);
    ///     assert_eq!(stack.length(), 1);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn pop(&self) -> Pin<Box<dyn Future<Output = Self::ArcSwapRef> + Send + '_>>;

    /// Asynchronously returns the top element from the stack without removing it.
    /// - Returns a `Compat<T>` which can be empty if the stack is empty.
    /// - Does not modify the stack; the element remains at the top.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let stack = Sequence::<i32>::allocate(5).await;
    ///     stack.push(Candidate::Value(10)).await;
    ///     
    ///     let top = stack.peek().await;
    ///     assert!(!top.empty());
    ///     assert_eq!(*top.as_arc().await.read().await, 10);
    ///     assert_eq!(stack.length(), 1); // Stack unchanged
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    #[must_use = "Peeking must serve a purpose!"]
    fn peek(&self) -> Pin<Box<dyn Future<Output = Self::ArcSwapRef> + Send + '_>>;

    /// Asynchronously pushes multiple elements onto the stack from an iterator.
    /// - `ignore_errors`: If true, handles overflow gracefully; if false, panics on overflow.
    /// - `AoN` (All or Nothing): If true, either all elements are pushed or none; if false, pushes as many as fit.
    /// - May trigger automatic resizing if capacity is insufficient.
    /// 
    /// ### -> `Behavior`
    /// - When `AoN = true` and `ignore_errors = false`: Panics if all elements cannot be pushed.
    /// - When `AoN = true` and `ignore_errors = true`: Pushes nothing if overflow would occur.
    /// - When `AoN = false` and `ignore_errors = false`: Panics if any element cannot be pushed.
    /// - When `AoN = false` and `ignore_errors = true`: Pushes as many elements as fit, ignores rest.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let stack = Sequence::<i32>::allocate(10).await;
    ///     
    ///     // Push multiple elements
    ///     stack.push_n(vec![1, 2, 3, 4, 5], false, true).await;
    ///     assert_eq!(stack.length(), 5);
    ///     
    ///     let top = stack.peek().await;
    ///     assert_eq!(*top.as_arc().await.read().await, 5);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn push_n<I>(&self, iter: I, ignore_errors: bool, AoN: bool) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>
    where
        I: IntoIterator<Item = T> + Send + 'static,
        I::IntoIter: Send;

    /// Asynchronously removes and returns n elements from the top of the stack.
    /// - `ignore_errors`: If true, handles underflow gracefully; if false, panics on underflow.
    /// - `AoN` (All or Nothing): If true, either pops exactly n or none; if false, pops as many as available.
    /// - Returns a new sequence containing the popped elements (non-reactive, cloned values).
    /// - Elements in the returned sequence maintain their original order (first popped at index 0).
    /// 
    /// ### -> `Behavior`
    /// - When `AoN = true` and `ignore_errors = false`: Panics if n > stack length.
    /// - When `AoN = true` and `ignore_errors = true`: Returns empty sequence if n > stack length.
    /// - When `AoN = false` and `ignore_errors = false`: Panics if n > stack length.
    /// - When `AoN = false` and `ignore_errors = true`: Pops min(n, length) elements.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let stack = Sequence::<i32>::allocate(10).await;
    ///     stack.push_n(vec![1, 2, 3, 4, 5], false, true).await;
    ///     
    ///     // Pop 3 elements
    ///     let popped = stack.pop_n(3, false, true).await;
    ///     assert_eq!(popped.length(), 3);
    ///     assert_eq!(stack.length(), 2);
    ///     
    ///     // Verify popped elements (in order: 5, 4, 3 -> stored as 5, 4, 3)
    ///     let first = popped.get(0, true).await;
    ///     assert_eq!(*first.as_arc().await.read().await, 3);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn pop_n(&self, n: usize, ignore_errors: bool, AoN: bool) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>
    where
        T: Clone;
    
    /// Asynchronously swaps the top two elements of the stack.
    /// - Returns true if the swap was successful.
    /// - Returns false if the stack has fewer than 2 elements.
    /// - Does not modify the stack size.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let stack = Sequence::<i32>::allocate(5).await;
    ///     stack.push(Candidate::Value(10)).await;
    ///     stack.push(Candidate::Value(20)).await;
    ///     
    ///     let swapped = stack.swap_top().await;
    ///     assert!(swapped);
    ///     
    ///     let top = stack.peek().await;
    ///     assert_eq!(*top.as_arc().await.read().await, 10);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn swap_top(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;

    /// Asynchronously duplicates the top element of the stack.
    /// - Returns true if the duplication was successful.
    /// - Returns false if the stack is empty.
    /// - Increases the stack size by one upon successful duplication.
    /// - Requires T to implement Clone.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let stack = Sequence::<i32>::allocate(5).await;
    ///     stack.push(Candidate::Value(10)).await;
    ///     
    ///     let duplicated = stack.dup().await;
    ///     assert!(duplicated);
    ///     assert_eq!(stack.length(), 2);
    ///     
    ///     let top = stack.peek().await;
    ///     assert_eq!(*top.as_arc().await.read().await, 10);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn dup(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>
    where
        T: Clone;
}

/// ### -> `Queue<T> Trait`.
/// 
/// Provides queue (FIFO - First In, First Out) operations for sequences,
/// enabling enqueue, dequeue, and bulk operations on queue-based data structures.
/// This trait implements classic queue semantics with async operations.
/// 
/// Type Parameters:
/// - `T`: The type of elements stored in the queue. Must implement `Send`, `Sync`, and have a static lifetime.
/// 
/// This trait typically works with both Sequence<T> and Arc<Sequence<T>> types,
/// allowing for flexible queue operations in concurrent environments.
/// 
/// All methods in this trait are asynchronous and return pinned boxed futures (as async
/// functions cannot be directly part of traits in Rust).
///
/// ### -> `Methods`
/// - `enqueue(value: Candidate<T>) -> Result<()>`:
///     - Asynchronously adds a value to the rear of the queue.
///     - May trigger resizing if capacity is reached.
///     - `Candidate<T>` can be either a direct value or a pre-built Arc.
/// 
/// - `dequeue() -> Result<Compat<T>>`:
///     - Asynchronously removes and returns the front element from the queue.
///     - Returns an error if the queue is empty.
///     - Decreases the queue size by one.
/// 
/// - `enqueue_n(iter: I, ignore_errors: bool, AoN: bool) -> Result<()>`:
///     - Asynchronously enqueues multiple elements from an iterator.
///     - `ignore_errors`: If true, silently handles overflow; if false, returns error on overflow.
///     - `AoN` (All or Nothing): If true, either all elements are enqueued or none; if false, enqueues as many as fit.
/// 
/// - `dequeue_n(n: usize, ignore_errors: bool, AoN: bool) -> Result<Arc<Sequence<T>>>`:
///     - Asynchronously removes and returns n elements from the front of the queue.
///     - `ignore_errors`: If true, handles underflow gracefully; if false, returns error on underflow.
///     - `AoN` (All or Nothing): If true, either dequeues exactly n or none; if false, dequeues as many as available.
/// 
/// ### -> `Usage`
/// 
/// ```
/// use supersonic::sequence::prelude::*;
/// use anyhow::Result;
/// 
/// async fn example() -> Result<()> {
///     let queue = Sequence::<i32>::allocate(10).await;
///     
///     // Enqueue elements
///     queue.enqueue(Candidate::Value(1)).await?;
///     queue.enqueue(Candidate::Value(2)).await?;
///     queue.enqueue(Candidate::Value(3)).await?;
///     
///     assert_eq!(queue.length(), 3);
///     
///     // Dequeue element
///     let dequeued = queue.dequeue().await?;
///     assert!(!dequeued.empty());
///     assert_eq!(*dequeued.as_arc().await.read().await, 1);
///     assert_eq!(queue.length(), 2);
///     
///     Ok(())
/// }
/// 
/// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
/// supersonic::future!(example());
/// ```
///
#[allow(non_snake_case)]
pub trait Queue<T>: Allocation<T>
where
    T: Send + Sync + 'static 
{
    /// The type defined for consistency across traits.
    /// Represents `Compat<T>`.
    type ArcSwapRef;

    /// Asynchronously adds a value to the rear of the queue.
    /// - May trigger automatic resizing if the current capacity is reached.
    /// - `Candidate<T>` can be either a direct value or a pre-built Arc.
    /// - Returns an error if the operation fails.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let queue = Sequence::<i32>::allocate(5).await;
    ///     queue.enqueue(Candidate::Value(10)).await?;
    ///     queue.enqueue(Candidate::Value(20)).await?;
    ///     
    ///     assert_eq!(queue.length(), 2);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn enqueue(&self, value: Candidate<T>) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>;
    
    /// Asynchronously removes and returns the front element from the queue.
    /// - Returns an error if the queue is empty.
    /// - Decreases the queue size by one upon successful dequeue.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let queue = Sequence::<i32>::allocate(5).await;
    ///     queue.enqueue(Candidate::Value(10)).await?;
    ///     queue.enqueue(Candidate::Value(20)).await?;
    ///     
    ///     let dequeued = queue.dequeue().await?;
    ///     assert!(!dequeued.empty());
    ///     assert_eq!(*dequeued.as_arc().await.read().await, 10);
    ///     assert_eq!(queue.length(), 1);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn dequeue(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send +'_>>;

    /// Asynchronously enqueues multiple elements from an iterator.
    /// - `ignore_errors`: If true, handles overflow gracefully; if false, returns error on overflow.
    /// - `AoN` (All or Nothing): If true, either all elements are enqueued or none; if false, enqueues as many as fit.
    /// - May trigger automatic resizing if capacity is insufficient.
    /// 
    /// ### -> `Behavior`
    /// - When `AoN = true` and `ignore_errors = false`: Returns error if all elements cannot be enqueued.
    /// - When `AoN = true` and `ignore_errors = true`: Enqueues nothing if overflow would occur.
    /// - When `AoN = false` and `ignore_errors = false`: Returns error if any element cannot be enqueued.
    /// - When `AoN = false` and `ignore_errors = true`: Enqueues as many elements as fit, ignores rest.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let queue = Sequence::<i32>::allocate(10).await;
    ///     
    ///     // Enqueue multiple elements
    ///     queue.enqueue_n(vec![1, 2, 3, 4, 5], false, true).await?;
    ///     assert_eq!(queue.length(), 5);
    ///     
    ///     let front = queue.dequeue().await?;
    ///     assert_eq!(*front.as_arc().await.read().await, 1);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn enqueue_n<I>(&self, iter: I, ignore_errors: bool, AoN: bool) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>
    where
        I: IntoIterator<Item = T> + Send + 'static,
        I::IntoIter: Send;

    /// Asynchronously removes and returns n elements from the front of the queue.
    /// - `ignore_errors`: If true, handles underflow gracefully; if false, returns error on underflow.
    /// - `AoN` (All or Nothing): If true, either dequeues exactly n or none; if false, dequeues as many as available.
    /// - Returns a new sequence containing the dequeued elements (non-reactive, cloned values).
    /// - Elements in the returned sequence maintain their original queue order.
    /// 
    /// ### -> `Behavior`
    /// - When `AoN = true` and `ignore_errors = false`: Returns error if n > queue length.
    /// - When `AoN = true` and `ignore_errors = true`: Returns empty sequence if n > queue length.
    /// - When `AoN = false` and `ignore_errors = false`: Returns error if n > queue length.
    /// - When `AoN = false` and `ignore_errors = true`: Dequeues min(n, length) elements.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let queue = Sequence::<i32>::allocate(10).await;
    ///     queue.enqueue_n(vec![1, 2, 3, 4, 5], false, true).await?;
    ///     
    ///     // Dequeue 3 elements
    ///     let dequeued = queue.dequeue_n(3, false, true).await?;
    ///     assert_eq!(dequeued.length(), 3);
    ///     assert_eq!(queue.length(), 2);
    ///     
    ///     // Verify dequeued elements (1, 2, 3)
    ///     let first = dequeued.get(0, true).await;
    ///     assert_eq!(*first.as_arc().await.read().await, 1);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn dequeue_n(&self, n: usize, ignore_errors: bool, AoN: bool) -> Pin<Box<dyn Future<Output = anyhow::Result<Arc<Self::SelfType>>> + Send + '_>>
    where
        T: Clone;
}

/// ### -> `SnapShot<T> Trait`.
/// 
/// Provides snapshot capabilities for sequences, allowing you to capture the current
/// state of a sequence as a Vec<T> at a specific point in time.
/// 
/// Type Parameters:
/// - `T`: The type of elements stored in the sequence. Must implement `Send`, `Sync`, `Clone`, and have a static lifetime.
/// 
/// This trait typically works with both Sequence<T> and Arc<Sequence<T>> types,
/// allowing for flexible snapshot operations in concurrent environments.
/// 
/// All methods in this trait are asynchronous and return pinned boxed futures (as async
/// functions cannot be directly part of traits in Rust).
///
/// ### -> `Methods`
/// - `snapshot() -> Vec<T>`:
///     - Asynchronously creates a snapshot of the sequence.
///     - Returns a Vec<T> containing cloned values of all elements in the sequence.
///     - The snapshot is independent of the original sequence (non-reactive).
/// 
/// ### -> `Usage`
/// 
/// ```
/// use supersonic::sequence::prelude::*;
/// use anyhow::Result;
/// 
/// async fn example() -> Result<()> {
///     let sequence = Sequence::<i32>::allocate(5).await;
///     for i in 0..5 {
///         sequence.append(Candidate::Value(i as i32)).await?;
///     }
///     
///     // Create a snapshot
///     let snapshot = sequence.snapshot().await;
///     assert_eq!(snapshot.len(), 5);
///     assert_eq!(snapshot, vec![0, 1, 2, 3, 4]);
///     
///     // Modifications to sequence don't affect snapshot
///     sequence.append(Candidate::Value(5)).await?;
///     assert_eq!(snapshot.len(), 5); // Unchanged
///     
///     Ok(())
/// }
/// 
/// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
/// supersonic::future!(example());
/// ```
///
pub trait SnapShot<T>: Allocation<T>
where
    T: Send + Sync + 'static,
    T: Clone,
{
    /// Asynchronously creates a snapshot of the sequence.
    /// - Returns a Vec<T> containing cloned values of all elements.
    /// - The snapshot is independent of the original sequence.
    /// - Changes to the original sequence do not affect the snapshot.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(5).await;
    ///     sequence.append(Candidate::Value(10)).await?;
    ///     sequence.append(Candidate::Value(20)).await?;
    ///     sequence.append(Candidate::Value(30)).await?;
    ///     
    ///     let snapshot = sequence.snapshot().await;
    ///     assert_eq!(snapshot, vec![10, 20, 30]);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    #[must_use = "Snapshot output must serve a purpose!"]
    fn snapshot<'a>(&'a self) -> Pin<Box<dyn Future<Output = Vec<T>> + Send + 'a>>;
}

/// ### -> `Bincode<T> Trait`.
/// 
/// Provides binary serialization and deserialization capabilities for sequences
/// using the bincode format. This trait enables efficient storage and transmission
/// of sequence data.
/// 
/// Type Parameters:
/// - `T`: The type of elements stored in the sequence. Must implement `Send`, `Sync`, `Clone`, and have a static lifetime.
///        For serialization, T must also implement `serde::Serialize`.
///        For deserialization, T must also implement `serde::de::DeserializeOwned`.
/// 
/// This trait requires both `Allocation<T>` and `SnapShot<T>` to be implemented.
/// 
/// All methods in this trait are asynchronous and return pinned boxed futures (as async
/// functions cannot be directly part of traits in Rust).
///
/// ### -> `Methods`
/// - `bincode(configuration: &BincodeConfiguration) -> Result<Vec<u8>>`:
///     - Asynchronously serializes the sequence to binary format.
///     - Returns a Vec<u8> containing the serialized data.
///     - Requires T to implement serde::Serialize.
/// 
/// - `from_bincode(bytes: &Vec<u8>, configuration: &BincodeConfiguration) -> Result<Self::SelfType>`:
///     - Asynchronously deserializes binary data into a new sequence.
///     - Returns a new sequence containing the deserialized elements.
///     - Requires T to implement serde::de::DeserializeOwned.
/// 
/// ### -> `Usage`
/// 
/// ```
/// use supersonic::sequence::prelude::*;
/// use supersonic::BincodeConfiguration;
/// use anyhow::Result;
/// 
/// async fn example() -> Result<()> {
///     let sequence = Sequence::<i32>::allocate(5).await;
///     for i in 0..5 {
///         sequence.append(Candidate::Value(i as i32)).await?;
///     }
///     
///     // Serialize to bincode
///     let config = BincodeConfiguration::Standard;
///     let bytes = sequence.bincode(&config).await?;
///     assert!(!bytes.is_empty());
///     
///     // Deserialize from bincode
///     let deserialized = Sequence::<i32>::from_bincode(&bytes, &config).await?;
///     assert_eq!(deserialized.length(), 5);
///     
///     Ok(())
/// }
/// 
/// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
/// supersonic::future!(example());
/// ```
///
pub trait Bincode<T>: Allocation<T> + SnapShot<T>
where 
    T: Clone,
    T: Send + Sync + 'static,
{

    /// Asynchronously serializes the sequence to binary format using bincode.
    /// - Returns a Vec<u8> containing the serialized binary data.
    /// - Requires T to implement serde::Serialize.
    /// - The configuration parameter allows customization of serialization behavior.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use supersonic::BincodeConfiguration;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(3).await;
    ///     sequence.append(Candidate::Value(10)).await?;
    ///     sequence.append(Candidate::Value(20)).await?;
    ///     sequence.append(Candidate::Value(30)).await?;
    ///     
    ///     let config = BincodeConfiguration::Standard;
    ///     let bytes = sequence.bincode(&config).await?;
    ///     assert!(!bytes.is_empty());
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    #[must_use = "Bincode serialization output must serve a purpose!"]
    fn bincode<'a>(&'a self, configuration: &'a BincodeConfiguration) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<u8>>> + Send + 'a>>
    where 
        T: serde::Serialize;

    /// Asynchronously deserializes binary data into a new sequence.
    /// - Returns a new sequence containing the deserialized elements.
    /// - Requires T to implement serde::de::DeserializeOwned.
    /// - The configuration parameter must match the one used during serialization.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use supersonic::BincodeConfiguration;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(3).await;
    ///     sequence.append(Candidate::Value(10)).await?;
    ///     sequence.append(Candidate::Value(20)).await?;
    ///     sequence.append(Candidate::Value(30)).await?;
    ///     
    ///     let config = BincodeConfiguration::Standard;
    ///     let bytes = sequence.bincode(&config).await?;
    ///     
    ///     // Deserialize
    ///     let deserialized = Sequence::<i32>::from_bincode(&bytes, &config).await?;
    ///     assert_eq!(deserialized.length(), 3);
    ///     
    ///     let val = deserialized.get(0, true).await;
    ///     assert_eq!(*val.as_arc().await.read().await, 10);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn from_bincode<'a>(bytes: &'a Vec<u8>, configuration: &'a BincodeConfiguration) -> Pin<Box<dyn Future<Output = anyhow::Result<Self::SelfType>> + Send + 'a>>
    where
        Self: Sized,
        T: serde::de::DeserializeOwned;
}

/// ### -> `Equality<T> Trait`.
/// 
/// Provides multiple equality comparison strategies for sequences,
/// allowing different levels of precision and performance trade-offs.
/// 
/// Type Parameters:
/// - `T`: The type of elements stored in the sequence. Must implement `PartialEq`, `Send`, `Sync`, `Clone`, and have a static lifetime.
/// 
/// This trait requires both `Allocation<T>` and `SnapShot<T>` to be implemented.
/// 
/// All methods in this trait are asynchronous and return pinned boxed futures (as async
/// functions cannot be directly part of traits in Rust).
///
/// ### -> `Methods`
/// - `atomic_eq(other: &Self) -> bool`:
///     - Performs atomic-level equality comparison.
///     - Compares length and capacity atomically.
///     - Fast but may not detect all differences.
/// 
/// - `try_eq(other: &Self) -> bool`:
///     - Attempts lightweight equality comparison.
///     - Compares basic metadata and may do partial element comparison.
///     - Faster than snapshot_eq but less thorough.
/// 
/// - `snapshot_eq(other: &Self) -> bool`:
///     - Performs deep equality comparison using snapshots.
///     - Takes snapshots of both sequences and compares all elements.
///     - Most accurate but potentially expensive for large sequences.
/// 
/// ### -> `Usage`
/// 
/// ```
/// use supersonic::sequence::prelude::*;
/// use anyhow::Result;
/// 
/// async fn example() -> Result<()> {
///     let seq1 = Sequence::<i32>::allocate(5).await;
///     let seq2 = Sequence::<i32>::allocate(5).await;
///     
///     for i in 0..5 {
///         seq1.append(Candidate::Value(i as i32)).await?;
///         seq2.append(Candidate::Value(i as i32)).await?;
///     }
///     
///     // Atomic equality
///     let atomic_equal = seq1.atomic_eq(&seq2).await;
///     
///     // Deep equality
///     let deep_equal = seq1.snapshot_eq(&seq2).await;
///     assert!(deep_equal);
///     
///     Ok(())
/// }
/// 
/// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
/// supersonic::future!(example());
/// ```
///
pub trait Equality<T> : Allocation<T> + SnapShot<T>
where
    T: PartialEq,
    T: Clone,
    T: Send + Sync + 'static,
{
    /// Performs atomic-level equality comparison.
    /// - Compares length and capacity using atomic operations.
    /// - Fast but may not detect all differences in element values.
    /// - Useful for quick checks when structural equality is sufficient.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let seq1 = Sequence::<i32>::allocate(5).await;
    ///     let seq2 = Sequence::<i32>::allocate(5).await;
    ///     
    ///     seq1.append(Candidate::Value(10)).await?;
    ///     seq2.append(Candidate::Value(10)).await?;
    ///     
    ///     let equal = seq1.atomic_eq(&seq2).await;
    ///     // May return true if lengths match, even if some elements differ
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn atomic_eq<'a>(&'a self, other: &'a Self) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
    
    /// Attempts lightweight equality comparison.
    /// - More thorough than atomic_eq but faster than snapshot_eq.
    /// - May compare basic metadata and sample elements.
    /// - Good balance between performance and accuracy.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let seq1 = Sequence::<i32>::allocate(5).await;
    ///     let seq2 = Sequence::<i32>::allocate(5).await;
    ///     
    ///     for i in 0..3 {
    ///         seq1.append(Candidate::Value(i as i32)).await?;
    ///         seq2.append(Candidate::Value(i as i32)).await?;
    ///     }
    ///     
    ///     let equal = seq1.try_eq(&seq2).await;
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn try_eq<'a>(&'a self, other: &'a Self) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
    
    /// Performs deep equality comparison using snapshots.
    /// - Takes snapshots of both sequences and compares all elements.
    /// - Most accurate equality check, comparing every element.
    /// - More expensive for large sequences as it clones all elements.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let seq1 = Sequence::<i32>::allocate(5).await;
    ///     let seq2 = Sequence::<i32>::allocate(5).await;
    ///     
    ///     for i in 0..5 {
    ///         seq1.append(Candidate::Value(i as i32)).await?;
    ///         seq2.append(Candidate::Value(i as i32)).await?;
    ///     }
    ///     
    ///     let equal = seq1.snapshot_eq(&seq2).await;
    ///     assert!(equal);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn snapshot_eq<'a>(&'a self, other: &'a Self) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
}

/// ### -> `Length Trait`.
/// 
/// Provides length-related operations for sequences, including length retrieval
/// and comparison capabilities. This is a synchronous trait for efficient length checks.
/// 
/// All methods in this trait are synchronous and do not require async/await.
///
/// ### -> `Methods`
/// - `length() -> usize`:
///     - Returns the current number of elements in the sequence.
///     - This is a constant-time operation.
/// 
/// - `length_eq(other: &Self) -> bool`:
///     - Compares if two sequences have the same length.
///     - Returns true if lengths are equal, false otherwise.
/// 
/// - `length_cmp(other: &Self) -> Option<Ordering>`:
///     - Compares the lengths of two sequences.
///     - Returns Some(Ordering) indicating Less, Equal, or Greater.
/// 
/// ### -> `Usage`
/// 
/// ```
/// use supersonic::sequence::prelude::*;
/// use anyhow::Result;
/// 
/// async fn example() -> Result<()> {
///     let seq1 = Sequence::<i32>::allocate(5).await;
///     let seq2 = Sequence::<i32>::allocate(5).await;
///     
///     for i in 0..3 {
///         seq1.append(Candidate::Value(i as i32)).await?;
///     }
///     
///     for i in 0..5 {
///         seq2.append(Candidate::Value(i as i32)).await?;
///     }
///     
///     // Get length
///     assert_eq!(seq1.length(), 3);
///     assert_eq!(seq2.length(), 5);
///     
///     // Compare lengths
///     assert!(!seq1.length_eq(&seq2));
///     
///     let ordering = seq1.length_cmp(&seq2);
///     assert_eq!(ordering, Some(std::cmp::Ordering::Less));
///     
///     Ok(())
/// }
/// 
/// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
/// supersonic::future!(example());
/// ```
///
pub trait Length {
    /// Returns the current number of elements in the sequence.
    /// - This is a constant-time operation (O(1)).
    /// - The value represents the actual number of elements, not the capacity.
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let sequence = Sequence::<i32>::allocate(10).await;
    ///     assert_eq!(sequence.length(), 0);
    ///     
    ///     sequence.append(Candidate::Value(42)).await?;
    ///     assert_eq!(sequence.length(), 1);
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn length(&self) -> usize;
    
    /// Compares if two sequences have the same length.
    /// - Returns true if both sequences have the same number of elements.
    /// - This is a constant-time operation (O(1)).
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// 
    /// async fn example() -> Result<()> {
    ///     let seq1 = Sequence::<i32>::allocate(5).await;
    ///     let seq2 = Sequence::<i32>::allocate(5).await;
    ///     
    ///     seq1.append(Candidate::Value(10)).await?;
    ///     seq2.append(Candidate::Value(20)).await?;
    ///     
    ///     assert!(seq1.length_eq(&seq2)); // Both have length 1
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn length_eq(&self, other: &Self) -> bool;
    
    /// Compares the lengths of two sequences.
    /// - Returns Some(Ordering::Less) if self.length() < other.length().
    /// - Returns Some(Ordering::Equal) if self.length() == other.length().
    /// - Returns Some(Ordering::Greater) if self.length() > other.length().
    /// - This is a constant-time operation (O(1)).
    /// 
    /// ### -> `Usage`
    /// 
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// use anyhow::Result;
    /// use std::cmp::Ordering;
    /// 
    /// async fn example() -> Result<()> {
    ///     let seq1 = Sequence::<i32>::allocate(5).await;
    ///     let seq2 = Sequence::<i32>::allocate(5).await;
    ///     
    ///     seq1.append(Candidate::Value(10)).await?;
    ///     seq2.append(Candidate::Value(20)).await?;
    ///     seq2.append(Candidate::Value(30)).await?;
    ///     
    ///     let ordering = seq1.length_cmp(&seq2);
    ///     assert_eq!(ordering, Some(Ordering::Less));
    ///     
    ///     Ok(())
    /// }
    /// 
    /// // to run asynchronous code blockingly in doctest (as doctest does not support async natively)
    /// supersonic::future!(example());
    /// ```
    fn length_cmp(&self, other: &Self) -> Option<std::cmp::Ordering>;
}
