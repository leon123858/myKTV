use crate::types::AudioUnit;
use std::f32::consts::PI;

/// Supported biquad filter types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    /// Low-pass filter: Allows frequencies below the cutoff to pass.
    LowPass,
    /// High-pass filter: Allows frequencies above the cutoff to pass.
    HighPass,
    /// Band-pass filter: Allows frequencies within a range around the cutoff to pass.
    BandPass,
}

/// Biquad IIR filter coefficients (Direct Form I)
#[derive(Debug, Clone, Copy)]
struct BiquadCoefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

/// Per-channel delay state
#[derive(Debug, Clone, Copy, Default)]
struct ChannelState {
    x1: f32, // input z^-1
    x2: f32, // input z^-2
    y1: f32, // output z^-1
    y2: f32, // output z^-2
}

/// A biquad IIR filter node.
///
/// `FilterNode` provides standard LowPass, HighPass, and BandPass filtering.
/// It supports dynamic updates of cutoff frequency and Q factor via
/// [`ControlMessage::SetParameter`](crate::graph::ControlMessage::SetParameter).
///
/// # Example
/// ```no_run
/// use rust_audio_api::nodes::{FilterNode, FilterType, NodeType};
/// use rust_audio_api::{AudioContext, NodeParameter};
///
/// let mut ctx = AudioContext::new().unwrap();
/// let sample_rate = ctx.sample_rate();
///
/// let mut filter_id = None;
/// let dest_id = ctx.build_graph(|builder| {
///     let filter = FilterNode::new(FilterType::LowPass, sample_rate, 1000.0, 0.707);
///     let id = builder.add_node(NodeType::Filter(filter));
///     filter_id = Some(id);
///     id
/// });
///
/// // Dynamically sweep the filter cutoff frequency to 2000 Hz
/// ctx.control_sender().send(
///     rust_audio_api::graph::ControlMessage::SetParameter(
///         filter_id.unwrap(),
///         NodeParameter::Cutoff(2000.0)
///     )
/// ).unwrap();
/// ```
pub struct FilterNode {
    filter_type: FilterType,
    sample_rate: f32,
    cutoff: f32,
    q: f32,
    coeffs: BiquadCoefficients,
    state: [ChannelState; 2], // L / R
}

impl FilterNode {
    /// Creates a new `FilterNode`.
    ///
    /// # Parameters
    /// - `filter_type`: The type of filter ([`FilterType`]).
    /// - `sample_rate`: Processing sample rate.
    /// - `cutoff_hz`: Cutoff frequency in Hz.
    /// - `q`: Quality factor (Resonance).
    pub fn new(filter_type: FilterType, sample_rate: u32, cutoff_hz: f32, q: f32) -> Self {
        let mut node = Self {
            filter_type,
            sample_rate: sample_rate as f32,
            cutoff: cutoff_hz,
            q,
            coeffs: BiquadCoefficients {
                b0: 0.0,
                b1: 0.0,
                b2: 0.0,
                a1: 0.0,
                a2: 0.0,
            },
            state: [ChannelState::default(); 2],
        };
        node.recalculate_coefficients();
        node
    }

    /// Sets the cutoff frequency (updates coefficients automatically).
    pub fn set_cutoff(&mut self, cutoff_hz: f32) {
        self.cutoff = cutoff_hz;
        self.recalculate_coefficients();
    }

    /// Sets the quality factor Q (updates coefficients automatically).
    pub fn set_q(&mut self, q: f32) {
        self.q = q;
        self.recalculate_coefficients();
    }

    /// Sets the filter type (updates coefficients automatically).
    pub fn set_filter_type(&mut self, filter_type: FilterType) {
        self.filter_type = filter_type;
        self.recalculate_coefficients();
    }

    /// Calculates biquad coefficients based on Audio Cookbook (Robert Bristow-Johnson) formulas
    fn recalculate_coefficients(&mut self) {
        let w0 = 2.0 * PI * self.cutoff / self.sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * self.q);

        let (b0, b1, b2, a0, a1, a2) = match self.filter_type {
            FilterType::LowPass => {
                let b1 = 1.0 - cos_w0;
                let b0 = b1 / 2.0;
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighPass => {
                let b1_raw = 1.0 + cos_w0;
                let b0 = b1_raw / 2.0;
                let b1 = -(1.0 + cos_w0);
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::BandPass => {
                let b0 = alpha;
                let b1 = 0.0;
                let b2 = -alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
        };

        // Normalization: divide all coefficients by a0
        let inv_a0 = 1.0 / a0;
        self.coeffs = BiquadCoefficients {
            b0: b0 * inv_a0,
            b1: b1 * inv_a0,
            b2: b2 * inv_a0,
            a1: a1 * inv_a0,
            a2: a2 * inv_a0,
        };
    }

    /// Performs Direct Form I biquad filtering on a single sample
    #[inline(always)]
    fn process_sample(coeffs: &BiquadCoefficients, state: &mut ChannelState, x: f32) -> f32 {
        let y = coeffs.b0 * x + coeffs.b1 * state.x1 + coeffs.b2 * state.x2
            - coeffs.a1 * state.y1
            - coeffs.a2 * state.y2;

        state.x2 = state.x1;
        state.x1 = x;
        state.y2 = state.y1;
        state.y1 = y;

        y
    }

    #[inline(always)]
    pub fn process(&mut self, input: Option<&AudioUnit>, output: &mut AudioUnit) {
        if let Some(in_unit) = input {
            let coeffs = self.coeffs;
            output.copy_from_slice(in_unit);

            dasp::slice::map_in_place(&mut output[..], |frame| {
                let left = Self::process_sample(&coeffs, &mut self.state[0], frame[0]);
                let right = Self::process_sample(&coeffs, &mut self.state[1], frame[1]);
                [left, right]
            });
        } else {
            dasp::slice::equilibrium(&mut output[..]);
        }
    }
}
