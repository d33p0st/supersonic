
use crate::sequence::prelude::*;

#[tokio::test]
async fn reverse_reactive() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 0..10 {
        sequence.append(Candidate::Value(i)).await?;
    }

    let reversed = Reactive::reverse(&sequence).await;
    assert_eq!(reversed.length(), 10);
    for i in 0..10 {
        let value = reversed.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, 9 - i);
    }

    // now check modification in original sequence changes reversed sequence
    // and vice versa
    // must use -> modify fn, not set fn
    
    // Modify original sequence at index 3 (value should be 3)
    sequence.modify(3, 100).await?;
    
    // Check that reversed sequence at index 6 (which points to original index 3) reflects the change
    let value_in_reversed = reversed.get(6, true).await;
    assert!(!value_in_reversed.empty());
    assert_eq!(*value_in_reversed.as_arc().await.read().await, 100, "Modification in original should reflect in reversed");
    
    // Modify reversed sequence at index 2 (which points to original index 7)
    reversed.modify(2, 200).await?;
    
    // Check that original sequence at index 7 reflects the change
    let value_in_original = sequence.get(7, true).await;
    assert!(!value_in_original.empty());
    assert_eq!(*value_in_original.as_arc().await.read().await, 200, "Modification in reversed should reflect in original");

    Ok(())
}

#[tokio::test]
async fn reverse_nonreactive() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 0..10 {
        sequence.append(Candidate::Value(i)).await?;
    }

    let reversed = NonReactive::reverse(&sequence).await;
    assert_eq!(reversed.length(), 10);
    for i in 0..10 {
        let value = reversed.get(i as usize, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, 9 - i);
    }

    // now check modification in original sequence changes reversed sequence
    // and vice versa
    // must use -> modify fn, not set fn
    
    // Modify original sequence at index 3 (value should be 3)
    sequence.modify(3, 100).await?;
    
    // Check that reversed sequence at index 6 (which points to original index 3) reflects the change
    let value_in_reversed = reversed.get(6, true).await;
    assert!(!value_in_reversed.empty());
    assert_ne!(*value_in_reversed.as_arc().await.read().await, 100, "Modification in original should not reflect in reversed");
    
    // Modify reversed sequence at index 2 (which points to original index 7)
    reversed.modify(2, 200).await?;
    
    // Check that original sequence at index 7 reflects the change
    let value_in_original = sequence.get(7, true).await;
    assert!(!value_in_original.empty());
    assert_ne!(*value_in_original.as_arc().await.read().await, 200, "Modification in reversed should not reflect in original");

    Ok(())
}