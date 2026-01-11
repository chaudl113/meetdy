# Meeting Stability Fix - Implementation Plan

**Date:** 2026-01-11
**Strategy:** Phương án 1 (Incremental Fix)
**Estimated Time:** 6-8 hours
**Risk Level:** Low-Medium

---

## 📋 Executive Summary

Fix 4 vấn đề critical/stability trong hệ thống meeting recording:

1. **Event Listener Cleanup** (Frontend) - Memory leak prevention
2. **State Synchronization** (Frontend) - Duration accuracy
3. **WAV Race Condition** (Backend) - Recording reliability
4. **Mic Disconnect Handler** (Backend) - Error resilience

**Approach:** Phase 1A (Frontend quick wins) → Phase 1B (Backend refactor)

---

## Phase 1A: Frontend Quick Wins (2h)

### Task 1A.1: Event Listener Cleanup (Abort Pattern)

**Problem:** Async race condition - event listeners không cleanup khi component unmount → memory leak

**Files to modify:**

- `src/stores/meetingStore.ts` (lines 22-58, 292-381)

**Implementation:**

```typescript
// 1. Add _initId to store state
interface MeetingStore {
  _initId: number;
  // ... existing fields
}

// 2. Implement abort pattern in initializeEventListeners
initializeEventListeners: async () => {
  const { cleanupEventListeners } = get();

  // Cleanup existing
  cleanupEventListeners();

  // Generate new ID
  const initId = Date.now();
  set({ _initId: initId });

  const unlisteners: UnlistenFn[] = [];

  // Helper to check validity
  const isValid = () => get()._initId === initId;

  try {
    const startedUnlisten = await listen<MeetingSession>("meeting_started", (event) => {
      if (!isValid()) return; // Abort if invalidated
      const session = event.payload;
      setCurrentSession(session);
      setSessionStatus("recording");
      _startDurationTimer();
    });

    if (!isValid()) {
      startedUnlisten(); // Cleanup if invalidated
      return;
    }
    unlisteners.push(startedUnlisten);

    // ... repeat for other listeners (meeting_stopped, meeting_processing, etc.)

    // Only commit if still valid
    if (isValid()) {
      set({
        _eventUnlisteners: unlisteners,
        _visibilityHandler: handleVisibilityChange
      });
    }
  } catch (e) {
    console.error("Failed to initialize listeners:", e);
  }
},

// 3. Update cleanup to invalidate pending inits
cleanupEventListeners: () => {
  set({ _initId: 0 }); // Invalidate all pending inits
  const { _eventUnlisteners, _visibilityHandler } = get();

  // Unsubscribe from Tauri events
  _eventUnlisteners.forEach(unlisten => unlisten());

  // Remove visibility change listener
  if (_visibilityHandler) {
    document.removeEventListener("visibilitychange", _visibilityHandler);
  }

  set({
    _eventUnlisteners: [],
    _visibilityHandler: null,
  });
},
```

**Testing:**

- [ ] Unit test: Mock `listen` with delay, call init → cleanup → verify no listeners active
- [ ] Integration: Mount/unmount `MeetingMode` 10 times, check memory heap

---

### Task 1A.2: State Synchronization

**Problem:** `recordingDuration` (frontend timer) không sync với `session.duration` (backend accurate) → hiển thị sai

**Files to modify:**

- `src/stores/meetingStore.ts` (lines 308-365)

**Implementation:**

```typescript
// Update ALL event listeners to sync duration

// meeting_started
const startedUnlisten = await listen<MeetingSession>(
  "meeting_started",
  (event) => {
    if (!isValid()) return;
    const session = event.payload;
    setCurrentSession(session);
    setSessionStatus("recording");
    _startDurationTimer();
    // ✅ NEW: Sync duration if available
    if (session.duration !== undefined && session.duration !== null) {
      setRecordingDuration(session.duration);
    }
  },
);

// meeting_stopped
const stoppedUnlisten = await listen<MeetingSession>(
  "meeting_stopped",
  (event) => {
    if (!isValid()) return;
    const session = event.payload;
    setCurrentSession(session);
    _stopDurationTimer();
    // ✅ NEW: Sync duration
    if (session.duration !== undefined && session.duration !== null) {
      setRecordingDuration(session.duration);
    }
  },
);

// meeting_processing
const processingUnlisten = await listen<MeetingSession>(
  "meeting_processing",
  (event) => {
    if (!isValid()) return;
    const session = event.payload;
    setCurrentSession(session);
    setSessionStatus("processing");
    _stopDurationTimer();
    // ✅ NEW: Sync duration
    if (session.duration !== undefined && session.duration !== null) {
      setRecordingDuration(session.duration);
    }
  },
);

// meeting_completed
const completedUnlisten = await listen<MeetingSession>(
  "meeting_completed",
  (event) => {
    if (!isValid()) return;
    const session = event.payload;
    setCurrentSession(session);
    setSessionStatus("completed");
    _stopDurationTimer();
    // ✅ NEW: Sync duration (CRITICAL - đây là final value)
    if (session.duration !== undefined && session.duration !== null) {
      setRecordingDuration(session.duration);
    }
  },
);

// meeting_failed
const failedUnlisten = await listen<MeetingSession>(
  "meeting_failed",
  (event) => {
    if (!isValid()) return;
    const session = event.payload;
    setCurrentSession(session);
    setSessionStatus("failed");
    _stopDurationTimer();
    // ✅ NEW: Sync duration (có thể có partial duration)
    if (session.duration !== undefined && session.duration !== null) {
      setRecordingDuration(session.duration);
    }
  },
);
```

**Testing:**

- [ ] Unit test: Verify each event updates `recordingDuration`
- [ ] Integration: Start → Stop → Reload page → Check duration matches

---

### Task 1A.3: UI/UX Enhancements (Optional)

**Files to modify:**

- `src/components/meeting/MeetingMode.tsx` (new useEffect)
- `src/components/meeting/MeetingErrorBoundary.tsx` (new file - optional)

**Implementation:**

```typescript
// src/components/meeting/MeetingMode.tsx

export const MeetingMode: React.FC = () => {
  const { t } = useTranslation();

  // ✅ NEW: Auto cleanup on unmount
  useEffect(() => {
    // Initialize listeners when component mounts
    useMeetingStore.getState().initializeEventListeners();

    // Cleanup when component unmounts
    return () => {
      useMeetingStore.getState().cleanupEventListeners();
    };
  }, []); // Empty deps - only on mount/unmount

  // ... rest of component
};
```

**Testing:**

- [ ] E2E: Navigate to/from meeting mode, verify no console errors
- [ ] Manual: Check browser DevTools → Memory → Listeners count

---

## Phase 1B: Backend Refactor (4-6h)

### Task 1B.1: WAV Race Condition Fix

**Problem:** `Arc::try_unwrap()` fails khi audio callback thread vẫn giữ reference → recording bị hủy

**Files to modify:**

- `src-tauri/src/managers/meeting.rs` (lines 235-255, 930-1120, 1390-1460, 1730-1770)

**Architecture Decision:**

- Introduce `WavWriterHandle` với `finalize_with_timeout` thay vì `Arc::try_unwrap`
- Use `Arc<AtomicBool>` để signal callback stop writing

**Implementation:**

```rust
// 1. Add WavWriterHandle struct (after MeetingSession struct)

use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, Ordering};

struct WavWriterHandle {
    inner: Arc<Mutex<Option<WavWriter<File>>>>,
    closed: Arc<AtomicBool>,
}

impl WavWriterHandle {
    fn new(writer: WavWriter<File>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(writer))),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn write_samples(&self, samples: &[f32]) -> Result<()> {
        // Check if closed
        if self.closed.load(Ordering::Relaxed) {
            return Ok(()); // Silently ignore writes after close
        }

        if let Ok(mut guard) = self.inner.lock() {
            if let Some(writer) = guard.as_mut() {
                for sample in samples {
                    let sample_i16 = (*sample * i16::MAX as f32) as i16;
                    writer.write_sample(sample_i16)?;
                }
                writer.flush()?;
            }
        }
        Ok(())
    }

    fn finalize_with_timeout(&self, timeout: Duration) -> Result<()> {
        // 1. Signal callback to stop writing
        self.closed.store(true, Ordering::SeqCst);

        let deadline = Instant::now() + timeout;

        // 2. Retry loop with timeout
        loop {
            if let Ok(mut guard) = self.inner.try_lock() {
                if let Some(writer) = guard.take() {
                    return writer.finalize()
                        .map_err(|e| anyhow::anyhow!("WAV finalize failed: {}", e));
                }
                return Ok(()); // Already finalized
            }

            if Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "Timeout finalizing WAV file after {:?}; partial audio saved",
                    timeout
                ));
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

// 2. Update MeetingManagerState
struct MeetingManagerState {
    current_session: Option<MeetingSession>,
    mixed_recorder: Option<MixedAudioRecorder>,
    // ✅ CHANGED: Use WavWriterHandle instead of Arc<Mutex<WavWriter>>
    wav_writer: Option<WavWriterHandle>,
}

// 3. Update start_recording (around line 930)
pub fn start_recording(&self, audio_source: AudioSourceType) -> Result<MeetingSession> {
    // ... existing validation ...

    // Create WAV writer
    let wav_writer = WavWriter::new(audio_file, spec)
        .map_err(|e| anyhow::anyhow!("Failed to create WAV writer: {}", e))?;

    // ✅ CHANGED: Wrap in WavWriterHandle
    let wav_handle = WavWriterHandle::new(wav_writer);

    // Add sample callback
    let wav_handle_clone = wav_handle.clone();
    let sample_callback = move |samples: Vec<f32>| {
        if let Err(e) = wav_handle_clone.write_samples(&samples) {
            error!("Failed to write audio samples: {}", e);
        }
    };

    // ... rest of start_recording (unchanged) ...

    // Update state
    {
        let mut state = self.state.lock().unwrap();
        state.mixed_recorder = Some(mixed_recorder);
        state.wav_writer = Some(wav_handle); // ✅ CHANGED
        state.current_session = Some(session_with_audio.clone());
    }

    // ... rest of function
}

// 4. Update stop_recording (around line 1097)
pub fn stop_recording(&self) -> Result<String> {
    // ... existing validation ...

    // Stop audio capture (unchanged)
    // ...

    // ✅ CHANGED: Finalize WAV file with timeout
    let wav_writer_opt = {
        let mut state = self.state.lock().unwrap();
        state.wav_writer.take()
    };

    if let Some(wav_handle) = wav_writer_opt {
        // Try to finalize with 5 second timeout
        if let Err(e) = wav_handle.finalize_with_timeout(Duration::from_secs(5)) {
            error!("Failed to finalize WAV file: {}", e);
            // Continue anyway - partial audio is saved
        } else {
            info!("Finalized WAV file for session {}", session_id);
        }
    }

    // ... rest of stop_recording (unchanged)
}

// 5. Update handle_mic_disconnect (around line 1405)
pub fn handle_mic_disconnect(&self, error_message: &str) {
    // ... existing validation ...

    // ✅ CHANGED: Finalize with timeout
    let wav_writer_opt = {
        let mut state = self.state.lock().unwrap();
        state.wav_writer.take()
    };

    if let Some(wav_handle) = wav_writer_opt {
        if let Err(e) = wav_handle.finalize_with_timeout(Duration::from_secs(5)) {
            error!("Failed to finalize WAV during mic disconnect: {}", e);
            // Continue - we still update status
        } else {
            info!("Finalized partial audio for session {}", session_id);
        }
    }

    // ... rest of handle_mic_disconnect (unchanged)
}

// 6. Update handle_app_shutdown (around line 1744)
pub fn handle_app_shutdown(&self) -> bool {
    // ... existing validation ...

    // ✅ CHANGED: Finalize with timeout
    let wav_writer_opt = {
        let mut state = self.state.lock().unwrap();
        state.wav_writer.take()
    };

    if let Some(wav_handle) = wav_writer_opt {
        if let Err(e) = wav_handle.finalize_with_timeout(Duration::from_secs(5)) {
            error!("Failed to finalize WAV during shutdown: {}", e);
        } else {
            info!("Finalized partial audio during shutdown");
        }
    }

    // ... rest of handle_app_shutdown (unchanged)
}
```

**Testing:**

- [ ] Unit: `test_finalize_with_timeout_success` - verify normal finalize works
- [ ] Unit: `test_finalize_with_timeout_timeout` - hold mutex in thread, force timeout
- [ ] Integration: `test_stop_recording_with_slow_callback` - delay callback, verify success
- [ ] Edge: `test_double_finalize` - call finalize twice, verify no panic
- [ ] Edge: `test_finalize_empty_samples` - zero samples, verify valid WAV file

---

### Task 1B.2: Mic Disconnect Integration

**Problem:** `handle_mic_disconnect` không được gọi từ audio stream errors → user không biết mic disconnect

**Files to modify:**

- `src-tauri/src/audio_toolkit/mixed_recorder.rs` (lines 32-210)
- `src-tauri/src/managers/meeting.rs` (lines 950-980)

**Implementation:**

```rust
// 1. Update MixedAudioRecorder struct (mixed_recorder.rs)

pub struct MixedAudioRecorder {
    config: AudioSourceConfig,
    sample_callback: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
    // ✅ NEW: Error callback
    error_callback: Option<Arc<dyn Fn(String) + Send + Sync + 'static>>,
    // ... existing fields
}

impl MixedAudioRecorder {
    pub fn new(config: AudioSourceConfig) -> Result<Self> {
        Ok(Self {
            config,
            sample_callback: None,
            error_callback: None, // ✅ NEW
            // ... existing fields
        })
    }

    // ✅ NEW: Add error callback setter
    pub fn with_error_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        self.error_callback = Some(Arc::new(cb));
        self
    }

    // 2. Update start() to wire error callback to AudioRecorder
    pub fn start(&mut self) -> Result<()> {
        match &self.config {
            AudioSourceConfig::MicrophoneOnly => {
                let mut mic_recorder = AudioRecorder::new_with_default_input()?;

                // Wire sample callback
                if let Some(cb) = &self.sample_callback {
                    let cb_clone = Arc::clone(cb);
                    mic_recorder = mic_recorder.with_sample_callback(move |samples| {
                        cb_clone(samples);
                    });
                }

                // ✅ NEW: Wire error callback
                if let Some(err_cb) = &self.error_callback {
                    let err_cb_clone = Arc::clone(err_cb);
                    mic_recorder = mic_recorder.with_error_callback(move |error| {
                        err_cb_clone(error);
                    });
                }

                mic_recorder.start()?;
                self.mic_recorder = Some(mic_recorder);
            }

            AudioSourceConfig::Mixed => {
                // Similar wiring for mic_recorder in Mixed mode
                // ... (same pattern as above)

                // ⚠️ Note: system_recorder might not support error callback yet
                // If not, only mic errors will be reported
            }

            // ... other configs
        }
        Ok(())
    }
}

// 3. Update MeetingSessionManager::start_recording (meeting.rs)

pub fn start_recording(&self, audio_source: AudioSourceType) -> Result<MeetingSession> {
    // ... existing setup ...

    // Create MixedAudioRecorder
    let mut mixed_recorder = MixedAudioRecorder::new(audio_config.clone())
        .map_err(|e| anyhow::anyhow!("Failed to create recorder: {}", e))?;

    // Add sample callback (existing code)
    mixed_recorder = mixed_recorder.with_sample_callback(sample_callback);

    // ✅ NEW: Add error callback
    let manager_clone = self.clone();
    let fired = Arc::new(AtomicBool::new(false));
    mixed_recorder = mixed_recorder.with_error_callback({
        let fired = Arc::clone(&fired);
        move |error| {
            // Only fire once (debounce)
            if fired.swap(true, Ordering::SeqCst) {
                return;
            }

            // Spawn async task to avoid blocking audio thread
            let manager = manager_clone.clone();
            let error_msg = error.clone();
            tauri::async_runtime::spawn(async move {
                manager.handle_mic_disconnect(&error_msg);
            });
        }
    });

    // Start audio capture
    mixed_recorder.start()
        .map_err(|e| anyhow::anyhow!("Failed to start audio: {}", e))?;

    // ... rest of function
}
```

**Testing:**

- [ ] Unit: `test_error_callback_stored` - verify callback is set
- [ ] Unit: `test_error_callback_debounce` - call twice, verify only fires once
- [ ] Integration: Simulate mic unplug → verify `mic_disconnected` event
- [ ] Integration: Error during stop → verify no double finalize
- [ ] Edge: Error callback fires after stop → verify no-op

---

## Testing Checklist

### Frontend Tests (src/)

**Event Cleanup:**

- [ ] `meetingStore.test.ts::test_init_cleanup_race` - Race condition handling
- [ ] `meetingStore.test.ts::test_multiple_init_calls` - Only last init active
- [ ] `MeetingMode.test.tsx::test_unmount_cleanup` - useEffect cleanup works

**State Sync:**

- [ ] `meetingStore.test.ts::test_duration_sync_on_completed` - Duration syncs on complete
- [ ] `meetingStore.test.ts::test_duration_sync_on_failed` - Partial duration syncs
- [ ] `MeetingControls.test.tsx::test_displays_backend_duration` - UI shows correct time

### Backend Tests (src-tauri/src/)

**WAV Handle:**

- [ ] `meeting::tests::test_wav_handle_finalize_success` - Normal finalize
- [ ] `meeting::tests::test_wav_handle_finalize_timeout` - Timeout handling
- [ ] `meeting::tests::test_wav_handle_write_after_close` - No writes after close

**Mic Disconnect:**

- [ ] `mixed_recorder::tests::test_error_callback_registration` - Callback stored
- [ ] `meeting::tests::test_mic_disconnect_during_recording` - Status → Failed
- [ ] `meeting::tests::test_mic_disconnect_after_stop` - No-op guard

### Integration Tests

- [ ] **E2E: Full recording flow** - Start → Record 10s → Stop → Verify duration ±1s
- [ ] **E2E: Mic disconnect** - Start → Unplug mic → Verify Failed status + partial audio
- [ ] **E2E: Component lifecycle** - Navigate to/from meeting 5 times → No memory leak
- [ ] **E2E: Reload during recording** - Start → Reload page → Verify Interrupted status

---

## Rollback Strategy

**If Phase 1A fails:**

1. Revert `src/stores/meetingStore.ts` changes
2. Keep existing timer-based duration (accept inaccuracy)
3. No data migration needed

**If Phase 1B fails:**

1. Revert `WavWriterHandle` → restore `Arc::try_unwrap`
2. Remove `MixedAudioRecorder::with_error_callback`
3. Accept potential recording failures on slow systems

**Database:**

- No schema changes required
- All fixes are logic-only

---

## Deployment Plan

1. **Phase 1A (Frontend):**
   - Deploy to dev → Test 24h → Deploy to prod
   - Monitor: Memory usage, duration accuracy

2. **Phase 1B (Backend):**
   - Deploy to dev → Stress test with 100+ recordings
   - Monitor: WAV finalize errors, mic disconnect events
   - Rollout: 10% → 50% → 100% over 1 week

3. **Monitoring:**
   - Track: `meeting_failed` events with "finalize timeout" error
   - Track: `mic_disconnected` events frequency
   - Alert: Duration drift > 5 seconds

---

## Success Metrics

**Phase 1A:**

- ✅ Zero memory leaks after 100 mount/unmount cycles
- ✅ Duration accuracy: <2 second drift after reload
- ✅ No event listener errors in logs

**Phase 1B:**

- ✅ WAV finalize success rate: >99.9%
- ✅ Mic disconnect detection: <5 second latency
- ✅ No recording data loss (partial audio always saved)

---

## Notes

- **Backward Compatibility:** All changes are internal, no API breaks
- **Performance:** Minimal overhead (<1ms per finalize attempt)
- **Security:** No new attack vectors introduced
- **Dependencies:** No new dependencies added

**Codex Session:** 019babad-8215-7ae0-a463-069ef7b344f1
**Gemini Session:** N/A (stateless analysis)
