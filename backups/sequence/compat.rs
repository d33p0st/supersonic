use tokio::sync::RwLock;
use arc_swap::ArcSwapOption;


/// A compatibility wrapper around internal `ArcSwapOption<_>`
/// that preserves the value semantics expected by the user.
/// 
/// ## Usage
/// ```
/// use supersonic::sequence::prelude::*;
///
/// async fn doctest() -> anyhow::Result<()> {
///     // allocate a sequence of i32
///     let sequence = Sequence::<i32>::allocate(1).await;
/// 
///     // insert one element
///     sequence.insert(0, Candidate::Value(42)).await;
/// 
///     let value = sequence.get(0, true).await; // this is of type Compat<i32>
///     // compat struct provides easy access over the value.
///     if !value.empty() {
///        let arc = value.as_arc().await; // Arc<RwLock<i32>>
///        // where modifying the value inside the RwLock also updates the value in the sequence.
///        // This is termed as "Reaction" and the property as "Reactivity" (see Sequence Docs).
///         
///         {
///             let mut write_guard = arc.write().await;
///             *write_guard += 1; // increment the value inside the RwLock
///         }
///     }
/// 
///     let updated_value = sequence.get(0, true).await;
///     let arc = updated_value.as_arc().await;
///     let read_guard = arc.read().await;
///     assert_eq!(*read_guard, 43); // value has been updated reactively
/// 
///     Ok(())
/// } 
///
/// // execution for doctest:
/// supersonic::future!(doctest());
/// ```
pub struct Compat<T> {
    value: ArcSwapOption<RwLock<T>>
}

impl <T> Compat<T>
where
    T: Send + Sync + 'static
{

    /// Compat creator -> not meant for user consumption directly.
    pub async fn create(with: ArcSwapOption<RwLock<T>>) -> Self {
        Self { value: with }
    }

    /// Returns the Arc<RwLock<T>> for the element if it exists otherwise results into panic.
    /// Therefore, it is good practive to check if the value is empty before requesting the arc.
    /// 
    /// ## Usage
    /// ```
    /// use supersonic::sequence::prelude::*;
    /// 
    /// async fn doctest() -> anyhow::Result<()> {
    ///     // allocate a sequence of i32
    ///     let sequence = Sequence::<i32>::allocate(1).await;
    /// 
    ///     // insert one element
    ///     sequence.insert(0, Candidate::Value(42)).await;
    /// 
    ///     // get the element
    ///     let value = sequence.get(0, true).await; // this is of type Compat<i32>
    /// 
    ///     // check if value is not empty
    ///     if !value.empty() {
    ///         let arc = value.as_arc().await; // Arc<RwLock<i32>>
    ///         let read_guard = arc.read().await;
    ///         assert_eq!(*read_guard, 42);
    ///     }
    /// 
    ///     Ok(())
    /// }
    /// 
    /// // execution for doctest:
    /// supersonic::future!(doctest());
    /// ```
    pub async fn as_arc(&self) -> std::sync::Arc<RwLock<T>> {
        if let Some(arc) = self.value.load_full() {
            return arc;
        } else {
            panic!("Compat: value is None!");
        }
    }

    /// Checks if the Compat wrapper is empty (i.e. contains no value).
    pub fn empty(&self) -> bool {
        self.value.load_full().is_none()
    }

}