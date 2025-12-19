
use crate::sequence::prelude::*;

#[tokio::test]
async fn slice() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 0..10 {
        sequence.append(Candidate::Value(i)).await?;
    }

    // Test positive step slicing [2:8:2] -> indices 2, 4, 6
    let sliced = Reactive::slice(&sequence, Some(2), Some(8), Some(2)).await;
    assert_eq!(sliced.length(), 3);
    
    let val0 = sliced.get(0, true).await;
    assert!(!val0.empty());
    assert_eq!(*val0.as_arc().await.read().await, 2);
    
    let val1 = sliced.get(1, true).await;
    assert!(!val1.empty());
    assert_eq!(*val1.as_arc().await.read().await, 4);
    
    let val2 = sliced.get(2, true).await;
    assert!(!val2.empty());
    assert_eq!(*val2.as_arc().await.read().await, 6);

    // Test reactivity: modify original sequence at index 4
    sequence.modify(4, 100).await?;
    
    // Check that sliced sequence at index 1 (which points to original index 4) reflects the change
    let val_modified = sliced.get(1, true).await;
    assert!(!val_modified.empty());
    assert_eq!(*val_modified.as_arc().await.read().await, 100, "Modification in original should reflect in sliced");
    
    // Test reactivity in reverse: modify sliced sequence at index 2 (original index 6)
    sliced.modify(2, 200).await?;
    
    // Check that original sequence at index 6 reflects the change
    let val_in_original = sequence.get(6, true).await;
    assert!(!val_in_original.empty());
    assert_eq!(*val_in_original.as_arc().await.read().await, 200, "Modification in sliced should reflect in original");

    Ok(())
}

#[tokio::test]
async fn slice_negative_step() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 0..10 {
        sequence.append(Candidate::Value(i)).await?;
    }

    // Test negative step slicing [8:2:-2] -> indices 8, 6, 4
    let sliced = Reactive::slice(&sequence, Some(8), Some(2), Some(-2)).await;
    assert_eq!(sliced.length(), 3);
    
    let val0 = sliced.get(0, true).await;
    assert!(!val0.empty());
    assert_eq!(*val0.as_arc().await.read().await, 8);
    
    let val1 = sliced.get(1, true).await;
    assert!(!val1.empty());
    assert_eq!(*val1.as_arc().await.read().await, 6);
    
    let val2 = sliced.get(2, true).await;
    assert!(!val2.empty());
    assert_eq!(*val2.as_arc().await.read().await, 4);

    // Test reactivity with negative step
    sequence.modify(6, 300).await?;
    
    let val_modified = sliced.get(1, true).await;
    assert!(!val_modified.empty());
    assert_eq!(*val_modified.as_arc().await.read().await, 300, "Modification in original should reflect in negative step slice");

    Ok(())
}

#[tokio::test]
async fn slice_default_params() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 0..10 {
        sequence.append(Candidate::Value(i)).await?;
    }

    // Test default parameters [::] -> full sequence
    let sliced = Reactive::slice(&sequence, None, None, None).await;
    assert_eq!(sliced.length(), 10);
    
    for i in 0..10 {
        let value = sliced.get(i, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, i as u32);
    }

    // Test default start/stop with step [::2] -> indices 0, 2, 4, 6, 8
    let sliced_step2 = Reactive::slice(&sequence, None, None, Some(2)).await;
    assert_eq!(sliced_step2.length(), 5);
    
    for i in 0..5 {
        let value = sliced_step2.get(i, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, (i * 2) as u32);
    }

    Ok(())
}

#[tokio::test]
async fn slice_negative_indices() -> anyhow::Result<()> {
    let sequence = Sequence::<u32>::allocate(10).await;
    for i in 0..10 {
        sequence.append(Candidate::Value(i)).await?;
    }

    // Test negative indices [-5:-1:1] -> indices 5, 6, 7, 8
    let sliced = Reactive::slice(&sequence, Some(-5), Some(-1), Some(1)).await;
    assert_eq!(sliced.length(), 4);
    
    for i in 0..4 {
        let value = sliced.get(i, true).await;
        assert!(!value.empty());
        assert_eq!(*value.as_arc().await.read().await, (5 + i) as u32);
    }

    Ok(())
}