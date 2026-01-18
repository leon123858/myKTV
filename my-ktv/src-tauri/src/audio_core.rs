pub mod audio_kernel;
pub mod const_number;

pub struct SendWrapper<T>(pub T);

unsafe impl<T> Send for SendWrapper<T> {}
unsafe impl<T> Sync for SendWrapper<T> {}

pub struct AudioKernel {
    _output_stream: cpal::Stream,
    _input_stream: Option<cpal::Stream>,
    pub audio_producer: rtrb::Producer<f32>,
}
