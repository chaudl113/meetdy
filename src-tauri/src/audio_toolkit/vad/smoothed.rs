use super::{VadFrame, VoiceActivityDetector};
use anyhow::Result;

pub struct SmoothedVad {
    inner_vad: Box<dyn VoiceActivityDetector>,
    #[allow(dead_code)]
    prefill_frames: usize,
    hangover_frames: usize,
    onset_frames: usize,

    /// Fixed-size ring buffer of pre-allocated Vec<f32> to avoid allocating
    /// a new Vec every 30ms frame in push_frame().
    frame_buffer: Vec<Vec<f32>>,
    frame_buffer_write_idx: usize,
    frame_buffer_count: usize,

    hangover_counter: usize,
    onset_counter: usize,
    in_speech: bool,

    temp_out: Vec<f32>,
}

impl SmoothedVad {
    pub fn new(
        inner_vad: Box<dyn VoiceActivityDetector>,
        prefill_frames: usize,
        hangover_frames: usize,
        onset_frames: usize,
    ) -> Self {
        let ring_capacity = prefill_frames + 1;
        Self {
            inner_vad,
            prefill_frames,
            hangover_frames,
            onset_frames,
            frame_buffer: (0..ring_capacity).map(|_| Vec::new()).collect(),
            frame_buffer_write_idx: 0,
            frame_buffer_count: 0,
            hangover_counter: 0,
            onset_counter: 0,
            in_speech: false,
            temp_out: Vec::new(),
        }
    }
}

impl VoiceActivityDetector for SmoothedVad {
    fn push_frame<'a>(&'a mut self, frame: &'a [f32]) -> Result<VadFrame<'a>> {
        // 1. Copy frame data into the current ring buffer slot (no allocation)
        let cap = self.frame_buffer.len();
        let slot = &mut self.frame_buffer[self.frame_buffer_write_idx];
        slot.clear();
        slot.extend_from_slice(frame);
        self.frame_buffer_write_idx = (self.frame_buffer_write_idx + 1) % cap;
        if self.frame_buffer_count < cap {
            self.frame_buffer_count += 1;
        }

        // 2. Delegate to the wrapped boolean VAD
        let is_voice = self.inner_vad.is_voice(frame)?;

        match (self.in_speech, is_voice) {
            // Potential start of speech - need to accumulate onset frames
            (false, true) => {
                self.onset_counter += 1;
                if self.onset_counter >= self.onset_frames {
                    // We have enough consecutive voice frames to trigger speech
                    self.in_speech = true;
                    self.hangover_counter = self.hangover_frames;
                    self.onset_counter = 0; // Reset for next time

                    // Collect prefill + current frame from ring buffer
                    self.temp_out.clear();
                    for i in 0..self.frame_buffer_count {
                        let idx = (self.frame_buffer_write_idx + cap - self.frame_buffer_count + i) % cap;
                        self.temp_out.extend(&self.frame_buffer[idx]);
                    }
                    Ok(VadFrame::Speech(&self.temp_out))
                } else {
                    // Not enough frames yet, still silence
                    Ok(VadFrame::Noise)
                }
            }

            // Ongoing Speech
            (true, true) => {
                self.hangover_counter = self.hangover_frames;
                Ok(VadFrame::Speech(frame))
            }

            // End of Speech or interruption during onset phase
            (true, false) => {
                if self.hangover_counter > 0 {
                    self.hangover_counter -= 1;
                    Ok(VadFrame::Speech(frame))
                } else {
                    self.in_speech = false;
                    Ok(VadFrame::Noise)
                }
            }

            // Silence or broken onset sequence
            (false, false) => {
                self.onset_counter = 0; // Reset onset counter on silence
                Ok(VadFrame::Noise)
            }
        }
    }

    fn reset(&mut self) {
        self.frame_buffer_write_idx = 0;
        self.frame_buffer_count = 0;
        for slot in &mut self.frame_buffer {
            slot.clear();
        }
        self.hangover_counter = 0;
        self.onset_counter = 0;
        self.in_speech = false;
        self.temp_out.clear();
    }
}
