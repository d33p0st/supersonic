
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

pub trait NonReactive<T>: Operation<T>
where
    T: Send + Sync + 'static,
    T: Clone,
    Self: Sync,
{

    #[must_use = "Extraction of elements increments reference counts of pre-existing elements without removing them! Must serve a purpose!"]
    fn extract(&self, range: Option<std::ops::Range<usize>>) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>;
    
    #[must_use = "Slicing is not 0 cost and must serve a purpose!"]
    fn slice(&self, start: Option<isize>, stop: Option<isize>, step: Option<isize>) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>;
    
    #[must_use = "Splitting is not 0 cost and must serve a purpose!"]
    fn split(&self, index: usize) -> Pin<Box<dyn Future<Output = (Arc<Self::SelfType>, Arc<Self::SelfType>)> + Send + '_>>;

    #[must_use = "Reversing is not 0 cost and must serve a purpose!"]
    fn reverse(&self) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>;
}


pub trait Stack<T> : Allocation<T>
where
    T: Send + Sync + 'static,
    Self: Sync
{
    type ArcSwapRef;

    fn push(&self, value: Candidate<T>) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
    fn pop(&self) -> Pin<Box<dyn Future<Output = Self::ArcSwapRef> + Send + '_>>;

    #[must_use = "Peeking must serve a purpose!"]
    fn peek(&self) -> Pin<Box<dyn Future<Output = Self::ArcSwapRef> + Send + '_>>;

    fn push_n<I>(&self, iter: I) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>
    where
        I: IntoIterator<Item = T> + Send + 'static,
        I::IntoIter: Send;

    fn pop_n(&self, n: usize) -> Pin<Box<dyn Future<Output = Arc<Self::SelfType>> + Send + '_>>
    where
        T: Clone;
    
    fn swap_top(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;

    fn dup(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>
    where
        T: Clone;
}

pub trait Queue<T>: Allocation<T>
where
    T: Send + Sync + 'static 
{
    type ArcSwapRef;

    fn enqueue(&self, value: Candidate<T>) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>;
    fn dequeue(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<Self::ArcSwapRef>> + Send +'_>>;

    #[allow(non_snake_case)]
    fn enqueue_n<I>(&self, iter: I, ignore_errors: bool, AoN: bool) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>
    where
        I: IntoIterator<Item = T> + Send + 'static,
        I::IntoIter: Send;

    #[allow(non_snake_case)]
    fn dequeue_n(&self, n: usize, ignore_errors: bool, AoN: bool) -> Pin<Box<dyn Future<Output = anyhow::Result<Arc<Self::SelfType>>> + Send + '_>>
    where
        T: Clone;
}

pub trait SnapShot<T>: Allocation<T>
where
    T: Send + Sync + 'static,
    T: Clone,
{
    #[must_use = "Snapshot output must serve a purpose!"]
    fn snapshot<'a>(&'a self) -> Pin<Box<dyn Future<Output = Vec<T>> + Send + 'a>>;
}

pub trait Bincode<T>: Allocation<T> + SnapShot<T>
where 
    T: Clone,
    T: Send + Sync + 'static,
{

    #[must_use = "Bincode serialization output must serve a purpose!"]
    fn bincode<'a>(&'a self, configuration: &'a BincodeConfiguration) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<u8>>> + Send + 'a>>
    where 
        T: serde::Serialize;

    fn from_bincode<'a>(bytes: &'a Vec<u8>, configuration: &'a BincodeConfiguration) -> Pin<Box<dyn Future<Output = anyhow::Result<Self::SelfType>> + Send + 'a>>
    where
        Self: Sized,
        T: serde::de::DeserializeOwned;
}

pub trait Equality<T> : Allocation<T> + SnapShot<T>
where
    T: PartialEq,
    T: Clone,
    T: Send + Sync + 'static,
{
    fn atomic_eq<'a>(&'a self, other: &'a Self) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
    fn try_eq<'a>(&'a self, other: &'a Self) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
    fn snapshot_eq<'a>(&'a self, other: &'a Self) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
}

pub trait Length {
    fn length(&self) -> usize;
    fn length_eq(&self, other: &Self) -> bool;
    fn length_cmp(&self, other: &Self) -> Option<std::cmp::Ordering>;
}
