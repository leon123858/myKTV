use dasp::signal::Signal;
use ringbuf::HeapRb;
use ringbuf::traits::{Producer, Split};
use rust_audio_api::nodes::resampler::RingIter;

#[test]
fn test_ring_iter_mono() {
    let ringbuf = HeapRb::<f32>::new(100);
    let (mut prod, cons) = ringbuf.split();

    prod.try_push(0.5).unwrap();
    prod.try_push(0.7).unwrap();

    let mut iter = RingIter {
        consumer: cons,
        channels: 1,
    };

    let frame1 = iter.next();
    assert_eq!(frame1, [0.5, 0.5]);

    let frame2 = iter.next();
    assert_eq!(frame2, [0.7, 0.7]);
}

#[test]
fn test_ring_iter_stereo() {
    let ringbuf = HeapRb::<f32>::new(100);
    let (mut prod, cons) = ringbuf.split();

    prod.try_push(0.1).unwrap(); // L1
    prod.try_push(0.2).unwrap(); // R1
    prod.try_push(0.3).unwrap(); // L2
    prod.try_push(0.4).unwrap(); // R2

    let mut iter = RingIter {
        consumer: cons,
        channels: 2,
    };

    let frame1 = iter.next();
    assert_eq!(frame1, [0.1, 0.2]);

    let frame2 = iter.next();
    assert_eq!(frame2, [0.3, 0.4]);
}

#[test]
fn test_ring_iter_multichannel() {
    let ringbuf = HeapRb::<f32>::new(100);
    let (mut prod, cons) = ringbuf.split();

    // 4 channels, one frame
    prod.try_push(0.1).unwrap();
    prod.try_push(0.2).unwrap();
    prod.try_push(0.3).unwrap();
    prod.try_push(0.4).unwrap();

    let mut iter = RingIter {
        consumer: cons,
        channels: 4,
    };

    let frame1 = iter.next();
    // Avg of 0.1, 0.2, 0.3, 0.4 is 0.25 (well, in f32)
    assert_eq!(frame1, [0.25, 0.25]);
}

#[test]
fn test_resampling_5hz_to_10hz() {
    let ringbuf = HeapRb::<f32>::new(1000);
    let (mut prod, cons) = ringbuf.split();

    // Push 500 samples of 1.0
    for _ in 0..500 {
        prod.try_push(1.0).unwrap();
    }

    let mut ring_iter = RingIter {
        consumer: cons,
        channels: 1,
    };

    // Verify RingIter works independently
    let first_frame = ring_iter.next();
    assert_eq!(
        first_frame,
        [1.0, 1.0],
        "RingIter should return the pushed samples"
    );

    // Re-initialize for the converter test with remaining samples
    let ring_buffer = dasp::ring_buffer::Fixed::from([[0.0; 2]; 100]);
    let sinc = dasp::interpolate::sinc::Sinc::new(ring_buffer);
    let mut converter = ring_iter.from_hz_to_hz(sinc, 5.0, 10.0);

    let mut output = Vec::new();
    // Pull many samples to get past any filter latency
    for _ in 0..400 {
        let frame = converter.next();
        output.push(frame[0]);
    }

    let has_signal = output.iter().any(|&x| x > 0.1);
    assert!(
        has_signal,
        "Output should eventually contain non-zero samples. Last 10 samples: {:?}",
        &output[390..]
    );
}
